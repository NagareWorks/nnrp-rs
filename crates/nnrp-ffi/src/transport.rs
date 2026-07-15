use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "transport-quic")]
use std::net::{IpAddr, Ipv4Addr};
#[cfg(any(feature = "transport-quic", feature = "transport-websocket"))]
use std::net::{SocketAddr, ToSocketAddrs};
#[cfg(any(feature = "transport-ipc", feature = "transport-websocket"))]
use std::str::FromStr;

#[cfg(feature = "transport-websocket")]
use http::{uri::Authority, Uri};
use nnrp_core::{
    CommonHeader, MessageType, TransportId, TransportProbeAckMetadata, TransportProbeMetadata,
    COMMON_HEADER_LEN, TRANSPORT_PROBE_METADATA_LEN,
};
#[cfg(any(
    feature = "transport-tcp",
    feature = "transport-quic",
    feature = "transport-websocket"
))]
use nnrp_runtime::FramedListener;
use nnrp_runtime::{
    BoxedFramedListener, BoxedFramedTransport, FramedTransport, RuntimeError, RuntimeFrameLimits,
    RuntimePacket,
};
use tokio::sync::Mutex as AsyncMutex;

use crate::{
    store_owned_buffer, NnrpBufferView, NnrpErrorFamily, NnrpFfiStatus, NnrpFfiStatusCode,
    NnrpHandle, NnrpHandleKind,
};

const DEFAULT_TIMEOUT_MS: u32 = 30_000;
const DEFAULT_MAX_FRAMES: u32 = 16;
const DEFAULT_MAX_PACKET_BYTES: u64 = 64 * 1024 * 1024;
const BATCH_LENGTH_PREFIX_BYTES: usize = std::mem::size_of::<u32>();
const DEFAULT_PROBE_SAMPLES: u32 = 3;
const MAX_PROBE_SAMPLES: u32 = 32;
const DEFAULT_PROBE_PAYLOAD_BYTES: u32 = 32 * 1024;

type SharedConnection = Arc<AsyncMutex<TransportConnectionState>>;
type SharedListener = Arc<BoxedFramedListener>;

struct TransportConnectionState {
    transport: BoxedFramedTransport,
    pending: VecDeque<RuntimePacket>,
    max_packet_bytes: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NnrpTransportOpenRequest {
    pub transport_id: u32,
    pub flags: u32,
    pub endpoint: NnrpBufferView,
    pub config: NnrpHandle,
    pub max_packet_bytes: u64,
    pub timeout_ms: u32,
    pub reserved0: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpTransportAcceptRequest {
    pub listener: NnrpHandle,
    pub timeout_ms: u32,
    pub reserved0: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NnrpTransportWriteBatchRequest {
    pub connection: NnrpHandle,
    pub frames: *const NnrpBufferView,
    pub frame_count: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpTransportReadBatchRequest {
    pub connection: NnrpHandle,
    pub max_frames: u32,
    pub timeout_ms: u32,
    pub max_bytes: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpTransportFrameBatch {
    pub payload_owner: NnrpHandle,
    pub payload: NnrpBufferView,
    pub frame_count: u32,
    pub reserved0: u32,
}

impl NnrpTransportFrameBatch {
    pub const fn empty() -> Self {
        Self {
            payload_owner: NnrpHandle::invalid(),
            payload: NnrpBufferView::empty(),
            frame_count: 0,
            reserved0: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NnrpTransportProbeRequest {
    pub open: NnrpTransportOpenRequest,
    pub sample_count: u32,
    pub probe_payload_bytes: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpTransportProbeResult {
    pub sample_count: u32,
    pub success_count: u32,
    pub median_throughput_bytes_per_second: u64,
    pub median_rtt_microseconds: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NnrpTransportClientSecurityConfigRequest {
    pub transport_id: u32,
    pub flags: u32,
    pub server_name: NnrpBufferView,
    pub trusted_certificate_der: NnrpBufferView,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NnrpTransportServerSecurityConfigRequest {
    pub transport_id: u32,
    pub flags: u32,
    pub certificate_der: NnrpBufferView,
    pub private_key_pkcs8_der: NnrpBufferView,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
enum TransportSecurityConfig {
    Client {
        transport_id: TransportId,
        server_name: String,
        trusted_certificate_der: Vec<u8>,
    },
    Server {
        transport_id: TransportId,
        certificate_der: Vec<u8>,
        private_key_pkcs8_der: Vec<u8>,
    },
}

#[allow(dead_code)]
enum TransportResource {
    Connection(SharedConnection),
    Listener {
        listener: SharedListener,
        endpoint: String,
        max_packet_bytes: usize,
    },
    SecurityConfig(TransportSecurityConfig),
}

#[derive(Default)]
struct TransportStore {
    slots: BTreeMap<(u32, u64), TransportSlot>,
    reusable: BTreeMap<u32, Vec<u64>>,
}

struct TransportSlot {
    generation: u32,
    resource: Option<TransportResource>,
}

impl TransportStore {
    fn insert(&mut self, kind: NnrpHandleKind, resource: TransportResource) -> NnrpHandle {
        let kind_value = kind as u32;
        let reusable_id = self.reusable.get_mut(&kind_value).and_then(Vec::pop);
        if let Some(id) = reusable_id {
            let slot = self
                .slots
                .get_mut(&(kind_value, id))
                .expect("reusable transport slot must exist");
            slot.generation = slot
                .generation
                .checked_add(1)
                .expect("exhausted transport slots are not reusable");
            slot.resource = Some(resource);
            return NnrpHandle::new(kind, id, slot.generation);
        }

        let handle = next_handle(kind);
        self.slots.insert(
            (handle.kind, handle.id),
            TransportSlot {
                generation: handle.generation,
                resource: Some(resource),
            },
        );
        handle
    }

    fn get(&self, handle: NnrpHandle) -> Option<&TransportResource> {
        let slot = self.slots.get(&(handle.kind, handle.id))?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.resource.as_ref()
    }

    fn close(&mut self, handle: NnrpHandle) -> Result<Option<TransportResource>, NnrpFfiStatus> {
        let (resource, reusable) = {
            let slot = self
                .slots
                .get_mut(&(handle.kind, handle.id))
                .ok_or_else(|| NnrpFfiStatus::invalid_handle(handle.kind))?;
            if handle.generation < slot.generation {
                return Ok(None);
            }
            if handle.generation != slot.generation {
                return Err(NnrpFfiStatus::invalid_handle(handle.kind));
            }
            let resource = slot.resource.take();
            let reusable = resource.is_some() && slot.generation < u32::MAX;
            (resource, reusable)
        };
        if reusable {
            self.reusable
                .entry(handle.kind)
                .or_default()
                .push(handle.id);
        }
        Ok(resource)
    }
}

static TRANSPORT_STORE: OnceLock<Mutex<TransportStore>> = OnceLock::new();
static NEXT_TRANSPORT_HANDLE: AtomicU64 = AtomicU64::new(1);
static TRANSPORT_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn transport_store() -> &'static Mutex<TransportStore> {
    TRANSPORT_STORE.get_or_init(|| Mutex::new(TransportStore::default()))
}

fn transport_runtime() -> &'static tokio::runtime::Runtime {
    TRANSPORT_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("nnrp-ffi-transport")
            .enable_all()
            .build()
            .expect("transport runtime must initialize")
    })
}

fn run_async<F, T>(future: F, timeout_ms: u32) -> Result<T, NnrpFfiStatus>
where
    F: Future<Output = Result<T, RuntimeError>> + Send + 'static,
    T: Send + 'static,
{
    run_async_mapped(future, timeout_ms, status_from_runtime_error)
}

fn run_async_mapped<F, T, E, M>(
    future: F,
    timeout_ms: u32,
    map_error: M,
) -> Result<T, NnrpFfiStatus>
where
    F: Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static,
    E: Send + 'static,
    M: FnOnce(E) -> NnrpFfiStatus + Send + 'static,
{
    let timeout = Duration::from_millis(effective_timeout(timeout_ms) as u64);
    let (sender, receiver) = mpsc::sync_channel(1);
    transport_runtime().spawn(async move {
        let result = match tokio::time::timeout(timeout, future).await {
            Ok(result) => result.map_err(map_error),
            Err(_) => Err(transport_status(NnrpFfiStatusCode::WouldBlock, 1)),
        };
        let _ = sender.send(result);
    });
    receiver
        .recv()
        .map_err(|_| transport_status(NnrpFfiStatusCode::InternalError, 1))?
}

fn effective_timeout(timeout_ms: u32) -> u32 {
    if timeout_ms == 0 {
        DEFAULT_TIMEOUT_MS
    } else {
        timeout_ms
    }
}

fn effective_packet_limit(max_packet_bytes: u64) -> Result<usize, NnrpFfiStatus> {
    let value = if max_packet_bytes == 0 {
        DEFAULT_MAX_PACKET_BYTES
    } else {
        max_packet_bytes
    };
    usize::try_from(value).map_err(|_| NnrpFfiStatus::invalid_argument(103))
}

fn transport_status(code: NnrpFfiStatusCode, detail_code: u32) -> NnrpFfiStatus {
    NnrpFfiStatus {
        status_code: code as u32,
        error_family: NnrpErrorFamily::Transport as u32,
        protocol_error_code: 0,
        detail_code,
    }
}

fn status_from_runtime_error(error: RuntimeError) -> NnrpFfiStatus {
    match error {
        RuntimeError::Protocol(error) => NnrpFfiStatus::from_core_error(&error),
        RuntimeError::UnsupportedTransport(_) | RuntimeError::FrameTooLarge { .. } => {
            transport_status(NnrpFfiStatusCode::InvalidArgument, 104)
        }
        RuntimeError::TransportClosed { .. } | RuntimeError::UnexpectedMessage(_) => {
            transport_status(NnrpFfiStatusCode::InvalidState, 105)
        }
        RuntimeError::Io(_) | RuntimeError::FrameIdOverflow | RuntimeError::Internal(_) => {
            transport_status(NnrpFfiStatusCode::InternalError, 106)
        }
    }
}

fn parse_transport_id(value: u32) -> Result<TransportId, NnrpFfiStatus> {
    let transport =
        TransportId::try_from_u32(value).map_err(|_| NnrpFfiStatus::invalid_argument(107))?;
    if transport == TransportId::Unspecified {
        return Err(NnrpFfiStatus::invalid_argument(107));
    }
    Ok(transport)
}

fn transport_is_linked(transport: TransportId) -> bool {
    match transport {
        #[cfg(feature = "transport-tcp")]
        TransportId::Tcp => true,
        #[cfg(feature = "transport-quic")]
        TransportId::Quic => true,
        #[cfg(feature = "transport-ipc")]
        TransportId::Ipc => true,
        #[cfg(feature = "transport-websocket")]
        TransportId::WebSocket => true,
        _ => false,
    }
}

unsafe fn copied_bytes(view: NnrpBufferView, detail: u32) -> Result<Vec<u8>, NnrpFfiStatus> {
    view.validate()?;
    if view.len == 0 {
        return Ok(Vec::new());
    }
    if view.ptr.is_null() {
        return Err(NnrpFfiStatus::invalid_argument(detail));
    }
    Ok(std::slice::from_raw_parts(view.ptr, view.len).to_vec())
}

unsafe fn copied_utf8(view: NnrpBufferView, detail: u32) -> Result<String, NnrpFfiStatus> {
    let bytes = copied_bytes(view, detail)?;
    String::from_utf8(bytes).map_err(|_| NnrpFfiStatus::invalid_argument(detail))
}

fn next_handle(kind: NnrpHandleKind) -> NnrpHandle {
    NnrpHandle::new(
        kind,
        NEXT_TRANSPORT_HANDLE.fetch_add(1, Ordering::Relaxed),
        1,
    )
}

fn insert_resource(kind: NnrpHandleKind, resource: TransportResource) -> NnrpHandle {
    let mut store = transport_store()
        .lock()
        .expect("transport store lock should not be poisoned");
    store.insert(kind, resource)
}

fn get_connection(handle: NnrpHandle) -> Result<SharedConnection, NnrpFfiStatus> {
    handle.validate_kind(NnrpHandleKind::TransportConnection)?;
    let store = transport_store()
        .lock()
        .map_err(|_| transport_status(NnrpFfiStatusCode::InternalError, 108))?;
    match store.get(handle) {
        Some(TransportResource::Connection(connection)) => Ok(Arc::clone(connection)),
        _ => Err(NnrpFfiStatus::invalid_handle(
            NnrpHandleKind::TransportConnection as u32,
        )),
    }
}

fn get_listener(handle: NnrpHandle) -> Result<(SharedListener, usize), NnrpFfiStatus> {
    handle.validate_kind(NnrpHandleKind::TransportListener)?;
    let store = transport_store()
        .lock()
        .map_err(|_| transport_status(NnrpFfiStatusCode::InternalError, 109))?;
    match store.get(handle) {
        Some(TransportResource::Listener {
            listener,
            max_packet_bytes,
            ..
        }) => Ok((Arc::clone(listener), *max_packet_bytes)),
        _ => Err(NnrpFfiStatus::invalid_handle(
            NnrpHandleKind::TransportListener as u32,
        )),
    }
}

fn get_listener_endpoint(handle: NnrpHandle) -> Result<String, NnrpFfiStatus> {
    handle.validate_kind(NnrpHandleKind::TransportListener)?;
    let store = transport_store()
        .lock()
        .map_err(|_| transport_status(NnrpFfiStatusCode::InternalError, 109))?;
    match store.get(handle) {
        Some(TransportResource::Listener { endpoint, .. }) => Ok(endpoint.clone()),
        _ => Err(NnrpFfiStatus::invalid_handle(
            NnrpHandleKind::TransportListener as u32,
        )),
    }
}

#[cfg(any(feature = "transport-quic", feature = "transport-websocket"))]
fn get_security_config(handle: NnrpHandle) -> Result<TransportSecurityConfig, NnrpFfiStatus> {
    handle.validate_kind(NnrpHandleKind::TransportSecurityConfig)?;
    let store = transport_store()
        .lock()
        .map_err(|_| transport_status(NnrpFfiStatusCode::InternalError, 110))?;
    match store.get(handle) {
        Some(TransportResource::SecurityConfig(config)) => Ok(config.clone()),
        _ => Err(NnrpFfiStatus::invalid_handle(
            NnrpHandleKind::TransportSecurityConfig as u32,
        )),
    }
}

fn packet_from_bytes(bytes: &[u8], limit: usize) -> Result<RuntimePacket, RuntimeError> {
    if bytes.len() < COMMON_HEADER_LEN {
        return Err(RuntimeError::Protocol(
            nnrp_core::NnrpError::SourceTooShort {
                expected: COMMON_HEADER_LEN,
                actual: bytes.len(),
            },
        ));
    }
    if bytes.len() > limit {
        return Err(RuntimeError::FrameTooLarge {
            declared: bytes.len(),
            max: limit,
        });
    }
    let header = CommonHeader::parse(&bytes[..COMMON_HEADER_LEN])?;
    let packet_len = header.packet_len()?;
    if packet_len != bytes.len() {
        return Err(RuntimeError::Protocol(
            nnrp_core::NnrpError::PacketLengthMismatch {
                declared: packet_len,
                actual: bytes.len(),
            },
        ));
    }
    let metadata_end = COMMON_HEADER_LEN + header.meta_len as usize;
    RuntimePacket::from_parts(
        header,
        bytes[COMMON_HEADER_LEN..metadata_end].to_vec(),
        bytes[metadata_end..].to_vec(),
    )
    .map_err(Into::into)
}

fn encode_batch(packets: &[Vec<u8>]) -> Result<Vec<u8>, NnrpFfiStatus> {
    let mut encoded = Vec::new();
    for packet in packets {
        let len = u32::try_from(packet.len()).map_err(|_| NnrpFfiStatus::invalid_argument(111))?;
        encoded.extend_from_slice(&len.to_le_bytes());
        encoded.extend_from_slice(packet);
    }
    Ok(encoded)
}

struct ProbeAwareTransport {
    inner: BoxedFramedTransport,
}

impl ProbeAwareTransport {
    fn new(inner: BoxedFramedTransport) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl FramedTransport for ProbeAwareTransport {
    fn transport_kind(&self) -> nnrp_runtime::RuntimeTransportKind {
        self.inner.transport_kind()
    }

    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
        loop {
            let packet = self.inner.read_packet().await?;
            if packet.header.message_type != MessageType::TransportProbe {
                return Ok(packet);
            }
            let probe = TransportProbeMetadata::parse(&packet.metadata)?;
            if packet.body.len() != probe.probe_payload_bytes as usize {
                return Err(RuntimeError::Protocol(
                    nnrp_core::NnrpError::DeclaredLengthMismatch {
                        field: "transport_probe.probe_payload_bytes",
                        declared: probe.probe_payload_bytes as usize,
                        actual: packet.body.len(),
                    },
                ));
            }
            let ack = TransportProbeAckMetadata {
                probe_id: probe.probe_id,
                server_recv_ts_us: unix_time_us(),
            };
            let ack_packet = RuntimePacket::new(
                CommonHeader::new(MessageType::TransportProbeAck, 0, 0),
                ack.to_bytes()?.to_vec(),
                Vec::new(),
            )?;
            self.inner.write_packet(&ack_packet).await?;
        }
    }

    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
        self.inner.write_packet(packet).await
    }

    async fn close(&mut self) -> Result<(), RuntimeError> {
        self.inner.close().await
    }
}

fn unix_time_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn wrap_connection(connection: BoxedFramedTransport, max_packet_bytes: usize) -> SharedConnection {
    Arc::new(AsyncMutex::new(TransportConnectionState {
        transport: Box::new(ProbeAwareTransport::new(connection)),
        pending: VecDeque::new(),
        max_packet_bytes,
    }))
}

#[cfg(any(
    feature = "transport-tcp",
    feature = "transport-ipc",
    feature = "transport-websocket"
))]
fn require_invalid_config(handle: NnrpHandle) -> Result<(), NnrpFfiStatus> {
    if handle == NnrpHandle::invalid() {
        Ok(())
    } else {
        Err(NnrpFfiStatus::invalid_argument(112))
    }
}

#[cfg(any(feature = "transport-tcp", feature = "transport-quic"))]
fn socket_endpoint(endpoint: &str, scheme: &str) -> Result<String, RuntimeError> {
    endpoint
        .strip_prefix(scheme)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or(RuntimeError::UnsupportedTransport(
            "transport endpoint scheme does not match artifact",
        ))
}

#[cfg(feature = "transport-websocket")]
fn websocket_listen_addr(endpoint: &str) -> Result<SocketAddr, RuntimeError> {
    let uri = endpoint.parse::<Uri>().map_err(|_| {
        RuntimeError::UnsupportedTransport("websocket listener endpoint is invalid")
    })?;
    if uri.scheme_str() != Some("ws") {
        return Err(RuntimeError::UnsupportedTransport(
            "plain websocket listener requires ws://",
        ));
    }
    let host = uri.host().ok_or(RuntimeError::UnsupportedTransport(
        "websocket listener host is missing",
    ))?;
    let port = uri.port_u16().unwrap_or(80);
    (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or(RuntimeError::UnsupportedTransport(
            "websocket listener endpoint did not resolve",
        ))
}

#[cfg(feature = "transport-quic")]
fn resolve_socket(value: &str) -> Result<SocketAddr, RuntimeError> {
    value
        .to_socket_addrs()?
        .next()
        .ok_or(RuntimeError::UnsupportedTransport(
            "transport endpoint did not resolve",
        ))
}

async fn connect_transport(
    transport: TransportId,
    endpoint: String,
    config: NnrpHandle,
    limit: RuntimeFrameLimits,
) -> Result<BoxedFramedTransport, RuntimeError> {
    match transport {
        #[cfg(feature = "transport-tcp")]
        TransportId::Tcp => {
            require_invalid_config(config).map_err(|_| {
                RuntimeError::UnsupportedTransport("TCP does not accept a security config")
            })?;
            let addr = socket_endpoint(&endpoint, "tcp://")?;
            Ok(Box::new(
                nnrp_runtime::TcpTransport::connect_with_limits(addr, limit).await?,
            ))
        }
        #[cfg(feature = "transport-ipc")]
        TransportId::Ipc => {
            require_invalid_config(config).map_err(|_| {
                RuntimeError::UnsupportedTransport("IPC does not accept a security config")
            })?;
            let endpoint = nnrp_transport_ipc::IpcEndpoint::from_str(&endpoint)?;
            Ok(Box::new(
                nnrp_transport_ipc::IpcTransport::connect_with_limits(&endpoint, limit).await?,
            ))
        }
        #[cfg(feature = "transport-websocket")]
        TransportId::WebSocket => {
            let endpoint = nnrp_transport_websocket::WebSocketEndpoint::from_str(&endpoint)?;
            if endpoint.is_secure() {
                return connect_secure_websocket(endpoint, config, limit).await;
            }
            require_invalid_config(config).map_err(|_| {
                RuntimeError::UnsupportedTransport("ws:// does not accept a security config")
            })?;
            Ok(Box::new(
                nnrp_transport_websocket::WebSocketTransport::connect_with_limits(&endpoint, limit)
                    .await?,
            ))
        }
        #[cfg(feature = "transport-quic")]
        TransportId::Quic => connect_quic(endpoint, config, limit).await,
        _ => Err(RuntimeError::UnsupportedTransport(
            "transport implementation is not linked into this artifact",
        )),
    }
}

async fn listen_transport(
    transport: TransportId,
    endpoint: String,
    config: NnrpHandle,
    limit: RuntimeFrameLimits,
) -> Result<(BoxedFramedListener, String), RuntimeError> {
    match transport {
        #[cfg(feature = "transport-tcp")]
        TransportId::Tcp => {
            require_invalid_config(config).map_err(|_| {
                RuntimeError::UnsupportedTransport("TCP does not accept a security config")
            })?;
            let addr = socket_endpoint(&endpoint, "tcp://")?;
            let listener = nnrp_runtime::TcpFramedListener::bind_with_limits(addr, limit).await?;
            let endpoint = format!("tcp://{}", listener.local_addr()?);
            Ok((Box::new(listener), endpoint))
        }
        #[cfg(feature = "transport-ipc")]
        TransportId::Ipc => {
            require_invalid_config(config).map_err(|_| {
                RuntimeError::UnsupportedTransport("IPC does not accept a security config")
            })?;
            let endpoint = nnrp_transport_ipc::IpcEndpoint::from_str(&endpoint)?;
            let listener =
                nnrp_transport_ipc::IpcFramedListener::bind_with_limits(&endpoint, limit).await?;
            Ok((Box::new(listener), endpoint.to_string()))
        }
        #[cfg(feature = "transport-websocket")]
        TransportId::WebSocket => {
            if endpoint.starts_with("wss://") {
                return listen_secure_websocket(endpoint, config, limit).await;
            }
            require_invalid_config(config).map_err(|_| {
                RuntimeError::UnsupportedTransport("ws:// does not accept a security config")
            })?;
            let addr = websocket_listen_addr(&endpoint)?;
            let listener =
                nnrp_transport_websocket::WebSocketFramedListener::bind_with_limits(addr, limit)
                    .await?;
            let endpoint = normalized_websocket_endpoint(&endpoint, listener.local_addr()?)?;
            Ok((Box::new(listener), endpoint))
        }
        #[cfg(feature = "transport-quic")]
        TransportId::Quic => listen_quic(endpoint, config, limit).await,
        _ => Err(RuntimeError::UnsupportedTransport(
            "transport implementation is not linked into this artifact",
        )),
    }
}

#[cfg(feature = "transport-quic")]
async fn connect_quic(
    endpoint: String,
    config: NnrpHandle,
    limit: RuntimeFrameLimits,
) -> Result<BoxedFramedTransport, RuntimeError> {
    let addr = resolve_socket(&socket_endpoint(&endpoint, "quic://")?)?;
    let config = get_security_config(config)
        .map_err(|_| RuntimeError::UnsupportedTransport("QUIC client config is required"))?;
    let TransportSecurityConfig::Client {
        transport_id,
        server_name,
        trusted_certificate_der,
    } = config
    else {
        return Err(RuntimeError::UnsupportedTransport(
            "QUIC requires a client security config",
        ));
    };
    if transport_id != TransportId::Quic {
        return Err(RuntimeError::UnsupportedTransport(
            "security config transport does not match QUIC",
        ));
    }
    let endpoint_config = nnrp_transport_quic::QuicClientEndpointConfig::with_root_certificate(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        server_name,
        trusted_certificate_der,
    );
    Ok(Box::new(
        nnrp_transport_quic::QuicTransport::connect_with_limits(addr, &endpoint_config, limit)
            .await?,
    ))
}

#[cfg(feature = "transport-quic")]
async fn listen_quic(
    endpoint: String,
    config: NnrpHandle,
    limit: RuntimeFrameLimits,
) -> Result<(BoxedFramedListener, String), RuntimeError> {
    let addr = resolve_socket(&socket_endpoint(&endpoint, "quic://")?)?;
    let config = get_security_config(config)
        .map_err(|_| RuntimeError::UnsupportedTransport("QUIC server config is required"))?;
    let TransportSecurityConfig::Server {
        transport_id,
        certificate_der,
        private_key_pkcs8_der,
    } = config
    else {
        return Err(RuntimeError::UnsupportedTransport(
            "QUIC requires a server security config",
        ));
    };
    if transport_id != TransportId::Quic {
        return Err(RuntimeError::UnsupportedTransport(
            "security config transport does not match QUIC",
        ));
    }
    let endpoint_config = nnrp_transport_quic::QuicServerEndpointConfig::with_single_certificate(
        addr,
        certificate_der,
        private_key_pkcs8_der,
    );
    let listener =
        nnrp_transport_quic::QuicFramedListener::bind_with_limits(&endpoint_config, limit)?;
    let endpoint = format!("quic://{}", listener.local_addr()?);
    Ok((Box::new(listener), endpoint))
}

#[cfg(feature = "transport-websocket")]
async fn connect_secure_websocket(
    endpoint: nnrp_transport_websocket::WebSocketEndpoint,
    config: NnrpHandle,
    limit: RuntimeFrameLimits,
) -> Result<BoxedFramedTransport, RuntimeError> {
    let config = get_security_config(config)
        .map_err(|_| RuntimeError::UnsupportedTransport("WSS client config is required"))?;
    let TransportSecurityConfig::Client {
        transport_id,
        server_name,
        trusted_certificate_der,
    } = config
    else {
        return Err(RuntimeError::UnsupportedTransport(
            "WSS requires a client security config",
        ));
    };
    if transport_id != TransportId::WebSocket {
        return Err(RuntimeError::UnsupportedTransport(
            "security config transport does not match WebSocket",
        ));
    }
    Ok(Box::new(
        nnrp_transport_websocket::WebSocketTransport::connect_secure_with_limits(
            &endpoint,
            &server_name,
            trusted_certificate_der,
            limit,
        )
        .await?,
    ))
}

#[cfg(feature = "transport-websocket")]
async fn listen_secure_websocket(
    endpoint: String,
    config: NnrpHandle,
    limit: RuntimeFrameLimits,
) -> Result<(BoxedFramedListener, String), RuntimeError> {
    let config = get_security_config(config)
        .map_err(|_| RuntimeError::UnsupportedTransport("WSS server config is required"))?;
    let TransportSecurityConfig::Server {
        transport_id,
        certificate_der,
        private_key_pkcs8_der,
    } = config
    else {
        return Err(RuntimeError::UnsupportedTransport(
            "WSS requires a server security config",
        ));
    };
    if transport_id != TransportId::WebSocket {
        return Err(RuntimeError::UnsupportedTransport(
            "security config transport does not match WebSocket",
        ));
    }
    let addr = secure_websocket_listen_addr(&endpoint)?;
    let listener = nnrp_transport_websocket::WebSocketFramedListener::bind_secure_with_limits(
        addr,
        certificate_der,
        private_key_pkcs8_der,
        limit,
    )
    .await?;
    let endpoint = normalized_websocket_endpoint(&endpoint, listener.local_addr()?)?;
    Ok((Box::new(listener), endpoint))
}

#[cfg(feature = "transport-websocket")]
fn secure_websocket_listen_addr(endpoint: &str) -> Result<SocketAddr, RuntimeError> {
    let uri = endpoint.parse::<Uri>().map_err(|_| {
        RuntimeError::UnsupportedTransport("websocket listener endpoint is invalid")
    })?;
    if uri.scheme_str() != Some("wss") {
        return Err(RuntimeError::UnsupportedTransport(
            "secure websocket listener requires wss://",
        ));
    }
    let host = uri.host().ok_or(RuntimeError::UnsupportedTransport(
        "websocket listener host is missing",
    ))?;
    let port = uri.port_u16().unwrap_or(443);
    (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or(RuntimeError::UnsupportedTransport(
            "websocket listener endpoint did not resolve",
        ))
}

#[cfg(feature = "transport-websocket")]
fn normalized_websocket_endpoint(
    endpoint: &str,
    local_addr: SocketAddr,
) -> Result<String, RuntimeError> {
    let uri = endpoint.parse::<Uri>().map_err(|_| {
        RuntimeError::UnsupportedTransport("websocket listener endpoint is invalid")
    })?;
    let host = uri.host().ok_or(RuntimeError::UnsupportedTransport(
        "websocket listener host is missing",
    ))?;
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    let mut parts = uri.into_parts();
    parts.authority = Some(
        format!("{host}:{}", local_addr.port())
            .parse::<Authority>()
            .map_err(|_| {
                RuntimeError::UnsupportedTransport("websocket listener port is invalid")
            })?,
    );
    Uri::from_parts(parts)
        .map(|uri| uri.to_string())
        .map_err(|_| RuntimeError::UnsupportedTransport("websocket listener endpoint is invalid"))
}

/// Creates an immutable client transport security configuration.
///
/// # Safety
///
/// Every non-empty buffer in `request` must remain readable for the duration of the call, and
/// `out_config` must be non-null, aligned, and writable for one [`NnrpHandle`].
pub(super) unsafe fn transport_client_security_config_create(
    request: NnrpTransportClientSecurityConfigRequest,
    out_config: *mut NnrpHandle,
) -> NnrpFfiStatus {
    if out_config.is_null() || request.flags != 0 {
        return NnrpFfiStatus::invalid_argument(113);
    }
    let result = (|| {
        let transport_id = parse_transport_id(request.transport_id)?;
        if !transport_is_linked(transport_id)
            || !matches!(transport_id, TransportId::Quic | TransportId::WebSocket)
        {
            return Err(NnrpFfiStatus::invalid_argument(114));
        }
        let server_name = copied_utf8(request.server_name, 115)?;
        let trusted_certificate_der = copied_bytes(request.trusted_certificate_der, 116)?;
        if server_name.is_empty() || trusted_certificate_der.is_empty() {
            return Err(NnrpFfiStatus::invalid_argument(117));
        }
        Ok(insert_resource(
            NnrpHandleKind::TransportSecurityConfig,
            TransportResource::SecurityConfig(TransportSecurityConfig::Client {
                transport_id,
                server_name,
                trusted_certificate_der,
            }),
        ))
    })();
    match result {
        Ok(handle) => {
            *out_config = handle;
            NnrpFfiStatus::ok()
        }
        Err(status) => status,
    }
}

/// Creates an immutable server transport security configuration.
///
/// # Safety
///
/// Every non-empty buffer in `request` must remain readable for the duration of the call, and
/// `out_config` must be non-null, aligned, and writable for one [`NnrpHandle`].
pub(super) unsafe fn transport_server_security_config_create(
    request: NnrpTransportServerSecurityConfigRequest,
    out_config: *mut NnrpHandle,
) -> NnrpFfiStatus {
    if out_config.is_null() || request.flags != 0 {
        return NnrpFfiStatus::invalid_argument(118);
    }
    let result = (|| {
        let transport_id = parse_transport_id(request.transport_id)?;
        if !transport_is_linked(transport_id)
            || !matches!(transport_id, TransportId::Quic | TransportId::WebSocket)
        {
            return Err(NnrpFfiStatus::invalid_argument(119));
        }
        let certificate_der = copied_bytes(request.certificate_der, 120)?;
        let private_key_pkcs8_der = copied_bytes(request.private_key_pkcs8_der, 121)?;
        if certificate_der.is_empty() || private_key_pkcs8_der.is_empty() {
            return Err(NnrpFfiStatus::invalid_argument(122));
        }
        Ok(insert_resource(
            NnrpHandleKind::TransportSecurityConfig,
            TransportResource::SecurityConfig(TransportSecurityConfig::Server {
                transport_id,
                certificate_der,
                private_key_pkcs8_der,
            }),
        ))
    })();
    match result {
        Ok(handle) => {
            *out_config = handle;
            NnrpFfiStatus::ok()
        }
        Err(status) => status,
    }
}

unsafe fn validated_open_request(
    request: NnrpTransportOpenRequest,
) -> Result<(TransportId, String, RuntimeFrameLimits, u32), NnrpFfiStatus> {
    if request.flags != 0 || request.reserved0 != 0 {
        return Err(NnrpFfiStatus::invalid_argument(123));
    }
    let transport = parse_transport_id(request.transport_id)?;
    if !transport_is_linked(transport) {
        return Err(NnrpFfiStatus::invalid_argument(107));
    }
    let endpoint = copied_utf8(request.endpoint, 124)?;
    if endpoint.is_empty() {
        return Err(NnrpFfiStatus::invalid_argument(124));
    }
    let limit = RuntimeFrameLimits::new(effective_packet_limit(request.max_packet_bytes)?);
    Ok((
        transport,
        endpoint,
        limit,
        effective_timeout(request.timeout_ms),
    ))
}

/// Establishes a transport connection using the implementation linked into this artifact.
///
/// # Safety
///
/// The endpoint buffer in `request` must remain readable for the duration of the call, and
/// `out_connection` must be non-null, aligned, and writable for one [`NnrpHandle`].
pub(super) unsafe fn transport_connect(
    request: NnrpTransportOpenRequest,
    out_connection: *mut NnrpHandle,
) -> NnrpFfiStatus {
    if out_connection.is_null() {
        return NnrpFfiStatus::invalid_argument(125);
    }
    let (transport, endpoint, limit, timeout) = match validated_open_request(request) {
        Ok(value) => value,
        Err(status) => return status,
    };
    let connection = match run_async(
        connect_transport(transport, endpoint, request.config, limit),
        timeout,
    ) {
        Ok(connection) => connection,
        Err(status) => return status,
    };
    *out_connection = insert_resource(
        NnrpHandleKind::TransportConnection,
        TransportResource::Connection(wrap_connection(connection, limit.max_packet_bytes)),
    );
    NnrpFfiStatus::ok()
}

/// Binds a transport listener using the implementation linked into this artifact.
///
/// # Safety
///
/// The endpoint buffer in `request` must remain readable for the duration of the call, and
/// `out_listener` must be non-null, aligned, and writable for one [`NnrpHandle`].
pub(super) unsafe fn transport_listen(
    request: NnrpTransportOpenRequest,
    out_listener: *mut NnrpHandle,
) -> NnrpFfiStatus {
    if out_listener.is_null() {
        return NnrpFfiStatus::invalid_argument(126);
    }
    let (transport, endpoint, limit, timeout) = match validated_open_request(request) {
        Ok(value) => value,
        Err(status) => return status,
    };
    let (listener, endpoint) = match run_async(
        listen_transport(transport, endpoint, request.config, limit),
        timeout,
    ) {
        Ok(listener) => listener,
        Err(status) => return status,
    };
    *out_listener = insert_resource(
        NnrpHandleKind::TransportListener,
        TransportResource::Listener {
            listener: Arc::new(listener),
            endpoint,
            max_packet_bytes: limit.max_packet_bytes,
        },
    );
    NnrpFfiStatus::ok()
}

/// Copies the normalized endpoint of a bound listener into a native-owned buffer.
///
/// # Safety
///
/// `out_buffer` and `out_endpoint` must be non-null, correctly aligned, and writable for one
/// value of their respective types. The returned owner must be released with `nnrp_buffer_release`.
pub(super) unsafe fn transport_listener_endpoint(
    listener: NnrpHandle,
    out_buffer: *mut NnrpHandle,
    out_endpoint: *mut NnrpBufferView,
) -> NnrpFfiStatus {
    if out_buffer.is_null() || out_endpoint.is_null() {
        return NnrpFfiStatus::invalid_argument(135);
    }
    let endpoint = match get_listener_endpoint(listener) {
        Ok(endpoint) => endpoint,
        Err(status) => return status,
    };
    match store_owned_buffer(endpoint.into_bytes()) {
        Ok((owner, view)) => {
            *out_buffer = owner;
            *out_endpoint = view;
            NnrpFfiStatus::ok()
        }
        Err(status) => status,
    }
}

/// Accepts one connection from a transport listener.
///
/// # Safety
///
/// `out_connection` must be non-null, aligned, and writable for one [`NnrpHandle`]. The listener
/// handle must not be closed concurrently with this call.
pub(super) unsafe fn transport_accept(
    request: NnrpTransportAcceptRequest,
    out_connection: *mut NnrpHandle,
) -> NnrpFfiStatus {
    if out_connection.is_null() || request.reserved0 != 0 {
        return NnrpFfiStatus::invalid_argument(127);
    }
    let (listener, max_packet_bytes) = match get_listener(request.listener) {
        Ok(listener) => listener,
        Err(status) => return status,
    };
    let connection = match run_async(async move { listener.accept().await }, request.timeout_ms) {
        Ok(connection) => connection,
        Err(status) => return status,
    };
    *out_connection = insert_resource(
        NnrpHandleKind::TransportConnection,
        TransportResource::Connection(wrap_connection(connection, max_packet_bytes)),
    );
    NnrpFfiStatus::ok()
}

/// Writes a batch of complete NNRP packets to a transport connection.
///
/// # Safety
///
/// `request.frames` must point to `frame_count` readable views, and every non-empty frame buffer
/// must remain readable for the duration of the call. The connection must not be closed
/// concurrently with this call.
pub(super) unsafe fn transport_write_batch(
    request: NnrpTransportWriteBatchRequest,
) -> NnrpFfiStatus {
    if request.flags != 0 || request.frame_count == 0 || request.frames.is_null() {
        return NnrpFfiStatus::invalid_argument(128);
    }
    let connection = match get_connection(request.connection) {
        Ok(connection) => connection,
        Err(status) => return status,
    };
    let views = std::slice::from_raw_parts(request.frames, request.frame_count as usize);
    let mut frame_bytes = Vec::with_capacity(views.len());
    for (index, view) in views.iter().copied().enumerate() {
        let bytes = match copied_bytes(view, 129) {
            Ok(bytes) => bytes,
            Err(mut status) => {
                status.detail_code = index as u32;
                return status;
            }
        };
        frame_bytes.push(bytes);
    }
    match run_async_mapped(
        async move {
            let mut connection = connection.lock().await;
            for (index, bytes) in frame_bytes.into_iter().enumerate() {
                let packet = packet_from_bytes(&bytes, connection.max_packet_bytes)
                    .map_err(|error| (index, error))?;
                connection
                    .transport
                    .write_packet(&packet)
                    .await
                    .map_err(|error| (index, error))?;
            }
            Ok(())
        },
        DEFAULT_TIMEOUT_MS,
        |(index, error)| {
            let mut status = status_from_runtime_error(error);
            status.detail_code = index as u32;
            status
        },
    ) {
        Ok(()) => NnrpFfiStatus::ok(),
        Err(status) => status,
    }
}

/// Reads a batch of complete NNRP packets into a native-owned buffer.
///
/// # Safety
///
/// `out_batch` must be non-null, aligned, and writable for one [`NnrpTransportFrameBatch`]. The
/// connection must not be closed concurrently with this call. A successful payload owner must be
/// released with `nnrp_buffer_release`.
pub(super) unsafe fn transport_read_batch(
    request: NnrpTransportReadBatchRequest,
    out_batch: *mut NnrpTransportFrameBatch,
) -> NnrpFfiStatus {
    if out_batch.is_null() {
        return NnrpFfiStatus::invalid_argument(130);
    }
    *out_batch = NnrpTransportFrameBatch::empty();
    let connection = match get_connection(request.connection) {
        Ok(connection) => connection,
        Err(status) => return status,
    };
    let max_frames = if request.max_frames == 0 {
        DEFAULT_MAX_FRAMES
    } else {
        request.max_frames
    } as usize;
    let max_bytes = match effective_packet_limit(request.max_bytes) {
        Ok(value) => value,
        Err(status) => return status,
    };
    let packets = match run_async(
        async move {
            let mut connection = connection.lock().await;
            let first = match connection.pending.pop_front() {
                Some(packet) => packet,
                None => connection.transport.read_packet().await?,
            };
            let first_bytes = first.to_bytes()?;
            let first_encoded_len = first_bytes
                .len()
                .checked_add(BATCH_LENGTH_PREFIX_BYTES)
                .ok_or(RuntimeError::FrameTooLarge {
                    declared: usize::MAX,
                    max: max_bytes,
                })?;
            if first_encoded_len > max_bytes {
                connection.pending.push_front(first);
                return Err(RuntimeError::FrameTooLarge {
                    declared: first_encoded_len,
                    max: max_bytes,
                });
            }
            let mut bytes = vec![first_bytes];
            let mut total = first_encoded_len;
            while bytes.len() < max_frames && total < max_bytes {
                let next = match connection.pending.pop_front() {
                    Some(packet) => Ok(Ok(packet)),
                    None => {
                        tokio::time::timeout(
                            Duration::from_millis(1),
                            connection.transport.read_packet(),
                        )
                        .await
                    }
                };
                match next {
                    Ok(Ok(packet)) => {
                        let packet_bytes = packet.to_bytes()?;
                        let packet_encoded_len = packet_bytes
                            .len()
                            .checked_add(BATCH_LENGTH_PREFIX_BYTES)
                            .ok_or(RuntimeError::FrameTooLarge {
                                declared: usize::MAX,
                                max: max_bytes,
                            })?;
                        if total
                            .checked_add(packet_encoded_len)
                            .is_none_or(|next_total| next_total > max_bytes)
                        {
                            connection.pending.push_front(packet);
                            break;
                        }
                        total += packet_encoded_len;
                        bytes.push(packet_bytes);
                    }
                    Ok(Err(error)) => return Err(error),
                    Err(_) => break,
                }
            }
            Ok(bytes)
        },
        request.timeout_ms,
    ) {
        Ok(packets) => packets,
        Err(status) => return status,
    };
    let frame_count = packets.len() as u32;
    let encoded = match encode_batch(&packets) {
        Ok(encoded) => encoded,
        Err(status) => return status,
    };
    let (payload_owner, payload) = match store_owned_buffer(encoded) {
        Ok(value) => value,
        Err(status) => return status,
    };
    *out_batch = NnrpTransportFrameBatch {
        payload_owner,
        payload,
        frame_count,
        reserved0: 0,
    };
    NnrpFfiStatus::ok()
}

/// Measures a transport path using acknowledged NNRP probe packets.
///
/// # Safety
///
/// The endpoint buffer in `request.open` must remain readable for the duration of the call, and
/// `out_result` must be non-null, aligned, and writable for one [`NnrpTransportProbeResult`].
pub(super) unsafe fn transport_probe(
    request: NnrpTransportProbeRequest,
    out_result: *mut NnrpTransportProbeResult,
) -> NnrpFfiStatus {
    if out_result.is_null() {
        return NnrpFfiStatus::invalid_argument(131);
    }
    let (transport, endpoint, limit, timeout) = match validated_open_request(request.open) {
        Ok(value) => value,
        Err(status) => return status,
    };
    let sample_count = if request.sample_count == 0 {
        DEFAULT_PROBE_SAMPLES
    } else {
        request.sample_count
    };
    if sample_count > MAX_PROBE_SAMPLES {
        return NnrpFfiStatus::invalid_argument(132);
    }
    let payload_bytes = if request.probe_payload_bytes == 0 {
        DEFAULT_PROBE_PAYLOAD_BYTES
    } else {
        request.probe_payload_bytes
    };
    let probe_packet_bytes = COMMON_HEADER_LEN
        .checked_add(TRANSPORT_PROBE_METADATA_LEN)
        .and_then(|overhead| overhead.checked_add(payload_bytes as usize));
    if probe_packet_bytes.is_none_or(|packet_bytes| packet_bytes > limit.max_packet_bytes) {
        return NnrpFfiStatus::invalid_argument(133);
    }
    let result = run_async(
        async move {
            let mut connection =
                connect_transport(transport, endpoint, request.open.config, limit).await?;
            let mut rtts = Vec::with_capacity(sample_count as usize);
            let mut throughputs = Vec::with_capacity(sample_count as usize);
            for probe_id in 1..=sample_count {
                let metadata = TransportProbeMetadata {
                    probe_id,
                    probe_payload_bytes: payload_bytes,
                    client_send_ts_us: unix_time_us(),
                };
                let packet = RuntimePacket::new(
                    CommonHeader::new(MessageType::TransportProbe, 0, 0),
                    metadata.to_bytes()?.to_vec(),
                    vec![0; payload_bytes as usize],
                )?;
                let started = Instant::now();
                connection.write_packet(&packet).await?;
                let ack = connection.read_packet().await?;
                if ack.header.message_type != MessageType::TransportProbeAck {
                    return Err(RuntimeError::UnexpectedMessage(
                        "transport probe expected TRANSPORT_PROBE_ACK",
                    ));
                }
                let ack = TransportProbeAckMetadata::parse(&ack.metadata)?;
                if ack.probe_id != probe_id {
                    return Err(RuntimeError::UnexpectedMessage(
                        "transport probe acknowledgement id mismatch",
                    ));
                }
                let rtt = started.elapsed().as_micros().max(1) as u64;
                rtts.push(rtt);
                throughputs.push((payload_bytes as u64).saturating_mul(1_000_000) / rtt);
            }
            connection.close().await?;
            rtts.sort_unstable();
            throughputs.sort_unstable();
            Ok(NnrpTransportProbeResult {
                sample_count,
                success_count: sample_count,
                median_throughput_bytes_per_second: throughputs[throughputs.len() / 2],
                median_rtt_microseconds: rtts[rtts.len() / 2],
            })
        },
        timeout,
    );
    match result {
        Ok(result) => {
            *out_result = result;
            NnrpFfiStatus::ok()
        }
        Err(status) => status,
    }
}

/// Closes and releases a transport connection, listener, or security configuration.
///
/// # Safety
///
/// The handle must have been returned by this library. No other thread may be using the resource
/// while it is closed; repeating this call after a successful close is permitted.
pub(super) unsafe fn transport_close(handle: NnrpHandle) -> NnrpFfiStatus {
    if !matches!(
        handle.kind,
        value if value == NnrpHandleKind::TransportConnection as u32
            || value == NnrpHandleKind::TransportListener as u32
            || value == NnrpHandleKind::TransportSecurityConfig as u32
    ) {
        return NnrpFfiStatus::invalid_handle(handle.kind);
    }
    let resource = transport_store()
        .lock()
        .map_err(|_| transport_status(NnrpFfiStatusCode::InternalError, 134))
        .and_then(|mut store| store.close(handle));
    let resource = match resource {
        Ok(Some(resource)) => resource,
        Ok(None) => return NnrpFfiStatus::ok(),
        Err(status) => return status,
    };
    if let TransportResource::Connection(connection) = resource {
        return match run_async(
            async move { connection.lock().await.transport.close().await },
            DEFAULT_TIMEOUT_MS,
        ) {
            Ok(()) => NnrpFfiStatus::ok(),
            Err(status) => status,
        };
    }
    NnrpFfiStatus::ok()
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;
    use crate::transport_exports::*;

    fn view(bytes: &[u8]) -> NnrpBufferView {
        NnrpBufferView {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
        }
    }

    fn open_request(
        transport_id: TransportId,
        endpoint: &str,
        config: NnrpHandle,
    ) -> NnrpTransportOpenRequest {
        open_request_with_limit(transport_id, endpoint, config, 0)
    }

    fn open_request_with_limit(
        transport_id: TransportId,
        endpoint: &str,
        config: NnrpHandle,
        max_packet_bytes: u64,
    ) -> NnrpTransportOpenRequest {
        NnrpTransportOpenRequest {
            transport_id: transport_id as u32,
            flags: 0,
            endpoint: view(endpoint.as_bytes()),
            config,
            max_packet_bytes,
            timeout_ms: 5_000,
            reserved0: 0,
        }
    }

    unsafe fn listen(
        transport_id: TransportId,
        endpoint: &str,
        config: NnrpHandle,
    ) -> (NnrpHandle, String) {
        let mut listener = NnrpHandle::invalid();
        assert_eq!(
            nnrp_transport_listen(open_request(transport_id, endpoint, config), &mut listener,),
            NnrpFfiStatus::ok()
        );
        let mut owner = NnrpHandle::invalid();
        let mut endpoint_view = NnrpBufferView::empty();
        assert_eq!(
            nnrp_transport_listener_endpoint(listener, &mut owner, &mut endpoint_view),
            NnrpFfiStatus::ok()
        );
        let endpoint = String::from_utf8(
            std::slice::from_raw_parts(endpoint_view.ptr, endpoint_view.len).to_vec(),
        )
        .expect("listener endpoint is UTF-8");
        assert_eq!(crate::nnrp_buffer_release(owner), NnrpFfiStatus::ok());
        (listener, endpoint)
    }

    unsafe fn connect(transport_id: TransportId, endpoint: &str, config: NnrpHandle) -> NnrpHandle {
        let mut connection = NnrpHandle::invalid();
        assert_eq!(
            nnrp_transport_connect(
                open_request(transport_id, endpoint, config),
                &mut connection,
            ),
            NnrpFfiStatus::ok()
        );
        connection
    }

    unsafe fn accept(listener: NnrpHandle) -> NnrpHandle {
        let mut connection = NnrpHandle::invalid();
        assert_eq!(
            nnrp_transport_accept(
                NnrpTransportAcceptRequest {
                    listener,
                    timeout_ms: 5_000,
                    reserved0: 0,
                },
                &mut connection,
            ),
            NnrpFfiStatus::ok()
        );
        connection
    }

    unsafe fn send(connection: NnrpHandle, packets: &[Vec<u8>]) {
        let views = packets
            .iter()
            .map(|packet| view(packet))
            .collect::<Vec<_>>();
        assert_eq!(
            nnrp_transport_write_batch(NnrpTransportWriteBatchRequest {
                connection,
                frames: views.as_ptr(),
                frame_count: views.len() as u32,
                flags: 0,
            }),
            NnrpFfiStatus::ok()
        );
    }

    unsafe fn receive(connection: NnrpHandle, expected_frames: u32) -> Vec<Vec<u8>> {
        let mut packets = Vec::new();
        while packets.len() < expected_frames as usize {
            let mut batch = NnrpTransportFrameBatch::empty();
            assert_eq!(
                nnrp_transport_read_batch(
                    NnrpTransportReadBatchRequest {
                        connection,
                        max_frames: expected_frames - packets.len() as u32,
                        timeout_ms: 5_000,
                        max_bytes: 0,
                    },
                    &mut batch,
                ),
                NnrpFfiStatus::ok()
            );
            assert!(batch.frame_count > 0);
            let encoded = std::slice::from_raw_parts(batch.payload.ptr, batch.payload.len);
            let mut offset = 0;
            while offset < encoded.len() {
                let len =
                    u32::from_le_bytes(encoded[offset..offset + 4].try_into().unwrap()) as usize;
                offset += 4;
                packets.push(encoded[offset..offset + len].to_vec());
                offset += len;
            }
            assert_eq!(
                crate::nnrp_buffer_release(batch.payload_owner),
                NnrpFfiStatus::ok()
            );
        }
        assert_eq!(packets.len(), expected_frames as usize);
        packets
    }

    fn ping_packet(frame_id: u32) -> Vec<u8> {
        let mut header = CommonHeader::new(MessageType::Ping, 0, 0);
        header.frame_id = frame_id;
        RuntimePacket::new(header, Vec::new(), Vec::new())
            .unwrap()
            .to_bytes()
            .unwrap()
    }

    fn assert_code(status: NnrpFfiStatus, expected: NnrpFfiStatusCode) {
        assert_eq!(status.status_code, expected as u32, "{status:?}");
    }

    unsafe fn assert_loopback(
        transport_id: TransportId,
        listen_endpoint: &str,
        client_config: NnrpHandle,
        server_config: NnrpHandle,
    ) {
        let (listener, endpoint) = listen(transport_id, listen_endpoint, server_config);
        let accept_task = thread::spawn(move || accept(listener));
        let client = connect(transport_id, &endpoint, client_config);
        let server = accept_task.join().expect("accept thread joins");

        let request = vec![ping_packet(1), ping_packet(2)];
        send(client, &request);
        assert_eq!(receive(server, 2), request);

        let response = vec![ping_packet(3)];
        send(server, &response);
        assert_eq!(receive(client, 1), response);

        assert_eq!(nnrp_transport_close(client), NnrpFfiStatus::ok());
        assert_eq!(nnrp_transport_close(client), NnrpFfiStatus::ok());
        assert_eq!(nnrp_transport_close(server), NnrpFfiStatus::ok());
        assert_eq!(nnrp_transport_close(listener), NnrpFfiStatus::ok());
    }

    #[cfg(feature = "transport-tcp")]
    #[test]
    fn tcp_artifact_ffi_runs_real_packet_batch_loopback() {
        unsafe {
            assert_loopback(
                TransportId::Tcp,
                "tcp://127.0.0.1:0",
                NnrpHandle::invalid(),
                NnrpHandle::invalid(),
            );
        }
    }

    #[cfg(all(feature = "transport-ipc", unix))]
    #[test]
    fn unix_ipc_artifact_ffi_runs_real_packet_batch_loopback() {
        let path = std::env::temp_dir().join(format!(
            "nnrp-ffi-{}-{}.sock",
            std::process::id(),
            NEXT_TRANSPORT_HANDLE.load(Ordering::Relaxed)
        ));
        let endpoint = format!("unix://{}", path.display());
        unsafe {
            assert_loopback(
                TransportId::Ipc,
                &endpoint,
                NnrpHandle::invalid(),
                NnrpHandle::invalid(),
            );
        }
        let _ = std::fs::remove_file(path);
    }

    #[cfg(all(feature = "transport-ipc", windows))]
    #[test]
    fn named_pipe_artifact_ffi_runs_real_packet_batch_loopback() {
        let endpoint = format!(
            "npipe://nnrp-ffi-{}-{}",
            std::process::id(),
            NEXT_TRANSPORT_HANDLE.load(Ordering::Relaxed)
        );
        unsafe {
            assert_loopback(
                TransportId::Ipc,
                &endpoint,
                NnrpHandle::invalid(),
                NnrpHandle::invalid(),
            );
        }
    }

    #[cfg(feature = "transport-websocket")]
    #[test]
    fn websocket_artifact_ffi_runs_real_binary_packet_loopback() {
        unsafe {
            assert_loopback(
                TransportId::WebSocket,
                "ws://127.0.0.1:0/nnrp",
                NnrpHandle::invalid(),
                NnrpHandle::invalid(),
            );
        }
    }

    #[cfg(feature = "transport-quic")]
    unsafe fn security_configs(transport_id: TransportId) -> (NnrpHandle, NnrpHandle) {
        let (_, certificate) =
            nnrp_transport_quic::QuicServerEndpointConfig::self_signed_localhost(
                "127.0.0.1:0".parse().unwrap(),
            )
            .unwrap();
        let server_name = b"localhost";
        let mut client = NnrpHandle::invalid();
        let mut server = NnrpHandle::invalid();
        assert_eq!(
            nnrp_transport_client_security_config_create(
                NnrpTransportClientSecurityConfigRequest {
                    transport_id: transport_id as u32,
                    flags: 0,
                    server_name: view(server_name),
                    trusted_certificate_der: view(&certificate.certificate_der),
                },
                &mut client,
            ),
            NnrpFfiStatus::ok()
        );
        assert_eq!(
            nnrp_transport_server_security_config_create(
                NnrpTransportServerSecurityConfigRequest {
                    transport_id: transport_id as u32,
                    flags: 0,
                    certificate_der: view(&certificate.certificate_der),
                    private_key_pkcs8_der: view(&certificate.private_key_pkcs8_der),
                },
                &mut server,
            ),
            NnrpFfiStatus::ok()
        );
        (client, server)
    }

    #[cfg(feature = "transport-quic")]
    #[test]
    fn quic_artifact_ffi_runs_real_packet_batch_loopback() {
        unsafe {
            let (client_config, server_config) = security_configs(TransportId::Quic);
            assert_loopback(
                TransportId::Quic,
                "quic://127.0.0.1:0",
                client_config,
                server_config,
            );
            assert_eq!(nnrp_transport_close(client_config), NnrpFfiStatus::ok());
            assert_eq!(nnrp_transport_close(server_config), NnrpFfiStatus::ok());
        }
    }

    #[cfg(all(feature = "transport-websocket", feature = "transport-quic"))]
    #[test]
    fn secure_websocket_artifact_ffi_runs_real_tls_packet_loopback() {
        unsafe {
            let (client_config, server_config) = security_configs(TransportId::WebSocket);
            assert_loopback(
                TransportId::WebSocket,
                "wss://localhost:0/nnrp",
                client_config,
                server_config,
            );
            assert_eq!(nnrp_transport_close(client_config), NnrpFfiStatus::ok());
            assert_eq!(nnrp_transport_close(server_config), NnrpFfiStatus::ok());
        }
    }

    #[cfg(feature = "transport-tcp")]
    #[test]
    fn transport_probe_requires_and_receives_peer_acknowledgements() {
        unsafe {
            let (listener, endpoint) =
                listen(TransportId::Tcp, "tcp://127.0.0.1:0", NnrpHandle::invalid());
            let server_task = thread::spawn(move || {
                let server = accept(listener);
                let mut batch = NnrpTransportFrameBatch::empty();
                let _ = nnrp_transport_read_batch(
                    NnrpTransportReadBatchRequest {
                        connection: server,
                        max_frames: 1,
                        timeout_ms: 5_000,
                        max_bytes: 0,
                    },
                    &mut batch,
                );
                let _ = nnrp_transport_close(server);
                let _ = nnrp_transport_close(listener);
            });
            let mut result = NnrpTransportProbeResult {
                sample_count: 0,
                success_count: 0,
                median_throughput_bytes_per_second: 0,
                median_rtt_microseconds: 0,
            };
            assert_eq!(
                nnrp_transport_probe(
                    NnrpTransportProbeRequest {
                        open: open_request(TransportId::Tcp, &endpoint, NnrpHandle::invalid(),),
                        sample_count: 0,
                        probe_payload_bytes: 0,
                    },
                    &mut result,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.sample_count, 3);
            assert_eq!(result.success_count, 3);
            assert!(result.median_throughput_bytes_per_second > 0);
            assert!(result.median_rtt_microseconds > 0);
            server_task.join().expect("probe server joins");
        }
    }

    #[test]
    fn transport_helpers_map_defaults_validation_and_runtime_errors() {
        assert_eq!(effective_timeout(0), DEFAULT_TIMEOUT_MS);
        assert_eq!(effective_timeout(7), 7);
        assert_eq!(
            effective_packet_limit(0).unwrap(),
            DEFAULT_MAX_PACKET_BYTES as usize
        );
        assert_eq!(effective_packet_limit(17).unwrap(), 17);
        assert!(parse_transport_id(TransportId::Unspecified as u32).is_err());
        assert!(parse_transport_id(u32::MAX).is_err());

        unsafe {
            assert_eq!(
                copied_bytes(NnrpBufferView::empty(), 1).unwrap(),
                Vec::<u8>::new()
            );
            assert!(copied_bytes(
                NnrpBufferView {
                    ptr: std::ptr::null(),
                    len: 1
                },
                2
            )
            .is_err());
            assert!(copied_utf8(view(&[0xff]), 3).is_err());
        }

        assert_code(
            status_from_runtime_error(RuntimeError::Protocol(
                nnrp_core::NnrpError::SourceTooShort {
                    expected: 1,
                    actual: 0,
                },
            )),
            NnrpFfiStatusCode::InvalidArgument,
        );
        for error in [
            RuntimeError::UnsupportedTransport("test"),
            RuntimeError::FrameTooLarge {
                declared: 2,
                max: 1,
            },
        ] {
            assert_code(
                status_from_runtime_error(error),
                NnrpFfiStatusCode::InvalidArgument,
            );
        }
        for error in [
            RuntimeError::TransportClosed {
                transport: nnrp_runtime::RuntimeTransportKind::Tcp,
                detail: "closed".to_string(),
            },
            RuntimeError::UnexpectedMessage("test"),
        ] {
            assert_code(
                status_from_runtime_error(error),
                NnrpFfiStatusCode::InvalidState,
            );
        }
        for error in [
            RuntimeError::Io(std::io::Error::other("test")),
            RuntimeError::FrameIdOverflow,
            RuntimeError::Internal("test"),
        ] {
            assert_code(
                status_from_runtime_error(error),
                NnrpFfiStatusCode::InternalError,
            );
        }

        assert!(packet_from_bytes(&[], DEFAULT_MAX_PACKET_BYTES as usize).is_err());
        let packet = ping_packet(90);
        assert!(packet_from_bytes(&packet, packet.len() - 1).is_err());
        let mut mismatched = packet.clone();
        mismatched.push(0);
        assert!(packet_from_bytes(&mismatched, mismatched.len()).is_err());
        assert_eq!(
            packet_from_bytes(&packet, packet.len())
                .unwrap()
                .header
                .frame_id,
            90
        );
    }

    #[test]
    fn transport_store_reuses_closed_slots_with_new_generations() {
        fn resource() -> TransportResource {
            TransportResource::SecurityConfig(TransportSecurityConfig::Client {
                transport_id: TransportId::Quic,
                server_name: "localhost".to_string(),
                trusted_certificate_der: Vec::new(),
            })
        }

        let mut store = TransportStore::default();
        let first = store.insert(NnrpHandleKind::TransportSecurityConfig, resource());
        assert!(store.close(first).unwrap().is_some());
        assert!(store.close(first).unwrap().is_none());

        let second = store.insert(NnrpHandleKind::TransportSecurityConfig, resource());
        assert_eq!(second.id, first.id);
        assert_eq!(second.generation, first.generation + 1);
        assert_eq!(store.slots.len(), 1);
        assert!(store.close(first).unwrap().is_none());
        assert!(store.get(first).is_none());
        assert!(store.get(second).is_some());
    }

    #[cfg(feature = "transport-websocket")]
    #[test]
    fn endpoint_helpers_reject_mismatched_and_malformed_schemes() {
        assert!(socket_endpoint("udp://127.0.0.1:1", "tcp://").is_err());
        assert!(websocket_listen_addr("not a url").is_err());
        assert!(websocket_listen_addr("wss://127.0.0.1:1").is_err());
        assert!(websocket_listen_addr("ws:///missing-host").is_err());
        assert!(
            normalized_websocket_endpoint("not a url", "127.0.0.1:1".parse().unwrap()).is_err()
        );
        assert_eq!(
            normalized_websocket_endpoint("ws://127.0.0.1:0/path", "127.0.0.1:23".parse().unwrap())
                .unwrap(),
            "ws://127.0.0.1:23/path"
        );
    }

    #[cfg(all(feature = "transport-quic", feature = "transport-websocket"))]
    #[test]
    fn security_config_ffi_rejects_invalid_shapes_and_cross_transport_use() {
        unsafe {
            let empty_client = NnrpTransportClientSecurityConfigRequest {
                transport_id: TransportId::Quic as u32,
                flags: 0,
                server_name: NnrpBufferView::empty(),
                trusted_certificate_der: NnrpBufferView::empty(),
            };
            assert_code(
                nnrp_transport_client_security_config_create(empty_client, std::ptr::null_mut()),
                NnrpFfiStatusCode::InvalidArgument,
            );
            let mut output = NnrpHandle::invalid();
            let mut flagged = empty_client;
            flagged.flags = 1;
            assert_code(
                nnrp_transport_client_security_config_create(flagged, &mut output),
                NnrpFfiStatusCode::InvalidArgument,
            );
            assert_code(
                nnrp_transport_client_security_config_create(empty_client, &mut output),
                NnrpFfiStatusCode::InvalidArgument,
            );
            let mut unsupported = empty_client;
            unsupported.transport_id = TransportId::Tcp as u32;
            assert_code(
                nnrp_transport_client_security_config_create(unsupported, &mut output),
                NnrpFfiStatusCode::InvalidArgument,
            );

            let empty_server = NnrpTransportServerSecurityConfigRequest {
                transport_id: TransportId::Quic as u32,
                flags: 0,
                certificate_der: NnrpBufferView::empty(),
                private_key_pkcs8_der: NnrpBufferView::empty(),
            };
            assert_code(
                nnrp_transport_server_security_config_create(empty_server, std::ptr::null_mut()),
                NnrpFfiStatusCode::InvalidArgument,
            );
            let mut flagged = empty_server;
            flagged.flags = 1;
            assert_code(
                nnrp_transport_server_security_config_create(flagged, &mut output),
                NnrpFfiStatusCode::InvalidArgument,
            );
            assert_code(
                nnrp_transport_server_security_config_create(empty_server, &mut output),
                NnrpFfiStatusCode::InvalidArgument,
            );

            let (quic_client, quic_server) = security_configs(TransportId::Quic);
            let (ws_client, ws_server) = security_configs(TransportId::WebSocket);
            let mut connection = NnrpHandle::invalid();
            let mut listener = NnrpHandle::invalid();

            for (transport, endpoint, config) in [
                (TransportId::Quic, "quic://127.0.0.1:1", quic_server),
                (TransportId::Quic, "quic://127.0.0.1:1", ws_client),
                (TransportId::WebSocket, "wss://localhost:1/nnrp", ws_server),
                (
                    TransportId::WebSocket,
                    "wss://localhost:1/nnrp",
                    quic_client,
                ),
            ] {
                assert_code(
                    nnrp_transport_connect(
                        open_request(transport, endpoint, config),
                        &mut connection,
                    ),
                    NnrpFfiStatusCode::InvalidArgument,
                );
            }
            for (transport, endpoint, config) in [
                (TransportId::Quic, "quic://127.0.0.1:0", quic_client),
                (TransportId::Quic, "quic://127.0.0.1:0", ws_server),
                (TransportId::WebSocket, "wss://localhost:0/nnrp", ws_client),
                (
                    TransportId::WebSocket,
                    "wss://localhost:0/nnrp",
                    quic_server,
                ),
            ] {
                assert_code(
                    nnrp_transport_listen(open_request(transport, endpoint, config), &mut listener),
                    NnrpFfiStatusCode::InvalidArgument,
                );
            }

            for handle in [quic_client, quic_server, ws_client, ws_server] {
                assert_eq!(nnrp_transport_close(handle), NnrpFfiStatus::ok());
            }
        }
    }

    #[cfg(feature = "transport-tcp")]
    #[test]
    fn public_transport_ffi_rejects_invalid_arguments_and_handles() {
        unsafe {
            let valid = open_request(TransportId::Tcp, "tcp://127.0.0.1:0", NnrpHandle::invalid());
            assert_code(
                nnrp_transport_connect(valid, std::ptr::null_mut()),
                NnrpFfiStatusCode::InvalidArgument,
            );
            assert_code(
                nnrp_transport_listen(valid, std::ptr::null_mut()),
                NnrpFfiStatusCode::InvalidArgument,
            );

            let mut output = NnrpHandle::invalid();
            let mut invalid = valid;
            invalid.flags = 1;
            assert_code(
                nnrp_transport_connect(invalid, &mut output),
                NnrpFfiStatusCode::InvalidArgument,
            );
            invalid = valid;
            invalid.reserved0 = 1;
            assert_code(
                nnrp_transport_listen(invalid, &mut output),
                NnrpFfiStatusCode::InvalidArgument,
            );
            invalid = valid;
            invalid.transport_id = TransportId::Unspecified as u32;
            assert_code(
                nnrp_transport_connect(invalid, &mut output),
                NnrpFfiStatusCode::InvalidArgument,
            );
            invalid = valid;
            invalid.endpoint = NnrpBufferView::empty();
            assert_code(
                nnrp_transport_connect(invalid, &mut output),
                NnrpFfiStatusCode::InvalidArgument,
            );
            invalid = valid;
            invalid.config = NnrpHandle::new(NnrpHandleKind::TransportSecurityConfig, 999, 1);
            assert_code(
                nnrp_transport_listen(invalid, &mut output),
                NnrpFfiStatusCode::InvalidArgument,
            );

            let bogus = NnrpHandle::new(NnrpHandleKind::TransportListener, u64::MAX, 1);
            let mut owner = NnrpHandle::invalid();
            let mut endpoint = NnrpBufferView::empty();
            assert_code(
                nnrp_transport_listener_endpoint(bogus, &mut owner, &mut endpoint),
                NnrpFfiStatusCode::InvalidHandle,
            );
            assert_code(
                nnrp_transport_listener_endpoint(bogus, std::ptr::null_mut(), &mut endpoint),
                NnrpFfiStatusCode::InvalidArgument,
            );
            let mut connection = NnrpHandle::invalid();
            assert_code(
                nnrp_transport_accept(
                    NnrpTransportAcceptRequest {
                        listener: bogus,
                        timeout_ms: 1,
                        reserved0: 0,
                    },
                    &mut connection,
                ),
                NnrpFfiStatusCode::InvalidHandle,
            );
            assert_code(
                nnrp_transport_accept(
                    NnrpTransportAcceptRequest {
                        listener: bogus,
                        timeout_ms: 1,
                        reserved0: 1,
                    },
                    &mut connection,
                ),
                NnrpFfiStatusCode::InvalidArgument,
            );

            assert_code(
                nnrp_transport_write_batch(NnrpTransportWriteBatchRequest {
                    connection: NnrpHandle::invalid(),
                    frames: std::ptr::null(),
                    frame_count: 0,
                    flags: 0,
                }),
                NnrpFfiStatusCode::InvalidArgument,
            );
            let packet = ping_packet(1);
            let frame = view(&packet);
            assert_code(
                nnrp_transport_write_batch(NnrpTransportWriteBatchRequest {
                    connection: NnrpHandle::invalid(),
                    frames: &frame,
                    frame_count: 1,
                    flags: 0,
                }),
                NnrpFfiStatusCode::InvalidHandle,
            );
            assert_code(
                nnrp_transport_read_batch(
                    NnrpTransportReadBatchRequest {
                        connection: NnrpHandle::invalid(),
                        max_frames: 1,
                        timeout_ms: 1,
                        max_bytes: 0,
                    },
                    std::ptr::null_mut(),
                ),
                NnrpFfiStatusCode::InvalidArgument,
            );
            let mut batch = NnrpTransportFrameBatch::empty();
            assert_code(
                nnrp_transport_read_batch(
                    NnrpTransportReadBatchRequest {
                        connection: NnrpHandle::invalid(),
                        max_frames: 1,
                        timeout_ms: 1,
                        max_bytes: 0,
                    },
                    &mut batch,
                ),
                NnrpFfiStatusCode::InvalidHandle,
            );

            let probe = NnrpTransportProbeRequest {
                open: valid,
                sample_count: 1,
                probe_payload_bytes: 1,
            };
            assert_code(
                nnrp_transport_probe(probe, std::ptr::null_mut()),
                NnrpFfiStatusCode::InvalidArgument,
            );
            let mut result = NnrpTransportProbeResult {
                sample_count: 0,
                success_count: 0,
                median_throughput_bytes_per_second: 0,
                median_rtt_microseconds: 0,
            };
            let mut too_many = probe;
            too_many.sample_count = MAX_PROBE_SAMPLES + 1;
            assert_code(
                nnrp_transport_probe(too_many, &mut result),
                NnrpFfiStatusCode::InvalidArgument,
            );
            let mut too_large = probe;
            too_large.open.max_packet_bytes =
                (COMMON_HEADER_LEN + TRANSPORT_PROBE_METADATA_LEN) as u64;
            too_large.probe_payload_bytes = 1;
            assert_code(
                nnrp_transport_probe(too_large, &mut result),
                NnrpFfiStatusCode::InvalidArgument,
            );

            assert_code(
                nnrp_transport_close(NnrpHandle::invalid()),
                NnrpFfiStatusCode::InvalidHandle,
            );
            assert_code(
                nnrp_transport_close(NnrpHandle::new(
                    NnrpHandleKind::TransportConnection,
                    u64::MAX,
                    1,
                )),
                NnrpFfiStatusCode::InvalidHandle,
            );
        }
    }

    #[cfg(feature = "transport-tcp")]
    #[test]
    fn connection_packet_limits_apply_to_connected_and_accepted_handles() {
        unsafe {
            let packet = ping_packet(9);
            let max_packet_bytes = (packet.len() - 1) as u64;
            let mut listener = NnrpHandle::invalid();
            assert_eq!(
                nnrp_transport_listen(
                    open_request_with_limit(
                        TransportId::Tcp,
                        "tcp://127.0.0.1:0",
                        NnrpHandle::invalid(),
                        max_packet_bytes,
                    ),
                    &mut listener,
                ),
                NnrpFfiStatus::ok()
            );
            let mut owner = NnrpHandle::invalid();
            let mut endpoint_view = NnrpBufferView::empty();
            assert_eq!(
                nnrp_transport_listener_endpoint(listener, &mut owner, &mut endpoint_view),
                NnrpFfiStatus::ok()
            );
            let endpoint = String::from_utf8(
                std::slice::from_raw_parts(endpoint_view.ptr, endpoint_view.len).to_vec(),
            )
            .unwrap();
            assert_eq!(crate::nnrp_buffer_release(owner), NnrpFfiStatus::ok());

            let accept_task = thread::spawn(move || accept(listener));
            let mut client = NnrpHandle::invalid();
            assert_eq!(
                nnrp_transport_connect(
                    open_request_with_limit(
                        TransportId::Tcp,
                        &endpoint,
                        NnrpHandle::invalid(),
                        max_packet_bytes,
                    ),
                    &mut client,
                ),
                NnrpFfiStatus::ok()
            );
            let server = accept_task.join().unwrap();
            let packet_view = view(&packet);
            for connection in [client, server] {
                assert_code(
                    nnrp_transport_write_batch(NnrpTransportWriteBatchRequest {
                        connection,
                        frames: &packet_view,
                        frame_count: 1,
                        flags: 0,
                    }),
                    NnrpFfiStatusCode::InvalidArgument,
                );
            }

            assert_eq!(nnrp_transport_close(client), NnrpFfiStatus::ok());
            assert_eq!(nnrp_transport_close(server), NnrpFfiStatus::ok());
            assert_eq!(nnrp_transport_close(listener), NnrpFfiStatus::ok());
        }
    }

    #[cfg(feature = "transport-tcp")]
    #[test]
    fn packet_batch_validation_and_read_limits_preserve_pending_packets() {
        unsafe {
            let (listener, endpoint) =
                listen(TransportId::Tcp, "tcp://127.0.0.1:0", NnrpHandle::invalid());
            let accept_task = thread::spawn(move || accept(listener));
            let client = connect(TransportId::Tcp, &endpoint, NnrpHandle::invalid());
            let server = accept_task.join().unwrap();

            let empty = NnrpBufferView::empty();
            assert_code(
                nnrp_transport_write_batch(NnrpTransportWriteBatchRequest {
                    connection: client,
                    frames: &empty,
                    frame_count: 1,
                    flags: 0,
                }),
                NnrpFfiStatusCode::InvalidArgument,
            );
            let null_frame = NnrpBufferView {
                ptr: std::ptr::null(),
                len: 1,
            };
            assert_code(
                nnrp_transport_write_batch(NnrpTransportWriteBatchRequest {
                    connection: client,
                    frames: &null_frame,
                    frame_count: 1,
                    flags: 0,
                }),
                NnrpFfiStatusCode::InvalidArgument,
            );
            let mut mismatch = ping_packet(10);
            mismatch.push(0);
            let mismatch = view(&mismatch);
            assert_code(
                nnrp_transport_write_batch(NnrpTransportWriteBatchRequest {
                    connection: client,
                    frames: &mismatch,
                    frame_count: 1,
                    flags: 0,
                }),
                NnrpFfiStatusCode::ProtocolError,
            );

            let packets = vec![ping_packet(11), ping_packet(12)];
            send(client, &packets);
            let mut batch = NnrpTransportFrameBatch::empty();
            let first_encoded_len = (BATCH_LENGTH_PREFIX_BYTES + packets[0].len()) as u64;
            assert_eq!(
                nnrp_transport_read_batch(
                    NnrpTransportReadBatchRequest {
                        connection: server,
                        max_frames: 2,
                        timeout_ms: 5_000,
                        max_bytes: first_encoded_len,
                    },
                    &mut batch,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(batch.frame_count, 1);
            assert_eq!(batch.payload.len as u64, first_encoded_len);
            let encoded = std::slice::from_raw_parts(batch.payload.ptr, batch.payload.len);
            assert_eq!(&encoded[BATCH_LENGTH_PREFIX_BYTES..], packets[0]);
            assert_eq!(
                crate::nnrp_buffer_release(batch.payload_owner),
                NnrpFfiStatus::ok()
            );
            assert_eq!(receive(server, 1), vec![packets[1].clone()]);

            let packet = ping_packet(13);
            send(client, std::slice::from_ref(&packet));
            batch = NnrpTransportFrameBatch::empty();
            assert_code(
                nnrp_transport_read_batch(
                    NnrpTransportReadBatchRequest {
                        connection: server,
                        max_frames: 1,
                        timeout_ms: 5_000,
                        max_bytes: (BATCH_LENGTH_PREFIX_BYTES + packet.len() - 1) as u64,
                    },
                    &mut batch,
                ),
                NnrpFfiStatusCode::InvalidArgument,
            );
            assert_eq!(receive(server, 1), vec![packet]);

            assert_eq!(nnrp_transport_close(client), NnrpFfiStatus::ok());
            assert_eq!(nnrp_transport_close(server), NnrpFfiStatus::ok());
            assert_eq!(nnrp_transport_close(listener), NnrpFfiStatus::ok());
        }
    }

    #[test]
    fn transport_ffi_layout_values_are_frozen() {
        assert_eq!(NnrpHandleKind::TransportConnection as u32, 10);
        assert_eq!(NnrpHandleKind::TransportListener as u32, 11);
        assert_eq!(NnrpHandleKind::TransportSecurityConfig as u32, 12);
        assert_eq!(std::mem::size_of::<NnrpTransportProbeResult>(), 24);
    }
}
