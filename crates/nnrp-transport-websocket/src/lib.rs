use std::{fmt, net::SocketAddr, str::FromStr};

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use nnrp_core::{CommonHeader, TransportId, COMMON_HEADER_LEN};
use nnrp_runtime::{
    BoxedFramedTransport, FramedListener, FramedTransport, NnrpClient, NnrpClientConfig,
    NnrpServer, NnrpServerConfig, RuntimeError, RuntimeFrameLimits, RuntimePacket,
    RuntimeTransportKind,
};
use nnrp_transport_provider::{
    TransportProviderDescriptor, TransportProviderKind, TransportProviderRegistry,
};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio_tungstenite::{
    accept_async, connect_async,
    tungstenite::{Error as WebSocketError, Message},
    MaybeTlsStream, WebSocketStream,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSocketEndpoint {
    Ws(String),
    Wss(String),
}

impl WebSocketEndpoint {
    pub fn ws(uri: impl Into<String>) -> Result<Self, RuntimeError> {
        parse_endpoint(uri.into(), "ws://", Self::Ws)
    }

    pub fn wss(uri: impl Into<String>) -> Result<Self, RuntimeError> {
        parse_endpoint(uri.into(), "wss://", Self::Wss)
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Ws(uri) | Self::Wss(uri) => uri,
        }
    }

    pub fn is_secure(&self) -> bool {
        matches!(self, Self::Wss(_))
    }
}

impl FromStr for WebSocketEndpoint {
    type Err = RuntimeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.starts_with("ws://") {
            Self::ws(value)
        } else if value.starts_with("wss://") {
            Self::wss(value)
        } else {
            Err(RuntimeError::UnsupportedTransport(
                "WebSocket endpoint must use ws:// or wss://",
            ))
        }
    }
}

impl fmt::Display for WebSocketEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug)]
pub struct WebSocketTransport {
    stream: WebSocketStreamKind,
    limits: RuntimeFrameLimits,
}

impl WebSocketTransport {
    pub async fn connect(endpoint: &WebSocketEndpoint) -> Result<Self, RuntimeError> {
        Self::connect_with_limits(endpoint, RuntimeFrameLimits::default()).await
    }

    pub async fn connect_with_limits(
        endpoint: &WebSocketEndpoint,
        limits: RuntimeFrameLimits,
    ) -> Result<Self, RuntimeError> {
        let (stream, _) = connect_async(endpoint.as_str()).await.map_err(runtime_ws)?;
        Ok(Self {
            stream: WebSocketStreamKind::Client(stream),
            limits,
        })
    }

    fn server(stream: WebSocketStream<TcpStream>, limits: RuntimeFrameLimits) -> Self {
        Self {
            stream: WebSocketStreamKind::Server(stream),
            limits,
        }
    }
}

#[async_trait]
impl FramedTransport for WebSocketTransport {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::WebSocket
    }

    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
        loop {
            let message =
                self.stream
                    .next_message()
                    .await?
                    .ok_or(RuntimeError::UnexpectedMessage(
                        "websocket stream closed before an NNRP binary frame",
                    ))?;
            match message {
                Message::Binary(bytes) => return packet_from_binary(bytes.as_ref(), self.limits),
                Message::Text(_) => {
                    return Err(RuntimeError::UnexpectedMessage(
                        "websocket text messages are not valid NNRP data frames",
                    ));
                }
                Message::Close(_) => {
                    return Err(RuntimeError::UnexpectedMessage(
                        "websocket close frame received before an NNRP data frame",
                    ));
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
            }
        }
    }

    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
        let bytes = packet.to_bytes()?;
        self.limits.validate_packet_len(bytes.len())?;
        self.stream
            .send_message(Message::Binary(bytes.into()))
            .await
            .map_err(runtime_ws)
    }

    async fn close(&mut self) -> Result<(), RuntimeError> {
        self.stream.close().await.map_err(runtime_ws)
    }
}

#[derive(Debug)]
pub struct WebSocketFramedListener {
    listener: TcpListener,
    limits: RuntimeFrameLimits,
}

impl WebSocketFramedListener {
    pub async fn bind(addr: impl ToSocketAddrs) -> Result<Self, RuntimeError> {
        Self::bind_with_limits(addr, RuntimeFrameLimits::default()).await
    }

    pub async fn bind_with_limits(
        addr: impl ToSocketAddrs,
        limits: RuntimeFrameLimits,
    ) -> Result<Self, RuntimeError> {
        Ok(Self {
            listener: TcpListener::bind(addr).await?,
            limits,
        })
    }
}

#[async_trait]
impl FramedListener for WebSocketFramedListener {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::WebSocket
    }

    fn local_addr(&self) -> Result<SocketAddr, RuntimeError> {
        Ok(self.listener.local_addr()?)
    }

    async fn accept(&self) -> Result<BoxedFramedTransport, RuntimeError> {
        let (stream, _) = self.listener.accept().await?;
        let websocket = accept_async(stream).await.map_err(runtime_ws)?;
        Ok(Box::new(WebSocketTransport::server(websocket, self.limits)))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WebSocketProvider;

impl WebSocketProvider {
    pub const NAME: &'static str = "nnrp-transport-websocket";

    pub fn descriptor() -> TransportProviderDescriptor {
        TransportProviderDescriptor::available(
            Self::NAME,
            env!("CARGO_PKG_VERSION"),
            TransportId::WebSocket,
            TransportProviderKind::PureRust,
        )
    }

    pub fn register(registry: &mut TransportProviderRegistry) {
        registry.register(Self::descriptor());
    }

    pub async fn connect_transport(
        endpoint: &WebSocketEndpoint,
    ) -> Result<WebSocketTransport, RuntimeError> {
        WebSocketTransport::connect(endpoint).await
    }

    pub async fn bind_listener(
        addr: impl ToSocketAddrs,
    ) -> Result<WebSocketFramedListener, RuntimeError> {
        WebSocketFramedListener::bind(addr).await
    }

    pub async fn connect(
        endpoint: &WebSocketEndpoint,
        config: NnrpClientConfig,
    ) -> Result<NnrpClient, RuntimeError> {
        NnrpClient::from_transport(
            Self::connect_transport(endpoint).await?,
            config.with_transport(RuntimeTransportKind::WebSocket),
        )
    }

    pub async fn bind(
        addr: impl ToSocketAddrs,
        config: NnrpServerConfig,
    ) -> Result<NnrpServer, RuntimeError> {
        NnrpServer::from_listener(
            Self::bind_listener(addr).await?,
            config.with_transport(RuntimeTransportKind::WebSocket),
        )
    }
}

pub fn register_websocket_provider(registry: &mut TransportProviderRegistry) {
    WebSocketProvider::register(registry);
}

#[derive(Debug)]
enum WebSocketStreamKind {
    Client(WebSocketStream<MaybeTlsStream<TcpStream>>),
    Server(WebSocketStream<TcpStream>),
}

impl WebSocketStreamKind {
    async fn next_message(&mut self) -> Result<Option<Message>, RuntimeError> {
        match self {
            Self::Client(stream) => stream.next().await.transpose().map_err(runtime_ws),
            Self::Server(stream) => stream.next().await.transpose().map_err(runtime_ws),
        }
    }

    async fn send_message(&mut self, message: Message) -> Result<(), WebSocketError> {
        match self {
            Self::Client(stream) => stream.send(message).await,
            Self::Server(stream) => stream.send(message).await,
        }
    }

    async fn close(&mut self) -> Result<(), WebSocketError> {
        match self {
            Self::Client(stream) => stream.close(None).await,
            Self::Server(stream) => stream.close(None).await,
        }
    }
}

fn parse_endpoint(
    value: String,
    expected_prefix: &'static str,
    constructor: impl FnOnce(String) -> WebSocketEndpoint,
) -> Result<WebSocketEndpoint, RuntimeError> {
    if !value.starts_with(expected_prefix) {
        return Err(RuntimeError::UnsupportedTransport(
            "WebSocket endpoint scheme does not match constructor",
        ));
    }
    if value[expected_prefix.len()..].is_empty() {
        return Err(RuntimeError::UnsupportedTransport(
            "WebSocket endpoint authority cannot be empty",
        ));
    }
    Ok(constructor(value))
}

fn packet_from_binary(
    bytes: &[u8],
    limits: RuntimeFrameLimits,
) -> Result<RuntimePacket, RuntimeError> {
    if bytes.len() < COMMON_HEADER_LEN {
        return Err(RuntimeError::UnexpectedMessage(
            "websocket binary message is shorter than an NNRP header",
        ));
    }
    let header = CommonHeader::parse(&bytes[..COMMON_HEADER_LEN])?;
    limits.validate_packet_len(header.packet_len()?)?;
    if header.packet_len()? != bytes.len() {
        return Err(nnrp_core::NnrpError::PacketLengthMismatch {
            declared: header.packet_len()?,
            actual: bytes.len(),
        }
        .into());
    }
    let meta_start = COMMON_HEADER_LEN;
    let meta_end = meta_start + header.meta_len as usize;
    let body_end = meta_end + header.body_len as usize;
    RuntimePacket::from_parts(
        header,
        bytes[meta_start..meta_end].to_vec(),
        bytes[meta_end..body_end].to_vec(),
    )
    .map_err(Into::into)
}

fn runtime_ws(error: WebSocketError) -> RuntimeError {
    RuntimeError::Io(std::io::Error::other(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnrp_core::{
        BackpressureLevel, FrameSubmitMetadata, InputProfile, PartialResultMetadata,
        PayloadKindBitmap, PressureMetadata, ResultClass, ResultPushMetadata, SubmitMode,
        TileIndexMode, STANDARD_PROFILE_TOKEN,
    };
    use nnrp_runtime::{NnrpClientEvent, NnrpResult};
    use nnrp_transport_provider::{RemoteTransportSupport, TransportPolicy};

    #[test]
    fn websocket_endpoint_parses_ws_and_wss_schemes() {
        let ws = "ws://127.0.0.1:8080/nnrp"
            .parse::<WebSocketEndpoint>()
            .unwrap();
        assert_eq!(ws.as_str(), "ws://127.0.0.1:8080/nnrp");
        assert!(!ws.is_secure());
        assert_eq!(ws.to_string(), ws.as_str());

        let wss = WebSocketEndpoint::wss("wss://example.test/nnrp").unwrap();
        assert_eq!(wss.as_str(), "wss://example.test/nnrp");
        assert!(wss.is_secure());

        assert!("http://example.test".parse::<WebSocketEndpoint>().is_err());
        assert!(WebSocketEndpoint::ws("wss://example.test").is_err());
        assert!(WebSocketEndpoint::ws("ws://").is_err());
    }

    #[test]
    fn websocket_provider_registers_and_selects_websocket() {
        let mut registry = TransportProviderRegistry::new();
        register_websocket_provider(&mut registry);
        assert_eq!(registry.providers().len(), 1);
        assert_eq!(registry.providers()[0].name, WebSocketProvider::NAME);
        assert_eq!(registry.providers()[0].transport_id, TransportId::WebSocket);

        let remote = RemoteTransportSupport::new([TransportId::WebSocket]);
        let selection = registry
            .select(&remote, TransportPolicy::ForceWebSocket)
            .expect("websocket provider should satisfy force websocket");
        assert_eq!(selection.selected.name, WebSocketProvider::NAME);
    }

    #[tokio::test]
    async fn websocket_loopback_submits_frame_and_receives_result() -> Result<(), RuntimeError> {
        let server = WebSocketProvider::bind("127.0.0.1:0", NnrpServerConfig::default()).await?;
        let endpoint = WebSocketEndpoint::ws(format!("ws://{}", server.local_addr()?))?;

        let server_task = tokio::spawn(async move {
            let mut session = server.accept().await?;
            let submit = session.receive_submit().await?;
            session
                .send_result(submit.frame_id, token_result(), b"ws-ok".to_vec())
                .await
        });

        let client = WebSocketProvider::connect(&endpoint, NnrpClientConfig::default()).await?;
        let mut session = client.open_session().await?;
        session.submit(token_submit(), b"hello".to_vec()).await?;
        let NnrpResult { body, .. } = session.await_result().await?;
        assert_eq!(body, b"ws-ok");

        server_task
            .await
            .map_err(|_| RuntimeError::Internal("websocket server task panicked"))??;
        Ok(())
    }

    #[tokio::test]
    async fn websocket_rejects_text_messages_as_data_frames() -> Result<(), RuntimeError> {
        let listener = WebSocketFramedListener::bind("127.0.0.1:0").await?;
        let endpoint = WebSocketEndpoint::ws(format!("ws://{}", listener.local_addr()?))?;
        let server_task = tokio::spawn(async move {
            let mut accepted = listener.accept().await?;
            accepted.read_packet().await
        });

        let (mut client, _) = connect_async(endpoint.as_str()).await.map_err(runtime_ws)?;
        client
            .send(Message::Text("not-nnrp".into()))
            .await
            .map_err(runtime_ws)?;

        let error = server_task
            .await
            .map_err(|_| RuntimeError::Internal("websocket text server task panicked"))?
            .expect_err("text messages should be rejected");
        assert!(matches!(error, RuntimeError::UnexpectedMessage(_)));
        Ok(())
    }

    #[tokio::test]
    async fn websocket_loopback_routes_partial_result_and_pressure() -> Result<(), RuntimeError> {
        let server = WebSocketProvider::bind("127.0.0.1:0", NnrpServerConfig::default()).await?;
        let endpoint = WebSocketEndpoint::ws(format!("ws://{}", server.local_addr()?))?;

        let server_task = tokio::spawn(async move {
            let mut session = server.accept().await?;
            let submit = session.receive_submit().await?;
            let credit = session.receive_pressure_update().await?;
            assert_eq!(credit.metadata.credit_window, 9);
            session.send_backpressure(soft_backpressure()).await?;
            session
                .send_partial_result(partial_result(submit.frame_id as u64), b"partial".to_vec())
                .await
        });

        let client = WebSocketProvider::connect(&endpoint, NnrpClientConfig::default()).await?;
        let mut session = client.open_session().await?;
        let frame_id = session
            .submit_nowait(token_submit(), b"partial-request".to_vec())
            .await?;
        session.send_credit_update(credit_update()).await?;

        match session.await_event().await? {
            NnrpClientEvent::Backpressure(pressure) => {
                assert_eq!(pressure.pressure_level, BackpressureLevel::Soft as u16);
                assert_eq!(pressure.credit_window, 2);
            }
            event => panic!("expected backpressure event, got {event:?}"),
        }
        match session.await_event().await? {
            NnrpClientEvent::PartialResult { metadata, body } => {
                assert_eq!(metadata.operation_id, frame_id as u64);
                assert_eq!(metadata.result_sequence, 1);
                assert_eq!(body, b"partial");
            }
            event => panic!("expected partial result event, got {event:?}"),
        }

        server_task
            .await
            .map_err(|_| RuntimeError::Internal("websocket control server task panicked"))??;
        Ok(())
    }

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

    fn partial_result(operation_id: u64) -> PartialResultMetadata {
        PartialResultMetadata {
            operation_id,
            result_sequence: 1,
            object_id: 0,
            delta_sequence: 0,
            body_bytes: 7,
            flags: 0,
        }
    }
}
