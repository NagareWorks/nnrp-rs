use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    Arc, Mutex, MutexGuard,
};
use std::task::Poll;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use futures_channel::mpsc;
use futures_timer::Delay;
use futures_util::{
    future::{poll_fn, select, Either},
    lock::Mutex as AsyncMutex,
    stream::StreamExt,
};
use nnrp_core::{
    validate_control_request_semantics, validate_partial_result_semantics,
    validate_pressure_semantics, validate_profile_assignment, validate_progress_semantics,
    validate_result_drop_header, validate_result_drop_reason_semantics,
    validate_scheduling_semantics, validate_trace_context_semantics, BudgetMetadata,
    CacheAckMetadata, CacheInvalidateMetadata, CacheMissMetadata, CacheObjectId, CacheObjectKind,
    CachePutMetadata, CacheReferenceMetadata, CapabilityMetadata, ClientHelloMetadata,
    CommonHeader, ConnectionLifecycle, ControlRequestMetadata, FlowUpdateMetadata,
    FrameSubmitMetadata, MessageType, ObjectDeltaMetadata, ObjectDescriptorMetadata,
    ObjectReferenceMetadata, ObjectReleaseMetadata, OperationCancelRequest, OperationDescriptor,
    OperationRegistry, PartialResultMetadata, PressureMetadata, ProgressMetadata,
    RecoverableErrorMetadata, ResultDropReasonMetadata, ResultHintMetadata, ResultPushMetadata,
    RetryAfterMetadata, RouteHintMetadata, RuntimeRole, SchedulingMetadata, SchemaRegistry,
    ServerHelloAckMetadata, SessionCloseAckMetadata, SessionCloseMetadata, SessionCloseStatus,
    SessionMigrateAckMetadata, SessionMigrateMetadata, SessionOpenAckMetadata, SessionOpenMetadata,
    SessionPatchAckMetadata, SessionPatchMetadata, SessionStatus, SupersedeMetadata,
    TraceContextMetadata, TransportProbeAckMetadata, TransportProbeMetadata, BUDGET_METADATA_LEN,
    CACHE_ACK_METADATA_LEN, CACHE_INVALIDATE_METADATA_LEN, CACHE_MISS_METADATA_LEN,
    CACHE_PUT_METADATA_LEN, CACHE_REFERENCE_METADATA_LEN, CAPABILITY_METADATA_LEN,
    CLIENT_HELLO_METADATA_LEN, CONTROL_REQUEST_METADATA_LEN, FLOW_UPDATE_METADATA_LEN,
    FRAME_SUBMIT_METADATA_LEN, OBJECT_DELTA_METADATA_LEN, OBJECT_DESCRIPTOR_METADATA_LEN,
    OBJECT_REFERENCE_METADATA_LEN, OBJECT_RELEASE_METADATA_LEN, PARTIAL_RESULT_METADATA_LEN,
    PRESSURE_METADATA_LEN, PROGRESS_METADATA_LEN, RECOVERABLE_ERROR_METADATA_LEN,
    RESULT_DROP_REASON_DEADLINE_EXPIRED, RESULT_DROP_REASON_METADATA_LEN, RESULT_PUSH_METADATA_LEN,
    RETRY_AFTER_METADATA_LEN, ROUTE_HINT_METADATA_LEN, SCHEDULING_FLAG_EMIT_DROP_REASON,
    SCHEDULING_METADATA_LEN, SERVER_HELLO_ACK_METADATA_LEN, SESSION_ACK_FLAG_RESUME_ENABLED,
    SESSION_CLOSE_ACK_METADATA_LEN, SESSION_ERROR_LIMIT_REACHED, SESSION_ERROR_NONE,
    SESSION_ERROR_PROFILE_UNSUPPORTED, SESSION_ERROR_RESUME_REJECTED,
    SESSION_ERROR_SCHEMA_UNSUPPORTED, SESSION_FLAG_ALLOW_RESUME, SESSION_MIGRATE_ACK_METADATA_LEN,
    SESSION_MIGRATE_METADATA_LEN, SESSION_OPEN_ACK_METADATA_LEN, SESSION_PATCH_ACK_METADATA_LEN,
    SESSION_PATCH_METADATA_LEN, SUPERSEDE_METADATA_LEN, TRACE_CONTEXT_METADATA_LEN,
};
#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
use tokio::net::TcpListener;

#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
use crate::TcpFramedListener;
use crate::{
    multiplex::{spawn_runtime_task, MultiplexedConnection},
    server_provider::{bind_server, BoundServerProvider},
    BoxedFramedListener, BoxedFramedTransport, FramedListener, NnrpRuntimeEvent, NnrpServerOptions,
    NnrpServerProvider, ProviderEndpoint, RuntimeError, RuntimeFrameHeader, RuntimePacket,
    RuntimePressureState,
};

#[derive(Clone)]
pub struct NnrpServerConfig {
    pub supported_profiles: Vec<u16>,
    pub supported_cache_objects: Vec<CacheObjectKind>,
    pub max_cache_objects: u64,
    pub max_cache_object_bytes: u32,
    pub schema_registry: SchemaRegistry,
    pub resume_token_bytes: u32,
    pub max_in_flight_operations: u16,
    pub granted_operation_credit: u16,
    pub lease_ttl_ms: u32,
    pub resume_window_ms: u32,
    pub application_policy: Arc<dyn NnrpServerPolicy>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NnrpServerAcceptOptions {
    pub timeout_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpServerPolicyDecision {
    pub accepted: bool,
    pub session_error_code: u32,
    pub diagnostic: Option<String>,
}

impl NnrpServerPolicyDecision {
    pub fn accept() -> Self {
        Self {
            accepted: true,
            session_error_code: SESSION_ERROR_NONE,
            diagnostic: None,
        }
    }

    pub fn reject(session_error_code: u32, diagnostic: impl Into<String>) -> Self {
        Self {
            accepted: false,
            session_error_code,
            diagnostic: Some(diagnostic.into()),
        }
    }
}

impl fmt::Debug for NnrpServerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NnrpServerConfig")
            .field("supported_profiles", &self.supported_profiles)
            .field("supported_cache_objects", &self.supported_cache_objects)
            .field("max_cache_objects", &self.max_cache_objects)
            .field("max_cache_object_bytes", &self.max_cache_object_bytes)
            .field("schema_registry", &self.schema_registry)
            .field("resume_token_bytes", &self.resume_token_bytes)
            .field("max_in_flight_operations", &self.max_in_flight_operations)
            .field("granted_operation_credit", &self.granted_operation_credit)
            .field("lease_ttl_ms", &self.lease_ttl_ms)
            .field("resume_window_ms", &self.resume_window_ms)
            .field("application_policy", &"<dyn NnrpServerPolicy>")
            .finish()
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait NnrpServerPolicy: Send + Sync {
    async fn evaluate(&self, open: &SessionOpenMetadata) -> NnrpServerPolicyDecision;
}

#[derive(Debug, Default)]
pub struct AllowAllServerPolicy;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl NnrpServerPolicy for AllowAllServerPolicy {
    async fn evaluate(&self, _open: &SessionOpenMetadata) -> NnrpServerPolicyDecision {
        NnrpServerPolicyDecision::accept()
    }
}

impl Default for NnrpServerConfig {
    fn default() -> Self {
        Self {
            supported_profiles: vec![nnrp_core::PROFILE_TOKEN],
            supported_cache_objects: Vec::new(),
            max_cache_objects: 0,
            max_cache_object_bytes: 0,
            schema_registry: SchemaRegistry::standard(),
            resume_token_bytes: 24,
            max_in_flight_operations: 4,
            granted_operation_credit: 2,
            lease_ttl_ms: 30_000,
            resume_window_ms: 120_000,
            application_policy: Arc::new(AllowAllServerPolicy),
        }
    }
}

impl NnrpServerConfig {
    pub fn with_supported_profiles(mut self, profiles: impl Into<Vec<u16>>) -> Self {
        self.supported_profiles = profiles.into();
        self
    }

    pub fn with_supported_cache_objects(
        mut self,
        objects: impl Into<Vec<CacheObjectKind>>,
    ) -> Self {
        self.supported_cache_objects = objects.into();
        self
    }

    pub fn with_cache_limits(mut self, max_objects: u64, max_object_bytes: u32) -> Self {
        self.max_cache_objects = max_objects;
        self.max_cache_object_bytes = max_object_bytes;
        self
    }

    pub fn with_schema_registry(mut self, schema_registry: SchemaRegistry) -> Self {
        self.schema_registry = schema_registry;
        self
    }

    pub fn with_resume_token_bytes(mut self, resume_token_bytes: u32) -> Self {
        self.resume_token_bytes = resume_token_bytes;
        self
    }

    pub fn with_application_policy<P>(mut self, policy: P) -> Self
    where
        P: NnrpServerPolicy + 'static,
    {
        self.application_policy = Arc::new(policy);
        self
    }

    async fn validate_client_open(
        &self,
        open: &SessionOpenMetadata,
    ) -> Result<(), NnrpServerPolicyDecision> {
        if !self.supported_profiles.contains(&open.profile_id)
            || validate_profile_assignment(open.profile_id).is_err()
        {
            return Err(NnrpServerPolicyDecision::reject(
                SESSION_ERROR_PROFILE_UNSUPPORTED,
                "requested profile is unsupported",
            ));
        }

        if self
            .schema_registry
            .lookup(open.schema_id, open.schema_version)
            .is_none()
        {
            return Err(NnrpServerPolicyDecision::reject(
                SESSION_ERROR_SCHEMA_UNSUPPORTED,
                "requested schema is unsupported",
            ));
        }

        if open.max_in_flight_operations > self.max_in_flight_operations {
            return Err(NnrpServerPolicyDecision::reject(
                SESSION_ERROR_LIMIT_REACHED,
                "requested in-flight limit exceeds the server limit",
            ));
        }

        let decision = self.application_policy.evaluate(open).await;
        if !decision.accepted {
            return Err(decision);
        }

        Ok(())
    }
}

pub struct NnrpServer {
    listeners: AsyncMutex<Option<Vec<BoundServerProvider>>>,
    next_accept_index: AtomicUsize,
    listener_set_closed: AtomicBool,
    bound_provider_endpoints: BTreeMap<nnrp_core::TransportId, ProviderEndpoint>,
    primary_local_addr: Option<std::net::SocketAddr>,
    config: NnrpServerConfig,
    sessions: SharedSessionRegistry,
    accepted_sessions: AsyncMutex<mpsc::UnboundedReceiver<NnrpServerSession>>,
    accepted_session_sender: mpsc::UnboundedSender<NnrpServerSession>,
}

pub struct NnrpServerSession {
    session_id: u32,
    active_transport_id: nnrp_core::TransportId,
    client_open: SessionOpenMetadata,
    transport: BoxedFramedTransport,
    lifecycle: ConnectionLifecycle,
    operations: OperationRegistry,
    frame_operations: BTreeMap<u32, u64>,
    operation_frames: BTreeMap<u64, u32>,
    pressure: RuntimePressureState,
    cache_objects: Vec<CacheObjectId>,
    supported_cache_objects: Vec<CacheObjectKind>,
    max_cache_objects: u64,
    max_cache_object_bytes: u32,
    connection_nonce: u64,
    sessions: SharedSessionRegistry,
    pending_close: Option<SessionCloseMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSessionRecord {
    pub session_id: u32,
    pub profile_id: u16,
    pub schema_id: u32,
    pub schema_version: u32,
    pub resume_enabled: bool,
    pub resume_token_bytes: u32,
    pub resume_token: Vec<u8>,
    pub resume_expires_at_ms: u64,
    pub last_operation_id: u64,
    pub active: bool,
    pub connection_nonce: u64,
}

static NEXT_CONNECTION_NONCE: AtomicU64 = AtomicU64::new(1);

type SharedSessionRegistry = Arc<Mutex<BTreeMap<u32, RuntimeSessionRecord>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpSubmit {
    pub operation_id: u64,
    pub frame_id: u32,
    pub metadata: FrameSubmitMetadata,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpCancel {
    pub frame_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpMigration {
    pub metadata: SessionMigrateMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpRuntimeControl {
    pub message_type: MessageType,
    pub metadata: ControlRequestMetadata,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpSchedulingUpdate {
    pub message_type: MessageType,
    pub metadata: SchedulingMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpPressureUpdate {
    pub message_type: MessageType,
    pub metadata: PressureMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NnrpServerEvent {
    Submit(NnrpSubmit),
    FrameCancel(NnrpCancel),
    PartialResult {
        metadata: PartialResultMetadata,
        body: Vec<u8>,
    },
    Progress {
        metadata: ProgressMetadata,
        body: Vec<u8>,
    },
    ResultDropReason {
        metadata: ResultDropReasonMetadata,
        body: Vec<u8>,
    },
    Control(NnrpRuntimeControl),
    Scheduling(NnrpSchedulingUpdate),
    Supersede {
        metadata: SupersedeMetadata,
        body: Vec<u8>,
    },
    Budget(BudgetMetadata),
    FlowUpdate(FlowUpdateMetadata),
    Pressure(NnrpPressureUpdate),
    Capability {
        message_type: MessageType,
        metadata: CapabilityMetadata,
        body: Vec<u8>,
    },
    RouteHint {
        message_type: MessageType,
        metadata: RouteHintMetadata,
        body: Vec<u8>,
    },
    TraceContext {
        frame_id: u32,
        metadata: TraceContextMetadata,
        body: Vec<u8>,
    },
    RecoverableError {
        metadata: RecoverableErrorMetadata,
        body: Vec<u8>,
    },
    RetryAfter {
        metadata: RetryAfterMetadata,
        body: Vec<u8>,
    },
    ObjectDeclare {
        metadata: ObjectDescriptorMetadata,
        body: Vec<u8>,
    },
    ObjectRef {
        metadata: ObjectReferenceMetadata,
        body: Vec<u8>,
    },
    ObjectRelease {
        metadata: ObjectReleaseMetadata,
        body: Vec<u8>,
    },
    ObjectDelta {
        message_type: MessageType,
        metadata: ObjectDeltaMetadata,
        body: Vec<u8>,
    },
    CacheReference {
        metadata: CacheReferenceMetadata,
        body: Vec<u8>,
    },
    CacheMiss {
        metadata: CacheMissMetadata,
        body: Vec<u8>,
    },
    CacheInvalidate(CacheInvalidateMetadata),
    Close(SessionCloseMetadata),
}

impl NnrpServer {
    pub async fn listen<I>(options: NnrpServerOptions, providers: I) -> Result<Self, RuntimeError>
    where
        I: IntoIterator<Item = Arc<dyn NnrpServerProvider>>,
    {
        let listeners = bind_server(&options, providers).await?;
        Ok(Self::from_bound_listeners(
            listeners,
            options.session_defaults,
        ))
    }

    #[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
    pub async fn bind_tcp(
        addr: impl tokio::net::ToSocketAddrs,
        config: NnrpServerConfig,
    ) -> Result<Self, RuntimeError> {
        Self::from_listener(
            TcpFramedListener::new(TcpListener::bind(addr).await?),
            config,
        )
    }

    pub async fn bind_quic(
        _endpoint: &str,
        _config: NnrpServerConfig,
    ) -> Result<Self, RuntimeError> {
        Err(RuntimeError::UnsupportedTransport(
            "QUIC provider is not installed; use from_listener with a QUIC FramedListener",
        ))
    }

    pub fn from_listener<L>(listener: L, config: NnrpServerConfig) -> Result<Self, RuntimeError>
    where
        L: FramedListener + 'static,
    {
        Self::from_boxed_listener(Box::new(listener), config)
    }

    pub fn from_boxed_listener(
        listener: BoxedFramedListener,
        config: NnrpServerConfig,
    ) -> Result<Self, RuntimeError> {
        Ok(Self::from_bound_listeners(
            vec![BoundServerProvider::from_listener(listener)],
            config,
        ))
    }

    pub fn from_bound_listener<L>(
        endpoint: ProviderEndpoint,
        listener: L,
        config: NnrpServerConfig,
    ) -> Result<Self, RuntimeError>
    where
        L: FramedListener + 'static,
    {
        Ok(Self::from_bound_listeners(
            vec![BoundServerProvider::new(endpoint, Box::new(listener))?],
            config,
        ))
    }

    fn from_bound_listeners(listeners: Vec<BoundServerProvider>, config: NnrpServerConfig) -> Self {
        let (accepted_session_sender, accepted_sessions) = mpsc::unbounded();
        let bound_provider_endpoints = listeners
            .iter()
            .filter_map(|listener| {
                listener
                    .provider_endpoint()
                    .cloned()
                    .map(|endpoint| (listener.transport_id(), endpoint))
            })
            .collect();
        let primary_local_addr = listeners.iter().find_map(BoundServerProvider::local_addr);
        Self {
            listeners: AsyncMutex::new(Some(listeners)),
            next_accept_index: AtomicUsize::new(0),
            listener_set_closed: AtomicBool::new(false),
            bound_provider_endpoints,
            primary_local_addr,
            config,
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
            accepted_sessions: AsyncMutex::new(accepted_sessions),
            accepted_session_sender,
        }
    }

    pub fn local_addr(&self) -> Result<std::net::SocketAddr, RuntimeError> {
        self.primary_local_addr
            .ok_or(RuntimeError::UnsupportedTransport(
                "server listener set does not expose an IP socket address",
            ))
    }

    pub fn bound_provider_endpoints(&self) -> &BTreeMap<nnrp_core::TransportId, ProviderEndpoint> {
        &self.bound_provider_endpoints
    }

    pub fn session_count(&self) -> Result<usize, RuntimeError> {
        Ok(self.session_registry()?.len())
    }

    pub fn is_listener_set_closed(&self) -> bool {
        self.listener_set_closed.load(Ordering::Acquire)
    }

    pub async fn accept(&self) -> Result<NnrpServerSession, RuntimeError> {
        self.accept_with_options(NnrpServerAcceptOptions::default())
            .await
    }

    pub async fn accept_with_options(
        &self,
        options: NnrpServerAcceptOptions,
    ) -> Result<NnrpServerSession, RuntimeError> {
        if options.timeout_ms == 0 {
            return self.accept_inner().await;
        }

        let accept = Box::pin(self.accept_inner());
        let timeout = Box::pin(Delay::new(std::time::Duration::from_millis(
            options.timeout_ms as u64,
        )));
        match select(accept, timeout).await {
            Either::Left((result, _)) => result,
            Either::Right(((), _)) => Err(RuntimeError::ServerAcceptTimeout),
        }
    }

    async fn accept_inner(&self) -> Result<NnrpServerSession, RuntimeError> {
        loop {
            {
                let mut accepted = self.accepted_sessions.lock().await;
                match accepted.try_recv() {
                    Ok(session) => return Ok(session),
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Closed) => {
                        return Err(RuntimeError::ServerListenerSetClosed)
                    }
                }
            }

            let queued_session = async {
                self.accepted_sessions
                    .lock()
                    .await
                    .next()
                    .await
                    .ok_or(RuntimeError::ServerListenerSetClosed)
            };
            let accepted_carrier = async {
                let mut listeners = self.listeners.lock().await;
                let Some(active) = listeners.as_ref() else {
                    self.listener_set_closed.store(true, Ordering::Release);
                    return Err(RuntimeError::ServerListenerSetClosed);
                };
                let start_index = self.next_accept_index.load(Ordering::Relaxed);
                match accept_stable(active, start_index).await {
                    Ok((accepted_index, transport_id, transport)) => {
                        self.next_accept_index
                            .store((accepted_index + 1) % active.len(), Ordering::Relaxed);
                        Ok((transport_id, transport))
                    }
                    Err(error) => {
                        listeners.take();
                        self.listener_set_closed.store(true, Ordering::Release);
                        Err(error)
                    }
                }
            };

            match select(Box::pin(queued_session), Box::pin(accepted_carrier)).await {
                Either::Left((session, _)) => return session,
                Either::Right((carrier, _)) => {
                    let (active_transport_id, transport) = carrier?;
                    let connection = MultiplexedConnection::start(transport);
                    spawn_runtime_task(serve_connection(
                        connection,
                        active_transport_id,
                        self.config.clone(),
                        Arc::clone(&self.sessions),
                        self.accepted_session_sender.clone(),
                    ));
                }
            }
        }
    }

    fn session_registry(
        &self,
    ) -> Result<MutexGuard<'_, BTreeMap<u32, RuntimeSessionRecord>>, RuntimeError> {
        self.sessions
            .lock()
            .map_err(|_| RuntimeError::Internal("server session registry lock poisoned"))
    }
}

async fn serve_connection(
    connection: MultiplexedConnection,
    active_transport_id: nnrp_core::TransportId,
    config: NnrpServerConfig,
    sessions: SharedSessionRegistry,
    accepted_sessions: mpsc::UnboundedSender<NnrpServerSession>,
) {
    let connection_nonce = NEXT_CONNECTION_NONCE.fetch_add(1, Ordering::Relaxed);
    let mut connection_session_ids = BTreeSet::new();
    if perform_server_hello(&connection, &config).await.is_err() {
        let _ = connection.close().await;
        return;
    }

    loop {
        let packet = match connection.read_control_packet().await {
            Ok(packet) => packet,
            Err(_) => break,
        };
        if packet.header.message_type == MessageType::TransportProbe {
            if respond_to_connection_probe(&connection, packet)
                .await
                .is_err()
            {
                break;
            }
            continue;
        }
        if packet.header.message_type != MessageType::SessionOpen {
            break;
        }
        let accepted_session_id = match accept_connection_session(
            &connection,
            active_transport_id,
            connection_nonce,
            &config,
            &sessions,
            &accepted_sessions,
            packet,
        )
        .await
        {
            Ok(session_id) => session_id,
            Err(_) => break,
        };
        if let Some(session_id) = accepted_session_id {
            connection_session_ids.insert(session_id);
        }
    }
    mark_connection_sessions_inactive(&sessions, connection_nonce, &connection_session_ids);
    let _ = connection.close().await;
}

async fn perform_server_hello(
    connection: &MultiplexedConnection,
    config: &NnrpServerConfig,
) -> Result<(), RuntimeError> {
    let packet = loop {
        let packet = connection.read_control_packet().await?;
        if packet.header.message_type == MessageType::TransportProbe {
            respond_to_connection_probe(connection, packet).await?;
            continue;
        }
        break packet;
    };
    if packet.header.message_type != MessageType::ClientHello {
        return Err(RuntimeError::UnexpectedMessage(
            "server expected CLIENT_HELLO before SESSION_OPEN",
        ));
    }
    if packet.metadata.len() != CLIENT_HELLO_METADATA_LEN || !packet.body.is_empty() {
        return Err(RuntimeError::UnexpectedMessage(
            "server received malformed CLIENT_HELLO",
        ));
    }
    let hello = ClientHelloMetadata::parse(&packet.metadata)?;
    let server_profiles = profile_bitmap(&config.supported_profiles)?;
    let accepted_profile_bitmap = hello.supported_profile_bitmap & server_profiles;
    if accepted_profile_bitmap == 0 {
        return Err(RuntimeError::UnexpectedMessage(
            "CLIENT_HELLO has no server-supported profile",
        ));
    }
    let server_cache_objects = cache_object_bitmap(&config.supported_cache_objects)? as u32;
    let ack = ServerHelloAckMetadata {
        selected_version_major: nnrp_core::CURRENT_VERSION_MAJOR,
        selected_wire_format: nnrp_core::CURRENT_WIRE_FORMAT,
        auth_status: 0,
        session_id: 0,
        accepted_profile_bitmap,
        accepted_payload_kind_bitmap: hello.supported_payload_kind_bitmap,
        accepted_codec_bitmap: hello.supported_codec_bitmap,
        accepted_compression_bitmap: hello.supported_compression_bitmap,
        accepted_dtype_bitmap: hello.supported_dtype_bitmap,
        accepted_layout_bitmap: hello.supported_layout_bitmap,
        cache_digest_bitmap: hello.cache_digest_bitmap as u32,
        cache_object_bitmap: hello.cache_object_bitmap as u32 & server_cache_objects,
        max_cache_entries: config.max_cache_objects.min(u64::from(u32::MAX)) as u32,
        max_cache_bytes: config.max_cache_object_bytes,
        max_lane_count: hello.max_lane_count.max(1),
        max_concurrent_frames: config.max_in_flight_operations,
        target_cadence_x100: hello.target_cadence_x100,
        latency_budget_ms: hello.latency_budget_ms,
        quality_tier: hello.quality_tier,
        degrade_policy: hello.degrade_policy,
        max_body_bytes: crate::RuntimeFrameLimits::DEFAULT_MAX_PACKET_BYTES as u32,
        token_ttl_ms: config.resume_window_ms,
        retry_after_ms: 0,
        control_extension_bytes: 0,
        server_flags: 0,
    };
    ack.validate_against_client_hello(&hello)?;
    connection
        .write_packet(RuntimePacket::new(
            CommonHeader::new(
                MessageType::ServerHelloAck,
                SERVER_HELLO_ACK_METADATA_LEN as u32,
                0,
            ),
            ack.to_bytes()?.to_vec(),
            Vec::new(),
        )?)
        .await
}

async fn accept_connection_session(
    connection: &MultiplexedConnection,
    active_transport_id: nnrp_core::TransportId,
    connection_nonce: u64,
    config: &NnrpServerConfig,
    sessions: &SharedSessionRegistry,
    accepted_sessions: &mpsc::UnboundedSender<NnrpServerSession>,
    packet: RuntimePacket,
) -> Result<Option<u32>, RuntimeError> {
    let open = SessionOpenMetadata::parse(&packet.metadata)?;
    nnrp_core::validate_session_recovery_request(&open)?;
    if packet.body.len() != open.resume_token_bytes as usize {
        return Err(RuntimeError::UnexpectedMessage(
            "SESSION_OPEN resume token length does not match body",
        ));
    }

    let validation = config.validate_client_open(&open).await;
    let validation_error = validation
        .as_ref()
        .err()
        .map(|decision| decision.session_error_code);
    let policy_diagnostic = validation
        .err()
        .and_then(|decision| decision.diagnostic)
        .unwrap_or_default();
    let resume_attempt = open.resume_token_bytes > 0;
    let now_ms = current_unix_ms();
    let mut recovery_error = None;
    let mut prior_last_operation_id = 0;
    let session_id = {
        let mut registry = sessions
            .lock()
            .map_err(|_| RuntimeError::Internal("server session registry lock poisoned"))?;
        registry.retain(|_, record| record.active || record.resume_expires_at_ms >= now_ms);
        if resume_attempt {
            match registry.get(&open.requested_session_id) {
                Some(record)
                    if record.resume_enabled
                        && !record.active
                        && record.resume_expires_at_ms >= now_ms
                        && constant_time_eq(&record.resume_token, &packet.body) =>
                {
                    prior_last_operation_id = record.last_operation_id;
                    open.requested_session_id
                }
                _ => {
                    recovery_error = Some(SESSION_ERROR_RESUME_REJECTED);
                    0
                }
            }
        } else if open.requested_session_id != 0 {
            if registry.contains_key(&open.requested_session_id) {
                recovery_error = Some(SESSION_ERROR_LIMIT_REACHED);
                0
            } else {
                open.requested_session_id
            }
        } else {
            first_available_session_id(registry.keys().copied()).unwrap_or(0)
        }
    };
    if session_id == 0 && recovery_error.is_none() {
        recovery_error = Some(SESSION_ERROR_LIMIT_REACHED);
    }
    let accepted = validation_error.is_none() && recovery_error.is_none();
    let resume_enabled = accepted
        && open.session_flags & SESSION_FLAG_ALLOW_RESUME != 0
        && config.resume_token_bytes > 0;
    let mut resume_token = if resume_enabled {
        let token_len = usize::try_from(config.resume_token_bytes).map_err(|_| {
            RuntimeError::UnexpectedMessage("configured resume token length is invalid")
        })?;
        if token_len > 4096 {
            return Err(RuntimeError::UnexpectedMessage(
                "configured resume token length exceeds runtime limit",
            ));
        }
        vec![0u8; token_len]
    } else {
        Vec::new()
    };
    if !resume_token.is_empty() {
        getrandom::fill(&mut resume_token)
            .map_err(|_| RuntimeError::Internal("operating system random source failed"))?;
    }
    let ack = SessionOpenAckMetadata {
        session_id: if accepted { session_id } else { 0 },
        accepted_profile_id: open.profile_id,
        accepted_priority_class: open.priority_class,
        session_status: if !accepted {
            SessionStatus::Rejected
        } else if resume_attempt {
            SessionStatus::Resumed
        } else {
            SessionStatus::Opened
        },
        schema_id: open.schema_id,
        schema_version: open.schema_version,
        granted_operation_credit: config.granted_operation_credit,
        max_in_flight_operations: config.max_in_flight_operations,
        lease_ttl_ms: config.lease_ttl_ms,
        resume_window_ms: config.resume_window_ms,
        resume_token_bytes: resume_token.len() as u32,
        session_extension_bytes: if accepted {
            0
        } else {
            u32::try_from(policy_diagnostic.len()).map_err(|_| {
                RuntimeError::UnexpectedMessage("server policy diagnostic exceeds wire length")
            })?
        },
        server_session_tag: if accepted { session_id as u64 } else { 0 },
        route_scope_id: 0,
        session_error_code: validation_error
            .or(recovery_error)
            .unwrap_or(SESSION_ERROR_NONE),
        session_flags_ack: if resume_enabled {
            SESSION_ACK_FLAG_RESUME_ENABLED
        } else {
            0
        },
    };

    let transport = if accepted {
        Some(connection.register_session(session_id).await?)
    } else {
        None
    };
    let ack_body = if accepted {
        resume_token.clone()
    } else {
        policy_diagnostic.into_bytes()
    };
    let mut ack_header = CommonHeader::new(
        MessageType::SessionOpenAck,
        SESSION_OPEN_ACK_METADATA_LEN as u32,
        ack_body.len() as u32,
    );
    ack_header.session_id = ack.session_id;
    connection
        .write_packet(RuntimePacket::new(
            ack_header,
            ack.to_bytes()?.to_vec(),
            ack_body,
        )?)
        .await?;
    let Some(transport) = transport else {
        return Ok(None);
    };

    let mut lifecycle = ConnectionLifecycle::new();
    lifecycle.apply_session_open_ack(&ack)?;
    sessions
        .lock()
        .map_err(|_| RuntimeError::Internal("server session registry lock poisoned"))?
        .insert(
            session_id,
            RuntimeSessionRecord {
                session_id,
                profile_id: ack.accepted_profile_id,
                schema_id: ack.schema_id,
                schema_version: ack.schema_version,
                resume_enabled,
                resume_token_bytes: ack.resume_token_bytes,
                resume_token,
                resume_expires_at_ms: now_ms.saturating_add(u64::from(config.resume_window_ms)),
                last_operation_id: prior_last_operation_id,
                active: true,
                connection_nonce,
            },
        );
    let session = NnrpServerSession {
        session_id,
        active_transport_id,
        client_open: open,
        transport: Box::new(transport),
        lifecycle,
        operations: OperationRegistry::new(),
        frame_operations: BTreeMap::new(),
        operation_frames: BTreeMap::new(),
        pressure: RuntimePressureState::default(),
        cache_objects: Vec::new(),
        supported_cache_objects: config.supported_cache_objects.clone(),
        max_cache_objects: config.max_cache_objects,
        max_cache_object_bytes: config.max_cache_object_bytes,
        connection_nonce,
        sessions: Arc::clone(sessions),
        pending_close: None,
    };
    if accepted_sessions.unbounded_send(session).is_err() {
        mark_session_inactive(sessions, session_id, connection_nonce)?;
        return Err(RuntimeError::ServerListenerSetClosed);
    }
    Ok(Some(session_id))
}

fn first_available_session_id(allocated_ids: impl IntoIterator<Item = u32>) -> Option<u32> {
    let mut candidate = 1u32;
    for allocated_id in allocated_ids {
        if allocated_id < candidate {
            continue;
        }
        if allocated_id > candidate {
            break;
        }
        candidate = candidate.checked_add(1)?;
    }
    Some(candidate)
}

fn mark_connection_sessions_inactive(
    sessions: &SharedSessionRegistry,
    connection_nonce: u64,
    session_ids: &BTreeSet<u32>,
) {
    let Ok(mut registry) = sessions.lock() else {
        return;
    };
    for session_id in session_ids {
        if let Some(record) = registry.get_mut(session_id) {
            if record.connection_nonce == connection_nonce {
                if record.resume_enabled {
                    record.active = false;
                } else {
                    registry.remove(session_id);
                }
            }
        }
    }
}

fn mark_session_inactive(
    sessions: &SharedSessionRegistry,
    session_id: u32,
    connection_nonce: u64,
) -> Result<(), RuntimeError> {
    let mut registry = sessions
        .lock()
        .map_err(|_| RuntimeError::Internal("server session registry lock poisoned"))?;
    let remove = match registry.get_mut(&session_id) {
        Some(record) if record.connection_nonce == connection_nonce => {
            record.active = false;
            !record.resume_enabled
        }
        _ => false,
    };
    if remove {
        registry.remove(&session_id);
    }
    Ok(())
}

async fn respond_to_connection_probe(
    connection: &MultiplexedConnection,
    packet: RuntimePacket,
) -> Result<(), RuntimeError> {
    let probe = TransportProbeMetadata::parse(&packet.metadata)?;
    if packet.body.len() != probe.probe_payload_bytes as usize {
        return Err(nnrp_core::NnrpError::DeclaredLengthMismatch {
            field: "transport_probe.probe_payload_bytes",
            declared: probe.probe_payload_bytes as usize,
            actual: packet.body.len(),
        }
        .into());
    }
    let ack = TransportProbeAckMetadata {
        probe_id: probe.probe_id,
        server_recv_ts_us: unix_time_us(),
    };
    connection
        .write_packet(RuntimePacket::new(
            CommonHeader::new(MessageType::TransportProbeAck, 0, 0),
            ack.to_bytes()?.to_vec(),
            Vec::new(),
        )?)
        .await
}

fn profile_bitmap(profiles: &[u16]) -> Result<u32, RuntimeError> {
    profiles.iter().try_fold(0u32, |bitmap, profile_id| {
        let bit = profile_id
            .checked_sub(1)
            .ok_or(RuntimeError::UnexpectedMessage(
                "profile id cannot map to CLIENT_HELLO bitmap",
            ))?;
        let mask = 1u32
            .checked_shl(u32::from(bit))
            .ok_or(RuntimeError::UnexpectedMessage(
                "profile id exceeds CLIENT_HELLO bitmap",
            ))?;
        Ok(bitmap | mask)
    })
}

fn cache_object_bitmap(cache_objects: &[CacheObjectKind]) -> Result<u16, RuntimeError> {
    cache_objects.iter().try_fold(0u16, |bitmap, kind| {
        let bit = (*kind as u32)
            .checked_sub(1)
            .ok_or(RuntimeError::UnexpectedMessage(
                "cache object kind cannot map to CLIENT_HELLO bitmap",
            ))?;
        let mask = 1u16
            .checked_shl(bit)
            .ok_or(RuntimeError::UnexpectedMessage(
                "cache object kind exceeds CLIENT_HELLO bitmap",
            ))?;
        Ok(bitmap | mask)
    })
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}

async fn accept_stable(
    listeners: &[BoundServerProvider],
    start_index: usize,
) -> Result<(usize, nnrp_core::TransportId, BoxedFramedTransport), RuntimeError> {
    if listeners.is_empty() {
        return Err(RuntimeError::ServerListenerSetClosed);
    }

    #[cfg(not(target_arch = "wasm32"))]
    type AcceptFuture<'a> =
        Pin<Box<dyn Future<Output = Result<BoxedFramedTransport, RuntimeError>> + Send + 'a>>;
    #[cfg(target_arch = "wasm32")]
    type AcceptFuture<'a> =
        Pin<Box<dyn Future<Output = Result<BoxedFramedTransport, RuntimeError>> + 'a>>;

    let mut accepts = listeners
        .iter()
        .map(|listener| listener.listener().accept())
        .collect::<Vec<AcceptFuture<'_>>>();
    let start_index = start_index % listeners.len();
    poll_fn(|context| {
        for offset in 0..accepts.len() {
            let index = (start_index + offset) % accepts.len();
            let accept = &mut accepts[index];
            if let Poll::Ready(result) = accept.as_mut().poll(context) {
                return Poll::Ready(
                    result.map(|transport| (index, listeners[index].transport_id(), transport)),
                );
            }
        }
        Poll::Pending
    })
    .await
}

fn unix_time_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros()
        .try_into()
        .unwrap_or(u64::MAX)
}

impl fmt::Debug for NnrpServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NnrpServer")
            .field("bound_provider_endpoints", &self.bound_provider_endpoints)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for NnrpServerSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NnrpServerSession")
            .field("session_id", &self.session_id)
            .field("active_transport_id", &self.active_transport_id)
            .field("client_open", &self.client_open)
            .field("transport", &self.transport.transport_kind())
            .field("lifecycle", &self.lifecycle)
            .field("operations", &self.operations)
            .field("pressure", &self.pressure)
            .field("cache_objects", &self.cache_objects)
            .field("max_cache_objects", &self.max_cache_objects)
            .finish_non_exhaustive()
    }
}

impl NnrpServerSession {
    pub fn session_id(&self) -> u32 {
        self.session_id
    }

    pub fn active_transport_id(&self) -> nnrp_core::TransportId {
        self.active_transport_id
    }

    pub fn client_open(&self) -> &SessionOpenMetadata {
        &self.client_open
    }

    pub fn lifecycle(&self) -> &ConnectionLifecycle {
        &self.lifecycle
    }

    pub fn operations(&self) -> &OperationRegistry {
        &self.operations
    }

    pub fn pressure_state(&self) -> RuntimePressureState {
        self.pressure
    }

    pub fn cache_object_count(&self) -> usize {
        self.cache_objects.len()
    }

    pub async fn receive_submit(&mut self) -> Result<NnrpSubmit, RuntimeError> {
        let packet = self.transport.read_packet().await?;
        self.handle_frame_submit_packet(packet)
    }

    fn handle_frame_submit_packet(
        &mut self,
        packet: RuntimePacket,
    ) -> Result<NnrpSubmit, RuntimeError> {
        if packet.header.message_type != MessageType::FrameSubmit {
            return Err(RuntimeError::UnexpectedMessage(
                "server expected FRAME_SUBMIT",
            ));
        }
        if packet.header.session_id != self.session_id {
            return Err(RuntimeError::UnexpectedMessage(
                "server received submit for another session",
            ));
        }
        if packet.metadata.len() != FRAME_SUBMIT_METADATA_LEN {
            return Err(RuntimeError::UnexpectedMessage(
                "server received malformed FRAME_SUBMIT metadata length",
            ));
        }

        let metadata = FrameSubmitMetadata::parse(&packet.metadata)?;
        if self.frame_operations.contains_key(&packet.header.frame_id) {
            return Err(RuntimeError::UnexpectedMessage(
                "server received duplicate FRAME_SUBMIT frame id",
            ));
        }
        self.operations.register(OperationDescriptor::new(
            self.session_id,
            metadata.operation_id,
        ))?;
        self.frame_operations
            .insert(packet.header.frame_id, metadata.operation_id);
        self.operation_frames
            .insert(metadata.operation_id, packet.header.frame_id);
        self.update_registry_last_operation(metadata.operation_id)?;

        Ok(NnrpSubmit {
            operation_id: metadata.operation_id,
            frame_id: packet.header.frame_id,
            metadata,
            body: packet.body,
        })
    }

    pub async fn await_event(&mut self) -> Result<NnrpRuntimeEvent, RuntimeError> {
        let packet = self.transport.read_packet().await?;
        let header = RuntimeFrameHeader::from(&packet.header);
        let event = match packet.header.message_type {
            MessageType::FrameSubmit => self
                .handle_frame_submit_packet(packet)
                .map(NnrpServerEvent::Submit),
            MessageType::FrameCancel => {
                self.require_session_packet(&packet, "server received cancel for another session")?;
                if !packet.metadata.is_empty() || !packet.body.is_empty() {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed FRAME_CANCEL lengths",
                    ));
                }
                let operation_id = self.operation_id_for_frame(packet.header.frame_id)?;
                self.operations.cancel(OperationCancelRequest {
                    session_id: self.session_id,
                    operation_id,
                    cancel_scope: nnrp_core::CancelScope::Operation,
                })?;
                Ok(NnrpServerEvent::FrameCancel(NnrpCancel {
                    frame_id: packet.header.frame_id,
                }))
            }
            MessageType::PartialResult => {
                self.require_session_packet(
                    &packet,
                    "server received partial result for another session",
                )?;
                if packet.metadata.len() != PARTIAL_RESULT_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed PARTIAL_RESULT metadata length",
                    ));
                }
                let metadata = PartialResultMetadata::parse(&packet.metadata)?;
                validate_partial_result_semantics(&metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.body_bytes as usize,
                    "server received PARTIAL_RESULT body length mismatch",
                )?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpServerEvent::PartialResult {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::Progress => {
                self.require_session_packet(
                    &packet,
                    "server received progress for another session",
                )?;
                if packet.metadata.len() != PROGRESS_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed PROGRESS metadata length",
                    ));
                }
                let metadata = ProgressMetadata::parse(&packet.metadata)?;
                validate_progress_semantics(&metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.body_bytes as usize,
                    "server received PROGRESS body length mismatch",
                )?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpServerEvent::Progress {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::ResultDropReason => {
                self.require_session_packet(
                    &packet,
                    "server received drop reason for another session",
                )?;
                if packet.metadata.len() != RESULT_DROP_REASON_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed RESULT_DROP_REASON metadata length",
                    ));
                }
                let metadata = ResultDropReasonMetadata::parse(&packet.metadata)?;
                validate_result_drop_reason_semantics(&metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "server received RESULT_DROP_REASON body length mismatch",
                )?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpServerEvent::ResultDropReason {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::Cancel | MessageType::Abort => {
                self.require_session_packet(
                    &packet,
                    "server received control for another session",
                )?;
                if packet.metadata.len() != CONTROL_REQUEST_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed runtime control lengths",
                    ));
                }
                let metadata = ControlRequestMetadata::parse(&packet.metadata)?;
                validate_control_request_semantics(packet.header.message_type, &metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "server received runtime control diagnostic body length mismatch",
                )?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                match packet.header.message_type {
                    MessageType::Cancel => {
                        self.operations.cancel(OperationCancelRequest {
                            session_id: self.session_id,
                            operation_id: metadata.operation_id,
                            cancel_scope: nnrp_core::CancelScope::Operation,
                        })?;
                    }
                    MessageType::Abort => self.operations.abort(metadata.operation_id)?,
                    _ => unreachable!("runtime control message type was matched earlier"),
                }
                Ok(NnrpServerEvent::Control(NnrpRuntimeControl {
                    message_type: packet.header.message_type,
                    metadata,
                    body: packet.body,
                }))
            }
            MessageType::PriorityUpdate | MessageType::Deadline | MessageType::ExpireAt => {
                self.require_session_packet(
                    &packet,
                    "server received scheduling update for another session",
                )?;
                if packet.metadata.len() != SCHEDULING_METADATA_LEN || !packet.body.is_empty() {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed scheduling metadata length",
                    ));
                }
                let metadata = SchedulingMetadata::parse(&packet.metadata)?;
                validate_scheduling_semantics(packet.header.message_type, &metadata)?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                self.operations.apply_scheduling_update(
                    self.session_id,
                    packet.header.message_type,
                    metadata,
                )?;
                Ok(NnrpServerEvent::Scheduling(NnrpSchedulingUpdate {
                    message_type: packet.header.message_type,
                    metadata,
                }))
            }
            MessageType::Supersede => {
                self.require_session_packet(
                    &packet,
                    "server received supersede for another session",
                )?;
                if packet.metadata.len() != SUPERSEDE_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed SUPERSEDE metadata length",
                    ));
                }
                let metadata = SupersedeMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "server received SUPERSEDE diagnostic body length mismatch",
                )?;
                self.require_operation_frame(metadata.old_operation_id, packet.header.frame_id)?;
                Ok(NnrpServerEvent::Supersede {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::BudgetUpdate => {
                self.require_session_packet(
                    &packet,
                    "server received budget update for another session",
                )?;
                if packet.metadata.len() != BUDGET_METADATA_LEN || !packet.body.is_empty() {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed BUDGET_UPDATE lengths",
                    ));
                }
                let metadata = BudgetMetadata::parse(&packet.metadata)?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpServerEvent::Budget(metadata))
            }
            MessageType::FlowUpdate => {
                if packet.metadata.len() != FLOW_UPDATE_METADATA_LEN || !packet.body.is_empty() {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed FLOW_UPDATE lengths",
                    ));
                }
                let metadata = FlowUpdateMetadata::parse(&packet.metadata)?;
                self.lifecycle
                    .validate_flow_update(&packet.header, &metadata)?;
                Ok(NnrpServerEvent::FlowUpdate(metadata))
            }
            MessageType::Backpressure | MessageType::CreditUpdate => {
                self.require_optional_session_packet(
                    &packet,
                    "server received pressure update for another session",
                )?;
                if packet.metadata.len() != PRESSURE_METADATA_LEN || !packet.body.is_empty() {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed pressure metadata length",
                    ));
                }
                let metadata = PressureMetadata::parse(&packet.metadata)?;
                validate_pressure_semantics(packet.header.message_type, &metadata)?;
                self.pressure
                    .apply_inbound(packet.header.message_type, metadata)?;
                Ok(NnrpServerEvent::Pressure(NnrpPressureUpdate {
                    message_type: packet.header.message_type,
                    metadata,
                }))
            }
            MessageType::CapabilityNegotiation | MessageType::DegradeProfile => {
                self.require_optional_session_packet(
                    &packet,
                    "server received capability update for another session",
                )?;
                if packet.metadata.len() != CAPABILITY_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed capability metadata length",
                    ));
                }
                let metadata = CapabilityMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.body_bytes as usize,
                    "server received capability body length mismatch",
                )?;
                Ok(NnrpServerEvent::Capability {
                    message_type: packet.header.message_type,
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::RouteHint | MessageType::ExecutionHint => {
                self.require_optional_session_packet(
                    &packet,
                    "server received route hint for another session",
                )?;
                if packet.metadata.len() != ROUTE_HINT_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed route hint metadata length",
                    ));
                }
                let metadata = RouteHintMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.body_bytes as usize,
                    "server received route hint body length mismatch",
                )?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpServerEvent::RouteHint {
                    message_type: packet.header.message_type,
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::TraceContext => {
                self.require_optional_session_packet(
                    &packet,
                    "server received trace context for another session",
                )?;
                if packet.metadata.len() != TRACE_CONTEXT_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed TRACE_CONTEXT metadata length",
                    ));
                }
                let metadata = TraceContextMetadata::parse(&packet.metadata)?;
                validate_trace_context_semantics(&metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.body_bytes as usize,
                    "server received TRACE_CONTEXT body length mismatch",
                )?;
                Ok(NnrpServerEvent::TraceContext {
                    frame_id: packet.header.frame_id,
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::ErrorRecoverable => {
                self.require_optional_session_packet(
                    &packet,
                    "server received recoverable error for another session",
                )?;
                if packet.metadata.len() != RECOVERABLE_ERROR_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed ERROR_RECOVERABLE metadata length",
                    ));
                }
                let metadata = RecoverableErrorMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "server received ERROR_RECOVERABLE diagnostic body length mismatch",
                )?;
                Ok(NnrpServerEvent::RecoverableError {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::RetryAfter => {
                self.require_optional_session_packet(
                    &packet,
                    "server received retry-after for another session",
                )?;
                if packet.metadata.len() != RETRY_AFTER_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed RETRY_AFTER metadata length",
                    ));
                }
                let metadata = RetryAfterMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "server received RETRY_AFTER diagnostic body length mismatch",
                )?;
                Ok(NnrpServerEvent::RetryAfter {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::ObjectDeclare => {
                self.require_session_packet(
                    &packet,
                    "server received object declaration for another session",
                )?;
                if packet.metadata.len() != OBJECT_DESCRIPTOR_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed OBJECT_DECLARE metadata length",
                    ));
                }
                let metadata = ObjectDescriptorMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.metadata_bytes as usize,
                    "server received OBJECT_DECLARE body length mismatch",
                )?;
                Ok(NnrpServerEvent::ObjectDeclare {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::ObjectRef => {
                self.require_session_packet(
                    &packet,
                    "server received object reference for another session",
                )?;
                if packet.metadata.len() != OBJECT_REFERENCE_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed OBJECT_REF metadata length",
                    ));
                }
                let metadata = ObjectReferenceMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.metadata_bytes as usize,
                    "server received OBJECT_REF body length mismatch",
                )?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpServerEvent::ObjectRef {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::ObjectRelease => {
                self.require_session_packet(
                    &packet,
                    "server received object release for another session",
                )?;
                if packet.metadata.len() != OBJECT_RELEASE_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed OBJECT_RELEASE metadata length",
                    ));
                }
                let metadata = ObjectReleaseMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "server received OBJECT_RELEASE body length mismatch",
                )?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpServerEvent::ObjectRelease {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::ObjectPatch | MessageType::ObjectDelta => {
                self.require_session_packet(
                    &packet,
                    "server received object delta for another session",
                )?;
                if packet.metadata.len() != OBJECT_DELTA_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed object delta metadata length",
                    ));
                }
                let metadata = ObjectDeltaMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.metadata_bytes.saturating_add(metadata.delta_bytes) as usize,
                    "server received object delta body length mismatch",
                )?;
                Ok(NnrpServerEvent::ObjectDelta {
                    message_type: packet.header.message_type,
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::CacheReference => {
                self.require_session_packet(
                    &packet,
                    "server received cache reference for another session",
                )?;
                if packet.metadata.len() != CACHE_REFERENCE_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed CACHE_REFERENCE metadata length",
                    ));
                }
                let metadata = CacheReferenceMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.metadata_bytes as usize,
                    "server received CACHE_REFERENCE body length mismatch",
                )?;
                Ok(NnrpServerEvent::CacheReference {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::CacheMiss => {
                self.require_session_packet(
                    &packet,
                    "server received cache miss for another session",
                )?;
                if packet.metadata.len() != CACHE_MISS_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed CACHE_MISS metadata length",
                    ));
                }
                let metadata = CacheMissMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "server received CACHE_MISS diagnostic body length mismatch",
                )?;
                Ok(NnrpServerEvent::CacheMiss {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::CacheInvalidate => {
                self.require_session_packet(
                    &packet,
                    "server received cache invalidate for another session",
                )?;
                if packet.metadata.len() != CACHE_INVALIDATE_METADATA_LEN || !packet.body.is_empty()
                {
                    return Err(RuntimeError::UnexpectedMessage(
                        "server received malformed CACHE_INVALIDATE lengths",
                    ));
                }
                Ok(NnrpServerEvent::CacheInvalidate(
                    CacheInvalidateMetadata::parse(&packet.metadata)?,
                ))
            }
            MessageType::SessionClose => {
                self.require_session_packet(&packet, "server received close for another session")?;
                let metadata = SessionCloseMetadata::parse(&packet.metadata)?;
                self.lifecycle
                    .begin_session_close(&packet.header, &metadata)?;
                self.pending_close = Some(metadata);
                Ok(NnrpServerEvent::Close(metadata))
            }
            _ => Err(RuntimeError::UnexpectedMessage(
                "server expected a submit, control, object, cache, or close event",
            )),
        }?;
        Ok(NnrpRuntimeEvent::from_server(header, event))
    }

    pub async fn send_result(
        &mut self,
        frame_id: u32,
        metadata: ResultPushMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        let operation_id = self.operation_id_for_frame(frame_id)?;
        if let Some(schedule) = self
            .operations
            .expire_if_stale(operation_id, current_unix_ms())?
        {
            if schedule.flags & SCHEDULING_FLAG_EMIT_DROP_REASON != 0 {
                self.send_result_drop_reason(ResultDropReasonMetadata {
                    operation_id,
                    result_sequence: schedule.update_sequence,
                    drop_reason_code: RESULT_DROP_REASON_DEADLINE_EXPIRED,
                    source_role: RuntimeRole::Server as u8,
                    flags: 0,
                    diagnostic_bytes: 0,
                })
                .await?;
            }
            return Err(nnrp_core::NnrpError::InvalidOperationTransition {
                from: nnrp_core::OperationState::Superseded,
                to: nnrp_core::OperationState::Completed,
            }
            .into());
        }
        self.operations.complete(operation_id)?;
        let mut header = CommonHeader::new(
            MessageType::ResultPush,
            RESULT_PUSH_METADATA_LEN as u32,
            body.len() as u32,
        );
        header.session_id = self.session_id;
        header.frame_id = frame_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                body,
            )?)
            .await?;
        Ok(())
    }

    pub async fn send_result_drop(&mut self, frame_id: u32) -> Result<(), RuntimeError> {
        self.operation_id_for_frame(frame_id)?;
        let mut header = CommonHeader::new(MessageType::ResultDrop, 0, 0);
        header.session_id = self.session_id;
        header.frame_id = frame_id;
        validate_result_drop_header(&header)?;
        self.transport
            .write_packet(&RuntimePacket::new(header, Vec::new(), Vec::new())?)
            .await?;
        Ok(())
    }

    pub async fn send_partial_result(
        &mut self,
        metadata: PartialResultMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        validate_partial_result_semantics(&metadata)?;
        if metadata.body_bytes as usize != body.len() {
            return Err(RuntimeError::UnexpectedMessage(
                "server PARTIAL_RESULT body length mismatch",
            ));
        }
        let mut header = CommonHeader::new(
            MessageType::PartialResult,
            PARTIAL_RESULT_METADATA_LEN as u32,
            body.len() as u32,
        );
        header.session_id = self.session_id;
        header.frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                body,
            )?)
            .await
    }

    pub async fn send_progress(
        &mut self,
        metadata: ProgressMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        validate_progress_semantics(&metadata)?;
        if metadata.body_bytes as usize != body.len() {
            return Err(RuntimeError::UnexpectedMessage(
                "server PROGRESS body length mismatch",
            ));
        }
        let mut header = CommonHeader::new(
            MessageType::Progress,
            PROGRESS_METADATA_LEN as u32,
            body.len() as u32,
        );
        header.session_id = self.session_id;
        header.frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                body,
            )?)
            .await
    }

    pub async fn send_result_drop_reason(
        &mut self,
        metadata: ResultDropReasonMetadata,
    ) -> Result<(), RuntimeError> {
        self.send_result_drop_reason_with_diagnostics(metadata, Vec::new())
            .await
    }

    pub async fn send_result_drop_reason_with_diagnostics(
        &mut self,
        metadata: ResultDropReasonMetadata,
        diagnostics: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        validate_result_drop_reason_semantics(&metadata)?;
        require_body_len(
            diagnostics.len(),
            metadata.diagnostic_bytes as usize,
            "server RESULT_DROP_REASON diagnostic body length mismatch",
        )?;
        let mut header = CommonHeader::new(
            MessageType::ResultDropReason,
            RESULT_DROP_REASON_METADATA_LEN as u32,
            diagnostics.len() as u32,
        );
        header.session_id = self.session_id;
        header.frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                diagnostics,
            )?)
            .await
    }

    pub async fn send_control_request(
        &mut self,
        message_type: MessageType,
        metadata: ControlRequestMetadata,
        diagnostics: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        validate_control_request_semantics(message_type, &metadata)?;
        require_body_len(
            diagnostics.len(),
            metadata.diagnostic_bytes as usize,
            "server runtime control diagnostic body length mismatch",
        )?;
        let frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.write_runtime_packet(
            message_type,
            frame_id,
            metadata.to_bytes()?.to_vec(),
            diagnostics,
        )
        .await
    }

    pub async fn send_scheduling_update(
        &mut self,
        message_type: MessageType,
        metadata: SchedulingMetadata,
    ) -> Result<(), RuntimeError> {
        validate_scheduling_semantics(message_type, &metadata)?;
        let frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.write_runtime_packet(
            message_type,
            frame_id,
            metadata.to_bytes()?.to_vec(),
            Vec::new(),
        )
        .await
    }

    pub async fn supersede_operation(
        &mut self,
        metadata: SupersedeMetadata,
        diagnostics: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        require_body_len(
            diagnostics.len(),
            metadata.diagnostic_bytes as usize,
            "server SUPERSEDE diagnostic body length mismatch",
        )?;
        let frame_id = self.correlated_frame_id(metadata.old_operation_id)?;
        self.write_runtime_packet(
            MessageType::Supersede,
            frame_id,
            metadata.to_bytes()?.to_vec(),
            diagnostics,
        )
        .await
    }

    pub async fn update_budget(&mut self, metadata: BudgetMetadata) -> Result<(), RuntimeError> {
        let frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.write_runtime_packet(
            MessageType::BudgetUpdate,
            frame_id,
            metadata.to_bytes()?.to_vec(),
            Vec::new(),
        )
        .await
    }

    pub async fn send_capability(
        &mut self,
        message_type: MessageType,
        metadata: CapabilityMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        if !matches!(
            message_type,
            MessageType::CapabilityNegotiation | MessageType::DegradeProfile
        ) {
            return Err(RuntimeError::UnexpectedMessage(
                "server capability send requires CAPABILITY_NEGOTIATION or DEGRADE_PROFILE",
            ));
        }
        require_body_len(
            body.len(),
            metadata.body_bytes as usize,
            "server capability body length mismatch",
        )?;
        let mut header = CommonHeader::new(
            message_type,
            CAPABILITY_METADATA_LEN as u32,
            body.len() as u32,
        );
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                body,
            )?)
            .await
    }

    pub async fn send_route_hint(
        &mut self,
        message_type: MessageType,
        metadata: RouteHintMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        if !matches!(
            message_type,
            MessageType::RouteHint | MessageType::ExecutionHint
        ) {
            return Err(RuntimeError::UnexpectedMessage(
                "server route hint send requires ROUTE_HINT or EXECUTION_HINT",
            ));
        }
        require_body_len(
            body.len(),
            metadata.body_bytes as usize,
            "server route hint body length mismatch",
        )?;
        let mut header = CommonHeader::new(
            message_type,
            ROUTE_HINT_METADATA_LEN as u32,
            body.len() as u32,
        );
        header.session_id = self.session_id;
        header.frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                body,
            )?)
            .await
    }

    pub async fn send_object_declare(
        &mut self,
        metadata: ObjectDescriptorMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        require_body_len(
            body.len(),
            metadata.metadata_bytes as usize,
            "server OBJECT_DECLARE body length mismatch",
        )?;
        let mut header = CommonHeader::new(
            MessageType::ObjectDeclare,
            OBJECT_DESCRIPTOR_METADATA_LEN as u32,
            body.len() as u32,
        );
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                body,
            )?)
            .await
    }

    pub async fn send_object_ref(
        &mut self,
        metadata: ObjectReferenceMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        require_body_len(
            body.len(),
            metadata.metadata_bytes as usize,
            "server OBJECT_REF body length mismatch",
        )?;
        let mut header = CommonHeader::new(
            MessageType::ObjectRef,
            OBJECT_REFERENCE_METADATA_LEN as u32,
            body.len() as u32,
        );
        header.session_id = self.session_id;
        header.frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                body,
            )?)
            .await
    }

    pub async fn send_object_release(
        &mut self,
        metadata: ObjectReleaseMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        require_body_len(
            body.len(),
            metadata.diagnostic_bytes as usize,
            "server OBJECT_RELEASE body length mismatch",
        )?;
        let mut header = CommonHeader::new(
            MessageType::ObjectRelease,
            OBJECT_RELEASE_METADATA_LEN as u32,
            body.len() as u32,
        );
        header.session_id = self.session_id;
        header.frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                body,
            )?)
            .await
    }

    pub async fn send_object_delta(
        &mut self,
        message_type: MessageType,
        metadata: ObjectDeltaMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        if !matches!(
            message_type,
            MessageType::ObjectPatch | MessageType::ObjectDelta
        ) {
            return Err(RuntimeError::UnexpectedMessage(
                "server object delta send requires OBJECT_PATCH or OBJECT_DELTA",
            ));
        }
        let expected_body_len =
            metadata.metadata_bytes.saturating_add(metadata.delta_bytes) as usize;
        require_body_len(
            body.len(),
            expected_body_len,
            "server object delta body length mismatch",
        )?;
        let mut header = CommonHeader::new(
            message_type,
            OBJECT_DELTA_METADATA_LEN as u32,
            body.len() as u32,
        );
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                body,
            )?)
            .await
    }

    pub async fn send_cache_put(
        &mut self,
        metadata: CachePutMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        self.validate_cache_put(&metadata)?;
        self.track_cache_object(CacheObjectId::from_put(&metadata))?;
        require_body_len(
            body.len(),
            metadata.object_bytes as usize,
            "server CACHE_PUT body length mismatch",
        )?;
        let mut header = CommonHeader::new(
            MessageType::CachePut,
            CACHE_PUT_METADATA_LEN as u32,
            body.len() as u32,
        );
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                body,
            )?)
            .await
    }

    pub async fn send_cache_ack(&mut self, metadata: CacheAckMetadata) -> Result<(), RuntimeError> {
        let mut header = CommonHeader::new(MessageType::CacheAck, CACHE_ACK_METADATA_LEN as u32, 0);
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await
    }

    pub async fn receive_cache_put(&mut self) -> Result<(CachePutMetadata, Vec<u8>), RuntimeError> {
        let packet = self.transport.read_packet().await?;
        self.require_session_packet(&packet, "server received cache put for another session")?;
        if packet.header.message_type != MessageType::CachePut
            || packet.metadata.len() != CACHE_PUT_METADATA_LEN
        {
            return Err(RuntimeError::UnexpectedMessage(
                "server expected a well-formed CACHE_PUT",
            ));
        }
        let metadata = CachePutMetadata::parse(&packet.metadata)?;
        self.validate_cache_put(&metadata)?;
        require_body_len(
            packet.body.len(),
            metadata.object_bytes as usize,
            "server received CACHE_PUT body length mismatch",
        )?;
        self.track_cache_object(CacheObjectId::from_put(&metadata))?;
        Ok((metadata, packet.body))
    }

    pub async fn receive_cache_ack(&mut self) -> Result<CacheAckMetadata, RuntimeError> {
        let packet = self.transport.read_packet().await?;
        self.require_session_packet(&packet, "server received cache ack for another session")?;
        if packet.header.message_type != MessageType::CacheAck
            || packet.metadata.len() != CACHE_ACK_METADATA_LEN
            || !packet.body.is_empty()
        {
            return Err(RuntimeError::UnexpectedMessage(
                "server expected a well-formed CACHE_ACK",
            ));
        }
        Ok(CacheAckMetadata::parse(&packet.metadata)?)
    }

    pub async fn send_ping(&mut self) -> Result<(), RuntimeError> {
        self.write_empty_role_message(MessageType::Ping).await
    }

    pub async fn send_pong(&mut self) -> Result<(), RuntimeError> {
        self.write_empty_role_message(MessageType::Pong).await
    }

    pub async fn receive_ping(&mut self) -> Result<(), RuntimeError> {
        self.receive_empty_role_message(MessageType::Ping, "server expected PING")
            .await
    }

    pub async fn receive_pong(&mut self) -> Result<(), RuntimeError> {
        self.receive_empty_role_message(MessageType::Pong, "server expected PONG")
            .await
    }

    async fn write_empty_role_message(
        &mut self,
        message_type: MessageType,
    ) -> Result<(), RuntimeError> {
        let mut header = CommonHeader::new(message_type, 0, 0);
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(header, Vec::new(), Vec::new())?)
            .await
    }

    async fn receive_empty_role_message(
        &mut self,
        expected: MessageType,
        error: &'static str,
    ) -> Result<(), RuntimeError> {
        let packet = self.transport.read_packet().await?;
        self.require_session_packet(&packet, error)?;
        if packet.header.message_type != expected
            || !packet.metadata.is_empty()
            || !packet.body.is_empty()
        {
            return Err(RuntimeError::UnexpectedMessage(error));
        }
        Ok(())
    }

    pub async fn send_cache_reference(
        &mut self,
        metadata: CacheReferenceMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        require_body_len(
            body.len(),
            metadata.metadata_bytes as usize,
            "server CACHE_REFERENCE body length mismatch",
        )?;
        let mut header = CommonHeader::new(
            MessageType::CacheReference,
            CACHE_REFERENCE_METADATA_LEN as u32,
            body.len() as u32,
        );
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                body,
            )?)
            .await
    }

    pub async fn send_cache_miss(
        &mut self,
        metadata: CacheMissMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        require_body_len(
            body.len(),
            metadata.diagnostic_bytes as usize,
            "server CACHE_MISS body length mismatch",
        )?;
        let mut header = CommonHeader::new(
            MessageType::CacheMiss,
            CACHE_MISS_METADATA_LEN as u32,
            body.len() as u32,
        );
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                body,
            )?)
            .await
    }

    pub async fn send_cache_invalidate(
        &mut self,
        metadata: CacheInvalidateMetadata,
    ) -> Result<(), RuntimeError> {
        let mut header = CommonHeader::new(
            MessageType::CacheInvalidate,
            CACHE_INVALIDATE_METADATA_LEN as u32,
            0,
        );
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await
    }

    pub async fn receive_cancel(&mut self) -> Result<NnrpCancel, RuntimeError> {
        let packet = self.transport.read_packet().await?;
        if packet.header.message_type != MessageType::FrameCancel {
            return Err(RuntimeError::UnexpectedMessage(
                "server expected FRAME_CANCEL",
            ));
        }
        self.require_session_packet(&packet, "server received cancel for another session")?;
        if packet.header.meta_len != 0 || packet.header.body_len != 0 {
            return Err(RuntimeError::UnexpectedMessage(
                "server received malformed FRAME_CANCEL lengths",
            ));
        }
        let operation_id = self.operation_id_for_frame(packet.header.frame_id)?;
        self.operations.cancel(OperationCancelRequest {
            session_id: self.session_id,
            operation_id,
            cancel_scope: nnrp_core::CancelScope::Operation,
        })?;
        Ok(NnrpCancel {
            frame_id: packet.header.frame_id,
        })
    }

    pub async fn receive_runtime_control(&mut self) -> Result<NnrpRuntimeControl, RuntimeError> {
        let packet = self.transport.read_packet().await?;
        if !matches!(
            packet.header.message_type,
            MessageType::Cancel | MessageType::Abort
        ) {
            return Err(RuntimeError::UnexpectedMessage(
                "server expected CANCEL or ABORT",
            ));
        }
        self.require_session_packet(&packet, "server received control for another session")?;
        if packet.metadata.len() != CONTROL_REQUEST_METADATA_LEN {
            return Err(RuntimeError::UnexpectedMessage(
                "server received malformed runtime control lengths",
            ));
        }

        let metadata = ControlRequestMetadata::parse(&packet.metadata)?;
        validate_control_request_semantics(packet.header.message_type, &metadata)?;
        require_body_len(
            packet.body.len(),
            metadata.diagnostic_bytes as usize,
            "server received runtime control diagnostic body length mismatch",
        )?;
        self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
        match packet.header.message_type {
            MessageType::Cancel => {
                self.operations.cancel(OperationCancelRequest {
                    session_id: self.session_id,
                    operation_id: metadata.operation_id,
                    cancel_scope: nnrp_core::CancelScope::Operation,
                })?;
            }
            MessageType::Abort => {
                self.operations.abort(metadata.operation_id)?;
            }
            _ => unreachable!("runtime control message type was validated earlier"),
        }
        Ok(NnrpRuntimeControl {
            message_type: packet.header.message_type,
            metadata,
            body: packet.body,
        })
    }

    pub async fn receive_scheduling_update(
        &mut self,
    ) -> Result<NnrpSchedulingUpdate, RuntimeError> {
        let packet = self.transport.read_packet().await?;
        if !matches!(
            packet.header.message_type,
            MessageType::PriorityUpdate | MessageType::Deadline | MessageType::ExpireAt
        ) {
            return Err(RuntimeError::UnexpectedMessage(
                "server expected PRIORITY_UPDATE, DEADLINE, or EXPIRE_AT",
            ));
        }
        self.require_session_packet(
            &packet,
            "server received scheduling update for another session",
        )?;
        if packet.metadata.len() != SCHEDULING_METADATA_LEN || !packet.body.is_empty() {
            return Err(RuntimeError::UnexpectedMessage(
                "server received malformed scheduling metadata length",
            ));
        }

        let metadata = SchedulingMetadata::parse(&packet.metadata)?;
        validate_scheduling_semantics(packet.header.message_type, &metadata)?;
        self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
        self.operations.apply_scheduling_update(
            self.session_id,
            packet.header.message_type,
            metadata,
        )?;
        Ok(NnrpSchedulingUpdate {
            message_type: packet.header.message_type,
            metadata,
        })
    }

    pub async fn receive_pressure_update(&mut self) -> Result<NnrpPressureUpdate, RuntimeError> {
        let packet = self.transport.read_packet().await?;
        if !matches!(
            packet.header.message_type,
            MessageType::Backpressure | MessageType::CreditUpdate
        ) {
            return Err(RuntimeError::UnexpectedMessage(
                "server expected BACKPRESSURE or CREDIT_UPDATE",
            ));
        }
        self.require_optional_session_packet(
            &packet,
            "server received pressure update for another session",
        )?;
        if packet.metadata.len() != PRESSURE_METADATA_LEN || !packet.body.is_empty() {
            return Err(RuntimeError::UnexpectedMessage(
                "server received malformed pressure metadata length",
            ));
        }

        let metadata = PressureMetadata::parse(&packet.metadata)?;
        validate_pressure_semantics(packet.header.message_type, &metadata)?;
        self.pressure
            .apply_inbound(packet.header.message_type, metadata)?;
        Ok(NnrpPressureUpdate {
            message_type: packet.header.message_type,
            metadata,
        })
    }

    pub async fn send_backpressure(
        &mut self,
        metadata: PressureMetadata,
    ) -> Result<(), RuntimeError> {
        validate_pressure_semantics(MessageType::Backpressure, &metadata)?;
        self.pressure
            .apply_outbound(MessageType::Backpressure, metadata)?;
        let mut header =
            CommonHeader::new(MessageType::Backpressure, PRESSURE_METADATA_LEN as u32, 0);
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await
    }

    pub async fn send_credit_update(
        &mut self,
        metadata: PressureMetadata,
    ) -> Result<(), RuntimeError> {
        validate_pressure_semantics(MessageType::CreditUpdate, &metadata)?;
        self.pressure
            .apply_outbound(MessageType::CreditUpdate, metadata)?;
        self.write_runtime_packet(
            MessageType::CreditUpdate,
            0,
            metadata.to_bytes()?.to_vec(),
            Vec::new(),
        )
        .await
    }

    pub async fn send_trace_context(
        &mut self,
        frame_id: u32,
        metadata: TraceContextMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        validate_trace_context_semantics(&metadata)?;
        require_body_len(
            body.len(),
            metadata.body_bytes as usize,
            "server trace context body length mismatch",
        )?;
        self.write_runtime_packet(
            MessageType::TraceContext,
            frame_id,
            metadata.to_bytes()?.to_vec(),
            body,
        )
        .await
    }

    pub async fn send_recoverable_error(
        &mut self,
        metadata: RecoverableErrorMetadata,
        diagnostics: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        require_body_len(
            diagnostics.len(),
            metadata.diagnostic_bytes as usize,
            "server recoverable error diagnostic body length mismatch",
        )?;
        self.write_runtime_packet(
            MessageType::ErrorRecoverable,
            metadata.related_frame_id,
            metadata.to_bytes()?.to_vec(),
            diagnostics,
        )
        .await
    }

    pub async fn send_retry_after(
        &mut self,
        metadata: RetryAfterMetadata,
        diagnostics: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        require_body_len(
            diagnostics.len(),
            metadata.diagnostic_bytes as usize,
            "server retry-after diagnostic body length mismatch",
        )?;
        self.write_runtime_packet(
            MessageType::RetryAfter,
            0,
            metadata.to_bytes()?.to_vec(),
            diagnostics,
        )
        .await
    }

    pub async fn send_result_hint(
        &mut self,
        metadata: ResultHintMetadata,
    ) -> Result<(), RuntimeError> {
        self.write_runtime_packet(
            MessageType::ResultHint,
            0,
            metadata.to_bytes()?.to_vec(),
            Vec::new(),
        )
        .await
    }

    async fn write_runtime_packet(
        &mut self,
        message_type: MessageType,
        frame_id: u32,
        metadata: Vec<u8>,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        let mut header = CommonHeader::new(message_type, metadata.len() as u32, body.len() as u32);
        header.session_id = self.session_id;
        header.frame_id = frame_id;
        self.transport
            .write_packet(&RuntimePacket::new(header, metadata, body)?)
            .await
    }

    pub fn track_cache_object(&mut self, object_id: CacheObjectId) -> Result<(), RuntimeError> {
        if self.cache_objects.contains(&object_id) {
            return Ok(());
        }
        if self.max_cache_objects != 0 && self.cache_objects.len() as u64 >= self.max_cache_objects
        {
            return Err(RuntimeError::UnexpectedMessage(
                "server cache object limit reached",
            ));
        }
        self.cache_objects.push(object_id);
        Ok(())
    }

    fn validate_cache_put(&self, metadata: &CachePutMetadata) -> Result<(), RuntimeError> {
        if !self.supported_cache_objects.contains(&metadata.object_kind) {
            return Err(RuntimeError::UnexpectedMessage(
                "CACHE_PUT object kind was not negotiated in CLIENT_HELLO",
            ));
        }
        if self.max_cache_object_bytes != 0 && metadata.object_bytes > self.max_cache_object_bytes {
            return Err(RuntimeError::UnexpectedMessage(
                "CACHE_PUT object exceeds the negotiated byte limit",
            ));
        }
        Ok(())
    }

    pub async fn receive_patch(&mut self) -> Result<SessionPatchMetadata, RuntimeError> {
        let packet = self.transport.read_packet().await?;
        if packet.header.message_type != MessageType::SessionPatch {
            return Err(RuntimeError::UnexpectedMessage(
                "server expected SESSION_PATCH",
            ));
        }
        self.require_session_packet(&packet, "server received patch for another session")?;
        if packet.metadata.len() != SESSION_PATCH_METADATA_LEN {
            return Err(RuntimeError::UnexpectedMessage(
                "server received malformed SESSION_PATCH metadata length",
            ));
        }
        Ok(SessionPatchMetadata::parse(&packet.metadata)?)
    }

    pub async fn send_patch_ack(
        &mut self,
        ack: SessionPatchAckMetadata,
    ) -> Result<(), RuntimeError> {
        let mut header = CommonHeader::new(
            MessageType::SessionPatchAck,
            SESSION_PATCH_ACK_METADATA_LEN as u32,
            ack.profile_patch_ack_bytes,
        );
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                ack.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await
    }

    pub async fn send_flow_update(
        &mut self,
        metadata: FlowUpdateMetadata,
    ) -> Result<(), RuntimeError> {
        let mut header =
            CommonHeader::new(MessageType::FlowUpdate, FLOW_UPDATE_METADATA_LEN as u32, 0);
        if !matches!(metadata.scope_kind, nnrp_core::FlowScopeKind::Connection) {
            header.session_id = self.session_id;
        }
        metadata.validate_routing(&header)?;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await
    }

    pub async fn receive_migrate(&mut self) -> Result<NnrpMigration, RuntimeError> {
        let packet = self.transport.read_packet().await?;
        if packet.header.message_type != MessageType::SessionMigrate {
            return Err(RuntimeError::UnexpectedMessage(
                "server expected SESSION_MIGRATE",
            ));
        }
        self.require_session_packet(&packet, "server received migrate for another session")?;
        if packet.metadata.len() != SESSION_MIGRATE_METADATA_LEN {
            return Err(RuntimeError::UnexpectedMessage(
                "server received malformed SESSION_MIGRATE metadata length",
            ));
        }
        Ok(NnrpMigration {
            metadata: SessionMigrateMetadata::parse(&packet.metadata)?,
        })
    }

    pub async fn send_migrate_ack(
        &mut self,
        request: &SessionMigrateMetadata,
        ack: SessionMigrateAckMetadata,
    ) -> Result<(), RuntimeError> {
        nnrp_core::validate_migration_recovery(request, &ack)?;
        let mut header = CommonHeader::new(
            MessageType::SessionMigrateAck,
            SESSION_MIGRATE_ACK_METADATA_LEN as u32,
            0,
        );
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                ack.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await
    }

    pub async fn receive_close(&mut self) -> Result<SessionCloseMetadata, RuntimeError> {
        let packet = self.transport.read_packet().await?;
        if packet.header.message_type != MessageType::SessionClose {
            return Err(RuntimeError::UnexpectedMessage(
                "server expected SESSION_CLOSE",
            ));
        }
        if packet.header.session_id != self.session_id {
            return Err(RuntimeError::UnexpectedMessage(
                "server received close for another session",
            ));
        }
        let close = SessionCloseMetadata::parse(&packet.metadata)?;
        self.lifecycle.begin_session_close(&packet.header, &close)?;
        self.pending_close = Some(close);
        Ok(close)
    }

    pub async fn ack_close(&mut self, close: &SessionCloseMetadata) -> Result<(), RuntimeError> {
        let ack = SessionCloseAckMetadata {
            close_status: SessionCloseStatus::Closed,
            last_operation_id: close.last_operation_id,
            session_error_code: SESSION_ERROR_NONE,
        };
        let mut header = CommonHeader::new(
            MessageType::SessionCloseAck,
            SESSION_CLOSE_ACK_METADATA_LEN as u32,
            0,
        );
        header.session_id = self.session_id;
        self.lifecycle.apply_session_close_ack(&header, &ack)?;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                ack.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await?;
        if self.pending_close == Some(*close) {
            self.pending_close = None;
        }
        Ok(())
    }

    pub async fn close(mut self) -> Result<(), RuntimeError> {
        self.close_in_place().await
    }

    pub async fn close_in_place(&mut self) -> Result<(), RuntimeError> {
        if let Some(close) = self.pending_close.take() {
            self.ack_close(&close).await?;
        }
        self.remove_from_registry()?;
        self.transport.close().await
    }

    fn require_session_packet(
        &self,
        packet: &RuntimePacket,
        message: &'static str,
    ) -> Result<(), RuntimeError> {
        if packet.header.session_id != self.session_id {
            return Err(RuntimeError::UnexpectedMessage(message));
        }
        Ok(())
    }

    fn require_optional_session_packet(
        &self,
        packet: &RuntimePacket,
        message: &'static str,
    ) -> Result<(), RuntimeError> {
        if packet.header.session_id != 0 && packet.header.session_id != self.session_id {
            return Err(RuntimeError::UnexpectedMessage(message));
        }
        Ok(())
    }

    fn correlated_frame_id(&self, operation_id: u64) -> Result<u32, RuntimeError> {
        if operation_id == 0 {
            return Ok(0);
        }
        self.operation_frames
            .get(&operation_id)
            .copied()
            .ok_or(nnrp_core::NnrpError::UnknownOperation(operation_id).into())
    }

    fn require_operation_frame(
        &self,
        operation_id: u64,
        frame_id: u32,
    ) -> Result<(), RuntimeError> {
        if self.correlated_frame_id(operation_id)? != frame_id {
            return Err(RuntimeError::UnexpectedMessage(
                "server runtime event frame id does not match its operation",
            ));
        }
        Ok(())
    }

    fn operation_id_for_frame(&self, frame_id: u32) -> Result<u64, RuntimeError> {
        self.frame_operations
            .get(&frame_id)
            .copied()
            .ok_or(RuntimeError::UnexpectedMessage(
                "server frame id is not bound to an operation",
            ))
    }

    fn update_registry_last_operation(&self, operation_id: u64) -> Result<(), RuntimeError> {
        let mut sessions = self.session_registry()?;
        if let Some(record) = sessions.get_mut(&self.session_id) {
            record.last_operation_id = record.last_operation_id.max(operation_id);
        }
        Ok(())
    }

    fn remove_from_registry(&self) -> Result<(), RuntimeError> {
        let mut sessions = self.session_registry()?;
        let remove = sessions
            .get(&self.session_id)
            .is_some_and(|record| record.connection_nonce == self.connection_nonce);
        if remove {
            sessions.remove(&self.session_id);
        }
        Ok(())
    }

    fn session_registry(
        &self,
    ) -> Result<MutexGuard<'_, BTreeMap<u32, RuntimeSessionRecord>>, RuntimeError> {
        self.sessions
            .lock()
            .map_err(|_| RuntimeError::Internal("server session registry lock poisoned"))
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn require_body_len(
    actual: usize,
    expected: usize,
    message: &'static str,
) -> Result<(), RuntimeError> {
    if actual != expected {
        return Err(RuntimeError::UnexpectedMessage(message));
    }
    Ok(())
}

#[cfg(test)]
mod accept_tests {
    use async_trait::async_trait;

    use super::*;
    use crate::RuntimeTransportKind;

    #[test]
    fn session_id_allocation_finds_first_gap_in_sorted_registry_keys() {
        assert_eq!(first_available_session_id([]), Some(1));
        assert_eq!(first_available_session_id([1, 2, 4, 8]), Some(3));
        assert_eq!(first_available_session_id([2, 3, 4]), Some(1));
        assert_eq!(first_available_session_id([0, 1, 2]), Some(3));
        assert_eq!(first_available_session_id([u32::MAX]), Some(1));
    }

    struct ReadyListener(RuntimeTransportKind);

    #[test]
    fn default_session_options_match_the_frozen_sdk_contract() {
        let config = NnrpServerConfig::default();

        assert_eq!(
            config.supported_profiles,
            vec![nnrp_core::STANDARD_PROFILE_TOKEN]
        );
        assert!(config.supported_cache_objects.is_empty());
        assert_eq!(config.max_cache_objects, 0);
        assert_eq!(config.max_cache_object_bytes, 0);
        assert!(config
            .schema_registry
            .lookup(
                nnrp_core::TOKEN_DELTA_SCHEMA_ID,
                nnrp_core::TOKEN_DELTA_SCHEMA_VERSION
            )
            .is_some());
        assert_eq!(config.resume_token_bytes, 24);
        assert_eq!(config.max_in_flight_operations, 4);
        assert_eq!(config.granted_operation_credit, 2);
        assert_eq!(config.lease_ttl_ms, 30_000);
        assert_eq!(config.resume_window_ms, 120_000);
    }

    #[async_trait]
    impl FramedListener for ReadyListener {
        fn transport_kind(&self) -> RuntimeTransportKind {
            self.0
        }

        fn local_addr(&self) -> Result<std::net::SocketAddr, RuntimeError> {
            Ok("127.0.0.1:4500".parse().unwrap())
        }

        async fn accept(&self) -> Result<BoxedFramedTransport, RuntimeError> {
            Ok(Box::new(ReadyTransport(self.0)))
        }
    }

    struct ReadyTransport(RuntimeTransportKind);

    #[async_trait]
    impl crate::FramedTransport for ReadyTransport {
        fn transport_kind(&self) -> RuntimeTransportKind {
            self.0
        }

        async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
            Err(RuntimeError::Internal(
                "ready test transport has no packets",
            ))
        }

        async fn write_packet(&mut self, _packet: &RuntimePacket) -> Result<(), RuntimeError> {
            Ok(())
        }

        async fn close(&mut self) -> Result<(), RuntimeError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn stable_accept_rotates_the_first_polled_ready_listener() {
        let listeners = [
            RuntimeTransportKind::Tcp,
            RuntimeTransportKind::Quic,
            RuntimeTransportKind::WebSocket,
        ]
        .into_iter()
        .map(|kind| BoundServerProvider::from_listener(Box::new(ReadyListener(kind))))
        .collect::<Vec<_>>();

        for (start_index, expected_transport) in [
            nnrp_core::TransportId::Tcp,
            nnrp_core::TransportId::Quic,
            nnrp_core::TransportId::WebSocket,
            nnrp_core::TransportId::Tcp,
        ]
        .into_iter()
        .enumerate()
        {
            let (accepted_index, transport_id, transport) =
                accept_stable(&listeners, start_index).await.unwrap();
            assert_eq!(accepted_index, start_index % listeners.len());
            assert_eq!(transport_id, expected_transport);
            assert_eq!(
                transport.transport_kind().transport_id(),
                expected_transport
            );
        }

        assert!(matches!(
            accept_stable(&[], 0).await,
            Err(RuntimeError::ServerListenerSetClosed)
        ));
    }
}
