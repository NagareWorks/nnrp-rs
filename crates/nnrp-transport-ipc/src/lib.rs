use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};

use async_trait::async_trait;
use nnrp_core::{CommonHeader, TransportId, COMMON_HEADER_LEN};
use nnrp_runtime::{
    BoxedFramedTransport, FramedListener, FramedTransport, NnrpClient, NnrpClientConfig,
    NnrpServer, NnrpServerConfig, RuntimeError, RuntimeFrameLimits, RuntimePacket,
    RuntimeTransportKind,
};
use nnrp_transport_provider::{
    TransportProviderDescriptor, TransportProviderKind, TransportProviderRegistry,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf},
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

const IPC_TASK_CHANNEL_CAPACITY: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcEndpoint {
    Unix(PathBuf),
    NamedPipe(String),
}

impl IpcEndpoint {
    pub fn unix(path: impl Into<PathBuf>) -> Self {
        Self::Unix(path.into())
    }

    pub fn named_pipe(name: impl Into<String>) -> Self {
        Self::NamedPipe(normalize_named_pipe(name.into()))
    }

    pub fn as_unix_path(&self) -> Option<&Path> {
        match self {
            Self::Unix(path) => Some(path.as_path()),
            Self::NamedPipe(_) => None,
        }
    }

    pub fn as_named_pipe(&self) -> Option<&str> {
        match self {
            Self::Unix(_) => None,
            Self::NamedPipe(path) => Some(path.as_str()),
        }
    }
}

impl FromStr for IpcEndpoint {
    type Err = RuntimeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if let Some(path) = value.strip_prefix("unix://") {
            if path.is_empty() {
                return Err(RuntimeError::UnsupportedTransport(
                    "unix IPC endpoint path cannot be empty",
                ));
            }
            return Ok(Self::unix(path));
        }
        if let Some(name) = value.strip_prefix("npipe://") {
            if name.is_empty() {
                return Err(RuntimeError::UnsupportedTransport(
                    "named pipe endpoint name cannot be empty",
                ));
            }
            return Ok(Self::named_pipe(name));
        }
        Err(RuntimeError::UnsupportedTransport(
            "IPC endpoint must use unix:// or npipe://",
        ))
    }
}

impl fmt::Display for IpcEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unix(path) => write!(formatter, "unix://{}", path.display()),
            Self::NamedPipe(path) => write!(formatter, "npipe://{path}"),
        }
    }
}

#[derive(Debug)]
pub struct IpcTransport {
    reader: mpsc::Receiver<Result<RuntimePacket, RuntimeError>>,
    writer: mpsc::Sender<IpcWriteCommand>,
    read_task: JoinHandle<()>,
    write_task: JoinHandle<()>,
}

impl IpcTransport {
    pub async fn connect(endpoint: &IpcEndpoint) -> Result<Self, RuntimeError> {
        Self::connect_with_limits(endpoint, RuntimeFrameLimits::default()).await
    }

    pub async fn connect_with_limits(
        endpoint: &IpcEndpoint,
        limits: RuntimeFrameLimits,
    ) -> Result<Self, RuntimeError> {
        Ok(Self::new(IpcStream::connect(endpoint).await?, limits))
    }

    fn new(stream: IpcStream, limits: RuntimeFrameLimits) -> Self {
        let (reader, writer) = tokio::io::split(stream);
        let (packet_tx, packet_rx) = mpsc::channel(IPC_TASK_CHANNEL_CAPACITY);
        let (write_tx, write_rx) = mpsc::channel(IPC_TASK_CHANNEL_CAPACITY);
        Self {
            reader: packet_rx,
            writer: write_tx,
            read_task: spawn_ipc_read_task(reader, limits, packet_tx),
            write_task: spawn_ipc_write_task(writer, limits, write_rx),
        }
    }
}

impl Drop for IpcTransport {
    fn drop(&mut self) {
        self.read_task.abort();
        self.write_task.abort();
    }
}

#[async_trait]
impl FramedTransport for IpcTransport {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::Ipc
    }

    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
        self.reader
            .recv()
            .await
            .ok_or_else(|| ipc_task_closed("read"))?
    }

    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
        let (ack, receiver) = oneshot::channel();
        self.writer
            .send(IpcWriteCommand::Packet {
                packet: packet.clone(),
                ack,
            })
            .await
            .map_err(|_| ipc_task_closed("write"))?;
        receiver.await.map_err(|_| ipc_task_closed("write"))?
    }

    async fn close(&mut self) -> Result<(), RuntimeError> {
        let (ack, receiver) = oneshot::channel();
        self.writer
            .send(IpcWriteCommand::Close { ack })
            .await
            .map_err(|_| ipc_task_closed("write"))?;
        receiver.await.map_err(|_| ipc_task_closed("write"))?
    }
}

enum IpcWriteCommand {
    Packet {
        packet: RuntimePacket,
        ack: oneshot::Sender<Result<(), RuntimeError>>,
    },
    Close {
        ack: oneshot::Sender<Result<(), RuntimeError>>,
    },
}

fn spawn_ipc_read_task(
    mut reader: ReadHalf<IpcStream>,
    limits: RuntimeFrameLimits,
    packets: mpsc::Sender<Result<RuntimePacket, RuntimeError>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let result = read_packet(&mut reader, limits).await;
            let should_stop = result.is_err();
            if packets.send(result).await.is_err() || should_stop {
                break;
            }
        }
    })
}

fn spawn_ipc_write_task(
    mut writer: WriteHalf<IpcStream>,
    limits: RuntimeFrameLimits,
    mut commands: mpsc::Receiver<IpcWriteCommand>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(command) = commands.recv().await {
            match command {
                IpcWriteCommand::Packet { packet, ack } => {
                    let result = write_packet(&mut writer, &packet, limits).await;
                    let should_stop = result.is_err();
                    let _ = ack.send(result);
                    if should_stop {
                        break;
                    }
                }
                IpcWriteCommand::Close { ack } => {
                    let result = writer.shutdown().await.map_err(Into::into);
                    let _ = ack.send(result);
                    break;
                }
            }
        }
    })
}

fn ipc_task_closed(direction: &'static str) -> RuntimeError {
    RuntimeError::TransportClosed {
        transport: RuntimeTransportKind::Ipc,
        detail: format!("ipc {direction} task stopped"),
    }
}

#[derive(Debug)]
pub struct IpcFramedListener {
    inner: IpcListener,
    limits: RuntimeFrameLimits,
}

impl IpcFramedListener {
    pub async fn bind(endpoint: &IpcEndpoint) -> Result<Self, RuntimeError> {
        Self::bind_with_limits(endpoint, RuntimeFrameLimits::default()).await
    }

    pub async fn bind_with_limits(
        endpoint: &IpcEndpoint,
        limits: RuntimeFrameLimits,
    ) -> Result<Self, RuntimeError> {
        Ok(Self {
            inner: IpcListener::bind(endpoint).await?,
            limits,
        })
    }
}

#[async_trait]
impl FramedListener for IpcFramedListener {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::Ipc
    }

    fn local_addr(&self) -> Result<std::net::SocketAddr, RuntimeError> {
        Err(RuntimeError::UnsupportedTransport(
            "IPC listener does not expose an IP socket address",
        ))
    }

    async fn accept(&self) -> Result<BoxedFramedTransport, RuntimeError> {
        Ok(Box::new(IpcTransport::new(
            self.inner.accept().await?,
            self.limits,
        )))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IpcProvider;

impl IpcProvider {
    pub const NAME: &'static str = "nnrp-transport-ipc";

    pub fn descriptor() -> TransportProviderDescriptor {
        TransportProviderDescriptor::available(
            Self::NAME,
            env!("CARGO_PKG_VERSION"),
            TransportId::Ipc,
            TransportProviderKind::PureRust,
        )
    }

    pub fn register(registry: &mut TransportProviderRegistry) {
        registry.register(Self::descriptor());
    }

    pub async fn connect_transport(endpoint: &IpcEndpoint) -> Result<IpcTransport, RuntimeError> {
        IpcTransport::connect(endpoint).await
    }

    pub async fn bind_listener(endpoint: &IpcEndpoint) -> Result<IpcFramedListener, RuntimeError> {
        IpcFramedListener::bind(endpoint).await
    }

    pub async fn connect(
        endpoint: &IpcEndpoint,
        config: NnrpClientConfig,
    ) -> Result<NnrpClient, RuntimeError> {
        NnrpClient::from_transport(
            Self::connect_transport(endpoint).await?,
            config.with_transport(RuntimeTransportKind::Ipc),
        )
    }

    pub async fn bind(
        endpoint: &IpcEndpoint,
        config: NnrpServerConfig,
    ) -> Result<NnrpServer, RuntimeError> {
        NnrpServer::from_listener(
            Self::bind_listener(endpoint).await?,
            config.with_transport(RuntimeTransportKind::Ipc),
        )
    }
}

pub fn register_ipc_provider(registry: &mut TransportProviderRegistry) {
    IpcProvider::register(registry);
}

async fn read_packet<R>(
    reader: &mut R,
    limits: RuntimeFrameLimits,
) -> Result<RuntimePacket, RuntimeError>
where
    R: AsyncRead + Unpin + Send,
{
    let mut header_bytes = [0u8; COMMON_HEADER_LEN];
    reader.read_exact(&mut header_bytes).await?;
    let header = CommonHeader::parse(&header_bytes)?;
    limits.validate_packet_len(header.packet_len()?)?;

    let mut metadata = vec![0u8; header.meta_len as usize];
    if !metadata.is_empty() {
        reader.read_exact(&mut metadata).await?;
    }

    let mut body = vec![0u8; header.body_len as usize];
    if !body.is_empty() {
        reader.read_exact(&mut body).await?;
    }

    RuntimePacket::from_parts(header, metadata, body).map_err(Into::into)
}

async fn write_packet<W>(
    writer: &mut W,
    packet: &RuntimePacket,
    limits: RuntimeFrameLimits,
) -> Result<(), RuntimeError>
where
    W: AsyncWrite + Unpin + Send,
{
    let bytes = packet.to_bytes()?;
    limits.validate_packet_len(bytes.len())?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}

fn normalize_named_pipe(value: String) -> String {
    let trimmed = value.trim_start_matches('/');
    if trimmed.starts_with(r"\\.\pipe\") {
        trimmed.to_string()
    } else {
        format!(r"\\.\pipe\{trimmed}")
    }
}

#[cfg(unix)]
type PlatformIpcStream = tokio::net::UnixStream;

#[cfg(windows)]
#[derive(Debug)]
enum PlatformIpcStream {
    Client(tokio::net::windows::named_pipe::NamedPipeClient),
    Server(tokio::net::windows::named_pipe::NamedPipeServer),
}

#[derive(Debug)]
struct IpcStream {
    inner: PlatformIpcStream,
}

impl IpcStream {
    async fn connect(endpoint: &IpcEndpoint) -> Result<Self, RuntimeError> {
        match endpoint {
            IpcEndpoint::Unix(path) => connect_unix(path).await,
            IpcEndpoint::NamedPipe(path) => connect_named_pipe(path).await,
        }
    }
}

impl AsyncRead for IpcStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
        buffer: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        #[cfg(unix)]
        {
            std::pin::Pin::new(&mut self.inner).poll_read(context, buffer)
        }
        #[cfg(windows)]
        {
            match &mut self.inner {
                PlatformIpcStream::Client(pipe) => {
                    std::pin::Pin::new(pipe).poll_read(context, buffer)
                }
                PlatformIpcStream::Server(pipe) => {
                    std::pin::Pin::new(pipe).poll_read(context, buffer)
                }
            }
        }
    }
}

impl AsyncWrite for IpcStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
        buffer: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        #[cfg(unix)]
        {
            std::pin::Pin::new(&mut self.inner).poll_write(context, buffer)
        }
        #[cfg(windows)]
        {
            match &mut self.inner {
                PlatformIpcStream::Client(pipe) => {
                    std::pin::Pin::new(pipe).poll_write(context, buffer)
                }
                PlatformIpcStream::Server(pipe) => {
                    std::pin::Pin::new(pipe).poll_write(context, buffer)
                }
            }
        }
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        #[cfg(unix)]
        {
            std::pin::Pin::new(&mut self.inner).poll_flush(context)
        }
        #[cfg(windows)]
        {
            match &mut self.inner {
                PlatformIpcStream::Client(pipe) => std::pin::Pin::new(pipe).poll_flush(context),
                PlatformIpcStream::Server(pipe) => std::pin::Pin::new(pipe).poll_flush(context),
            }
        }
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        #[cfg(unix)]
        {
            std::pin::Pin::new(&mut self.inner).poll_shutdown(context)
        }
        #[cfg(windows)]
        {
            match &mut self.inner {
                PlatformIpcStream::Client(pipe) => std::pin::Pin::new(pipe).poll_shutdown(context),
                PlatformIpcStream::Server(pipe) => std::pin::Pin::new(pipe).poll_shutdown(context),
            }
        }
    }
}

#[derive(Debug)]
struct IpcListener {
    inner: PlatformIpcListener,
}

impl IpcListener {
    async fn bind(endpoint: &IpcEndpoint) -> Result<Self, RuntimeError> {
        match endpoint {
            IpcEndpoint::Unix(path) => bind_unix(path).await,
            IpcEndpoint::NamedPipe(path) => bind_named_pipe(path).await,
        }
    }

    async fn accept(&self) -> Result<IpcStream, RuntimeError> {
        self.inner.accept().await
    }
}

#[cfg(unix)]
type PlatformIpcListener = UnixIpcListener;

#[cfg(unix)]
#[derive(Debug)]
struct UnixIpcListener {
    listener: tokio::net::UnixListener,
}

#[cfg(unix)]
impl UnixIpcListener {
    async fn accept(&self) -> Result<IpcStream, RuntimeError> {
        let (stream, _) = self.listener.accept().await?;
        Ok(IpcStream { inner: stream })
    }
}

#[cfg(unix)]
async fn connect_unix(path: &Path) -> Result<IpcStream, RuntimeError> {
    Ok(IpcStream {
        inner: tokio::net::UnixStream::connect(path).await?,
    })
}

#[cfg(not(unix))]
async fn connect_unix(_path: &Path) -> Result<IpcStream, RuntimeError> {
    Err(RuntimeError::UnsupportedTransport(
        "unix IPC endpoint is not available on this platform",
    ))
}

#[cfg(unix)]
async fn bind_unix(path: &Path) -> Result<IpcListener, RuntimeError> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(IpcListener {
        inner: UnixIpcListener {
            listener: tokio::net::UnixListener::bind(path)?,
        },
    })
}

#[cfg(not(unix))]
async fn bind_unix(_path: &Path) -> Result<IpcListener, RuntimeError> {
    Err(RuntimeError::UnsupportedTransport(
        "unix IPC endpoint is not available on this platform",
    ))
}

#[cfg(windows)]
type PlatformIpcListener = WindowsIpcListener;

#[cfg(windows)]
#[derive(Debug)]
struct WindowsIpcListener {
    pipe_name: String,
}

#[cfg(windows)]
impl WindowsIpcListener {
    async fn accept(&self) -> Result<IpcStream, RuntimeError> {
        let server =
            tokio::net::windows::named_pipe::ServerOptions::new().create(&self.pipe_name)?;
        server.connect().await?;
        Ok(IpcStream {
            inner: PlatformIpcStream::Server(server),
        })
    }
}

#[cfg(windows)]
async fn connect_named_pipe(path: &str) -> Result<IpcStream, RuntimeError> {
    Ok(IpcStream {
        inner: PlatformIpcStream::Client(
            tokio::net::windows::named_pipe::ClientOptions::new().open(path)?,
        ),
    })
}

#[cfg(not(windows))]
async fn connect_named_pipe(_path: &str) -> Result<IpcStream, RuntimeError> {
    Err(RuntimeError::UnsupportedTransport(
        "named pipe IPC endpoint is not available on this platform",
    ))
}

#[cfg(windows)]
async fn bind_named_pipe(path: &str) -> Result<IpcListener, RuntimeError> {
    Ok(IpcListener {
        inner: WindowsIpcListener {
            pipe_name: path.to_string(),
        },
    })
}

#[cfg(not(windows))]
async fn bind_named_pipe(_path: &str) -> Result<IpcListener, RuntimeError> {
    Err(RuntimeError::UnsupportedTransport(
        "named pipe IPC endpoint is not available on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use nnrp_core::{
        BackpressureLevel, CommonHeader, FrameSubmitMetadata, InputProfile, MessageType,
        PayloadKindBitmap, PressureMetadata, ResultClass, ResultDropReasonMetadata,
        ResultPushMetadata, SubmitMode, TileIndexMode, STANDARD_PROFILE_TOKEN,
    };
    #[cfg(unix)]
    use nnrp_runtime::{NnrpClientEvent, NnrpResult};
    use nnrp_transport_provider::{RemoteTransportSupport, TransportPolicy};
    #[cfg(unix)]
    use tokio::time::{timeout, Duration};

    #[test]
    fn ipc_endpoint_parses_platform_schemes() {
        let unix = "unix:///tmp/nnrp.sock".parse::<IpcEndpoint>().unwrap();
        assert_eq!(unix, IpcEndpoint::unix("/tmp/nnrp.sock"));
        assert_eq!(unix.as_unix_path(), Some(Path::new("/tmp/nnrp.sock")));
        assert_eq!(unix.as_named_pipe(), None);
        assert_eq!(unix.to_string(), "unix:///tmp/nnrp.sock");

        let pipe = "npipe://nnrp-test".parse::<IpcEndpoint>().unwrap();
        assert_eq!(pipe, IpcEndpoint::named_pipe("nnrp-test"));
        assert_eq!(pipe.as_unix_path(), None);
        assert_eq!(pipe.as_named_pipe(), Some(r"\\.\pipe\nnrp-test"));
        assert_eq!(pipe.to_string(), r"npipe://\\.\pipe\nnrp-test");
        assert_eq!(
            IpcEndpoint::named_pipe(r"\\.\pipe\nnrp-test").as_named_pipe(),
            Some(r"\\.\pipe\nnrp-test")
        );
        assert!("unix://".parse::<IpcEndpoint>().is_err());
        assert!("npipe://".parse::<IpcEndpoint>().is_err());
        assert!("tcp://127.0.0.1:1".parse::<IpcEndpoint>().is_err());
    }

    #[test]
    fn ipc_provider_registers_and_selects_ipc() {
        let mut registry = TransportProviderRegistry::new();
        register_ipc_provider(&mut registry);
        assert_eq!(registry.providers().len(), 1);
        assert_eq!(registry.providers()[0].name, IpcProvider::NAME);
        assert_eq!(registry.providers()[0].transport_id, TransportId::Ipc);

        let remote = RemoteTransportSupport::new([TransportId::Ipc]);
        let selection = registry
            .select(&remote, TransportPolicy::ForceIpc)
            .expect("ipc provider should satisfy force ipc");
        assert_eq!(selection.selected.name, IpcProvider::NAME);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unix_ipc_loopback_submits_frame_and_receives_result() -> Result<(), RuntimeError> {
        let path = std::env::temp_dir().join(format!(
            "nnrp-ipc-{}-{}.sock",
            std::process::id(),
            unique_suffix()
        ));
        let endpoint = IpcEndpoint::unix(path.clone());
        let server = IpcProvider::bind(&endpoint, NnrpServerConfig::default()).await?;

        let server_task = tokio::spawn(async move {
            let mut session = server.accept().await?;
            let submit = session.receive_submit().await?;
            session
                .send_result(submit.frame_id, token_result(), b"ipc-ok".to_vec())
                .await
        });

        let client = IpcProvider::connect(&endpoint, NnrpClientConfig::default()).await?;
        let mut session = client.open_session().await?;
        session.submit(token_submit(), b"hello".to_vec()).await?;
        let NnrpResult { body, .. } = session.await_result().await?;
        assert_eq!(body, b"ipc-ok");

        server_task
            .await
            .map_err(|_| RuntimeError::Internal("ipc server task panicked"))??;
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unix_ipc_raw_transport_exposes_kind_and_close() -> Result<(), RuntimeError> {
        let path = std::env::temp_dir().join(format!(
            "nnrp-ipc-raw-{}-{}.sock",
            std::process::id(),
            unique_suffix()
        ));
        let endpoint = IpcEndpoint::unix(path.clone());
        let listener = IpcProvider::bind_listener(&endpoint).await?;
        assert!(matches!(
            listener.local_addr(),
            Err(RuntimeError::UnsupportedTransport(_))
        ));

        let server_task = tokio::spawn(async move {
            let mut accepted = listener.accept().await?;
            assert_eq!(accepted.transport_kind(), RuntimeTransportKind::Ipc);
            accepted.close().await
        });

        let mut client = IpcProvider::connect_transport(&endpoint).await?;
        assert_eq!(client.transport_kind(), RuntimeTransportKind::Ipc);
        client.close().await?;

        server_task
            .await
            .map_err(|_| RuntimeError::Internal("ipc raw server task panicked"))??;
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unix_ipc_read_wait_survives_caller_cancellation() -> Result<(), RuntimeError> {
        let path = std::env::temp_dir().join(format!(
            "nnrp-ipc-cancel-read-{}-{}.sock",
            std::process::id(),
            unique_suffix()
        ));
        let endpoint = IpcEndpoint::unix(path.clone());
        let listener = IpcProvider::bind_listener(&endpoint).await?;

        let server_task = tokio::spawn(async move {
            let mut accepted = listener.accept().await?;
            tokio::time::sleep(Duration::from_millis(50)).await;
            accepted.write_packet(&cancellation_probe_packet()).await?;
            accepted.close().await
        });

        let mut client = IpcProvider::connect_transport(&endpoint).await?;
        let cancelled = timeout(Duration::from_millis(10), client.read_packet()).await;
        assert!(
            cancelled.is_err(),
            "first read wait should time out before the server writes"
        );

        let packet = timeout(Duration::from_secs(1), client.read_packet())
            .await
            .map_err(|_| RuntimeError::Internal("ipc cancellation probe timed out"))??;
        assert_eq!(packet.body, b"ipc-cancel-safe");
        client.close().await?;

        server_task
            .await
            .map_err(|_| RuntimeError::Internal("ipc cancellation server task panicked"))??;
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unix_ipc_worker_shutdown_reports_transport_closed() -> Result<(), RuntimeError> {
        let path = std::env::temp_dir().join(format!(
            "nnrp-ipc-worker-stop-{}-{}.sock",
            std::process::id(),
            unique_suffix()
        ));
        let endpoint = IpcEndpoint::unix(path.clone());
        let listener = IpcProvider::bind_listener(&endpoint).await?;

        let server_task = tokio::spawn(async move {
            let mut accepted = listener.accept().await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            accepted.close().await
        });

        let mut client = IpcProvider::connect_transport(&endpoint).await?;
        client.read_task.abort();
        tokio::task::yield_now().await;
        assert_ipc_closed(
            client
                .read_packet()
                .await
                .expect_err("read task is stopped"),
        );

        client.write_task.abort();
        tokio::task::yield_now().await;
        assert_ipc_closed(
            client
                .write_packet(&cancellation_probe_packet())
                .await
                .expect_err("write task is stopped"),
        );
        assert_ipc_closed(client.close().await.expect_err("write task is stopped"));

        server_task
            .await
            .map_err(|_| RuntimeError::Internal("ipc worker stop server task panicked"))??;
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unix_ipc_loopback_routes_cancel_drop_reason_and_pressure() -> Result<(), RuntimeError>
    {
        let path = std::env::temp_dir().join(format!(
            "nnrp-ipc-control-{}-{}.sock",
            std::process::id(),
            unique_suffix()
        ));
        let endpoint = IpcEndpoint::unix(path.clone());
        let server = IpcProvider::bind(&endpoint, NnrpServerConfig::default()).await?;

        let server_task = tokio::spawn(async move {
            let mut session = server.accept().await?;
            let submit = session.receive_submit().await?;
            let credit = session.receive_pressure_update().await?;
            assert_eq!(credit.metadata.credit_window, 9);
            session.send_backpressure(soft_backpressure()).await?;

            let control = session.receive_runtime_control().await?;
            assert_eq!(control.metadata.operation_id, submit.frame_id as u64);
            session
                .send_result_drop_reason(drop_reason(submit.frame_id as u64))
                .await
        });

        let client = IpcProvider::connect(&endpoint, NnrpClientConfig::default()).await?;
        let mut session = client.open_session().await?;
        let frame_id = session
            .submit_nowait(token_submit(), b"cancel-me".to_vec())
            .await?;
        session.send_credit_update(credit_update()).await?;
        session.cancel_operation(frame_id as u64, 7).await?;

        match session.await_event().await? {
            NnrpClientEvent::Backpressure(pressure) => {
                assert_eq!(pressure.pressure_level, BackpressureLevel::Soft as u16);
                assert_eq!(pressure.credit_window, 2);
            }
            event => panic!("expected backpressure event, got {event:?}"),
        }
        match session.await_event().await? {
            NnrpClientEvent::ResultDropReason {
                metadata: reason,
                body,
            } => {
                assert_eq!(reason.operation_id, frame_id as u64);
                assert_eq!(reason.drop_reason_code, 7);
                assert!(body.is_empty());
            }
            event => panic!("expected result drop reason event, got {event:?}"),
        }

        server_task
            .await
            .map_err(|_| RuntimeError::Internal("ipc control server task panicked"))??;
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn named_pipe_endpoints_report_platform_boundary() {
        let endpoint = IpcEndpoint::named_pipe("nnrp-no-windows");
        assert!(matches!(
            IpcProvider::connect_transport(&endpoint).await,
            Err(RuntimeError::UnsupportedTransport(_))
        ));
        assert!(matches!(
            IpcProvider::bind_listener(&endpoint).await,
            Err(RuntimeError::UnsupportedTransport(_))
        ));
    }

    #[cfg(unix)]
    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos()
    }

    #[cfg(unix)]
    fn token_submit() -> FrameSubmitMetadata {
        FrameSubmitMetadata {
            src_width: 0,
            src_height: 0,
            tile_width: 0,
            tile_height: 0,
            tile_count: 0,
            section_count: 0,
            frame_class: 0,
            input_profile: InputProfile::Unspecified,
            tile_index_mode: TileIndexMode::DenseRange,
            latency_budget_ms: 25,
            target_fps_x100: 0,
            retry_of_frame: 0,
            tile_base_id: 0,
            camera_bytes: 0,
            tile_index_bytes: 0,
            submit_mode: SubmitMode::Inline,
            budget_policy: 0,
            loss_tolerance_policy: 0,
            object_ref_mask: 0,
            dependency_frame_id: 0,
            payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
            payload_frame_count: 1,
        }
    }

    #[cfg(unix)]
    fn token_result() -> ResultPushMetadata {
        ResultPushMetadata {
            status_code: 200,
            result_flags: 0,
            section_count: 0,
            tile_count: 0,
            active_profile_id: STANDARD_PROFILE_TOKEN,
            inference_ms: 1,
            queue_ms: 0,
            server_total_ms: 1,
            tile_base_id: 0,
            tile_index_bytes: 0,
            result_class: ResultClass::Complete,
            applied_budget_policy: 0,
            reused_frame_id: 0,
            covered_tile_count: 0,
            dropped_tile_count: 0,
            payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
            payload_frame_count: 1,
        }
    }

    #[cfg(unix)]
    fn credit_update() -> PressureMetadata {
        PressureMetadata {
            scope_id: 1,
            credit_window: 9,
            pressure_level: BackpressureLevel::None as u16,
            pressure_reason: 0,
            retry_after_ms: 0,
            flags: 0,
        }
    }

    #[cfg(unix)]
    fn soft_backpressure() -> PressureMetadata {
        PressureMetadata {
            scope_id: 1,
            credit_window: 2,
            pressure_level: BackpressureLevel::Soft as u16,
            pressure_reason: 1,
            retry_after_ms: 25,
            flags: 0,
        }
    }

    #[cfg(unix)]
    fn drop_reason(operation_id: u64) -> ResultDropReasonMetadata {
        ResultDropReasonMetadata {
            operation_id,
            result_sequence: 1,
            drop_reason_code: 7,
            source_role: 2,
            flags: 0,
            diagnostic_bytes: 0,
        }
    }

    #[cfg(unix)]
    fn cancellation_probe_packet() -> RuntimePacket {
        RuntimePacket::new(
            CommonHeader::new(MessageType::Ping, 0, 15),
            Vec::new(),
            b"ipc-cancel-safe".to_vec(),
        )
        .expect("cancellation probe packet should build")
    }

    #[cfg(unix)]
    fn assert_ipc_closed(error: RuntimeError) {
        match error {
            RuntimeError::TransportClosed { transport, detail } => {
                assert_eq!(transport, RuntimeTransportKind::Ipc);
                assert!(detail.contains("ipc"));
                assert!(detail.contains("task stopped"));
            }
            error => panic!("expected ipc transport closed diagnostic, got {error:?}"),
        }
    }
}
