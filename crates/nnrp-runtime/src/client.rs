use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use nnrp_core::{
    validate_control_request_semantics, validate_partial_result_semantics,
    validate_pressure_semantics, validate_progress_semantics, validate_result_drop_header,
    validate_result_drop_reason_semantics, validate_scheduling_semantics,
    validate_trace_context_semantics, BudgetMetadata, CacheAckMetadata, CacheInvalidateMetadata,
    CacheMissMetadata, CacheObjectKind, CachePutMetadata, CacheReferenceMetadata,
    CapabilityMetadata, ClientHelloMetadata, CommonHeader, ConnectionLifecycle,
    ControlRequestMetadata, FlowUpdateMetadata, FrameSubmitMetadata, InFlightPolicy, MessageType,
    ObjectDeltaMetadata, ObjectDescriptorMetadata, ObjectReferenceMetadata, ObjectReleaseMetadata,
    PartialResultMetadata, PressureMetadata, ProgressMetadata, RecoverableErrorMetadata,
    ResultDropReasonMetadata, ResultHintMetadata, ResultPushMetadata, ResultTerminalState,
    RetryAfterMetadata, RouteHintMetadata, SchedulingMetadata, ServerHelloAckMetadata,
    SessionCloseAckMetadata, SessionCloseMetadata, SessionCloseReason, SessionMigrateAckMetadata,
    SessionMigrateMetadata, SessionOpenAckMetadata, SessionOpenMetadata, SessionPatchAckMetadata,
    SessionPatchMetadata, SessionPriorityClass, SessionStatus, SupersedeMetadata,
    TraceContextMetadata, TransportId, CACHE_ACK_METADATA_LEN, CACHE_INVALIDATE_METADATA_LEN,
    CACHE_MISS_METADATA_LEN, CACHE_PUT_METADATA_LEN, CACHE_REFERENCE_METADATA_LEN,
    CAPABILITY_METADATA_LEN, CLIENT_HELLO_METADATA_LEN, CONTROL_REQUEST_FLAG_COOPERATIVE_ALLOWED,
    CONTROL_REQUEST_FLAG_HARD_ABORT_ALLOWED, CONTROL_REQUEST_METADATA_LEN,
    FRAME_SUBMIT_METADATA_LEN, OBJECT_DELTA_METADATA_LEN, OBJECT_DESCRIPTOR_METADATA_LEN,
    OBJECT_REFERENCE_METADATA_LEN, OBJECT_RELEASE_METADATA_LEN, PARTIAL_RESULT_METADATA_LEN,
    PRESSURE_METADATA_LEN, PROGRESS_METADATA_LEN, RECOVERABLE_ERROR_METADATA_LEN,
    RESULT_DROP_REASON_METADATA_LEN, RESULT_HINT_METADATA_LEN, RESULT_PUSH_METADATA_LEN,
    RETRY_AFTER_METADATA_LEN, ROUTE_HINT_METADATA_LEN, SCHEDULING_FLAG_DISCARD_STALE,
    SCHEDULING_FLAG_EMIT_DROP_REASON, SCHEDULING_METADATA_LEN, SESSION_CLOSE_ACK_METADATA_LEN,
    SESSION_CLOSE_METADATA_LEN, SESSION_ERROR_NONE, SESSION_MIGRATE_ACK_METADATA_LEN,
    SESSION_MIGRATE_METADATA_LEN, SESSION_OPEN_METADATA_LEN, SESSION_PATCH_ACK_METADATA_LEN,
    SESSION_PATCH_METADATA_LEN, STANDARD_PROFILE_TOKEN, TOKEN_DELTA_SCHEMA_ID,
    TOKEN_DELTA_SCHEMA_VERSION, TRACE_CONTEXT_METADATA_LEN,
};

#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
use crate::TcpTransport;
use crate::{
    client_provider::{connect_client, NnrpClientOptions, NnrpClientProvider},
    multiplex::MultiplexedConnection,
    BoxedFramedTransport, FramedTransport, NnrpRuntimeEvent, NnrpRuntimeEventMetadata,
    NnrpRuntimeEventTail, NnrpSubmitRequest, NnrpTerminalEvent, OperationLifecycleEvent,
    RuntimeError, RuntimeFrameHeader, RuntimePacket, RuntimePressureState,
};
use futures_util::lock::Mutex as AsyncMutex;
use nnrp_transport_provider::TransportSelection;
use std::sync::Arc;

const MAX_PENDING_EVENTS_DURING_SESSION_PATCH: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpClientConfig {
    pub requested_session_id: u32,
    pub profile_id: u16,
    pub schema_id: u32,
    pub schema_version: u32,
    pub priority_class: SessionPriorityClass,
    pub default_deadline_ms: u32,
    pub max_in_flight_operations: u16,
    pub lease_ttl_hint_ms: u32,
    pub allow_resume: bool,
    pub resume_token_bytes: u32,
    pub cache_hints: Vec<CacheObjectKind>,
}

impl Default for NnrpClientConfig {
    fn default() -> Self {
        Self {
            requested_session_id: 0,
            profile_id: STANDARD_PROFILE_TOKEN,
            schema_id: TOKEN_DELTA_SCHEMA_ID,
            schema_version: TOKEN_DELTA_SCHEMA_VERSION,
            priority_class: SessionPriorityClass::Balanced,
            default_deadline_ms: 500,
            max_in_flight_operations: 4,
            lease_ttl_hint_ms: 30_000,
            allow_resume: false,
            resume_token_bytes: 0,
            cache_hints: Vec::new(),
        }
    }
}

impl NnrpClientConfig {
    pub fn with_cache_hints(mut self, cache_hints: impl Into<Vec<CacheObjectKind>>) -> Self {
        self.cache_hints = cache_hints.into();
        self
    }

    pub fn with_resume(mut self, resume_token_bytes: u32) -> Self {
        self.allow_resume = true;
        self.resume_token_bytes = resume_token_bytes;
        self
    }
}

pub struct NnrpClient {
    connection: MultiplexedConnection,
    config: NnrpClientConfig,
    lifecycle: AsyncMutex<ConnectionLifecycle>,
    hello: AsyncMutex<Option<ClientHelloMetadata>>,
    open_lock: AsyncMutex<()>,
    transport_selection: Option<TransportSelection>,
}

pub struct NnrpClientSession {
    session_id: u32,
    next_frame_id: u32,
    operation_frames: BTreeMap<u64, u32>,
    frame_operations: BTreeMap<u32, u64>,
    seen_operation_ids: BTreeSet<u64>,
    last_operation_id: u64,
    transport: BoxedFramedTransport,
    lifecycle: ConnectionLifecycle,
    pressure: RuntimePressureState,
    pending_events: VecDeque<(NnrpClientEvent, RuntimePacket)>,
    recovery_ticket: Option<NnrpSessionRecoveryTicket>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpSessionRecoveryTicket {
    session_id: u32,
    resume_token: Vec<u8>,
    resume_from_operation_id: Option<u64>,
    resume_window_ms: u32,
}

impl NnrpSessionRecoveryTicket {
    const MAGIC: [u8; 4] = *b"NRTK";
    const VERSION: u16 = 1;
    const FIXED_PREFIX_BYTES: usize = 28;
    const FLAG_RESUME_FROM_OPERATION_ID_PRESENT: u16 = 1;

    pub fn session_id(&self) -> u32 {
        self.session_id
    }

    pub fn resume_from_operation_id(&self) -> Option<u64> {
        self.resume_from_operation_id
    }

    pub fn resume_window_ms(&self) -> u32 {
        self.resume_window_ms
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let flags = if self.resume_from_operation_id.is_some() {
            Self::FLAG_RESUME_FROM_OPERATION_ID_PRESENT
        } else {
            0
        };
        let token_len = u32::try_from(self.resume_token.len())
            .expect("runtime-issued recovery tokens always fit the wire length");
        let mut encoded = Vec::with_capacity(Self::FIXED_PREFIX_BYTES + self.resume_token.len());
        encoded.extend_from_slice(&Self::MAGIC);
        encoded.extend_from_slice(&Self::VERSION.to_le_bytes());
        encoded.extend_from_slice(&flags.to_le_bytes());
        encoded.extend_from_slice(&self.session_id.to_le_bytes());
        encoded.extend_from_slice(&token_len.to_le_bytes());
        encoded.extend_from_slice(&self.resume_window_ms.to_le_bytes());
        encoded.extend_from_slice(&self.resume_from_operation_id.unwrap_or(0).to_le_bytes());
        encoded.extend_from_slice(&self.resume_token);
        encoded
    }

    pub fn from_bytes(encoded: &[u8]) -> Result<Self, RuntimeError> {
        if encoded.len() < Self::FIXED_PREFIX_BYTES {
            return Err(RuntimeError::InvalidRecoveryTicket("ticket is truncated"));
        }
        if encoded[..4] != Self::MAGIC {
            return Err(RuntimeError::InvalidRecoveryTicket(
                "magic does not match NRTK",
            ));
        }
        let version = read_ticket_u16(encoded, 4);
        if version != Self::VERSION {
            return Err(RuntimeError::InvalidRecoveryTicket(
                "version is unsupported",
            ));
        }
        let flags = read_ticket_u16(encoded, 6);
        if flags & !Self::FLAG_RESUME_FROM_OPERATION_ID_PRESENT != 0 {
            return Err(RuntimeError::InvalidRecoveryTicket(
                "reserved flags are non-zero",
            ));
        }
        let session_id = read_ticket_u32(encoded, 8);
        if session_id == 0 {
            return Err(RuntimeError::InvalidRecoveryTicket("session id is zero"));
        }
        let token_len = usize::try_from(read_ticket_u32(encoded, 12))
            .map_err(|_| RuntimeError::InvalidRecoveryTicket("token length is invalid"))?;
        if token_len == 0 {
            return Err(RuntimeError::InvalidRecoveryTicket("resume token is empty"));
        }
        let expected_len = Self::FIXED_PREFIX_BYTES.checked_add(token_len).ok_or(
            RuntimeError::InvalidRecoveryTicket("ticket length overflows"),
        )?;
        if encoded.len() != expected_len {
            return Err(RuntimeError::InvalidRecoveryTicket(
                "ticket has truncated or trailing token bytes",
            ));
        }
        let resume_window_ms = read_ticket_u32(encoded, 16);
        let resume_from_operation_id = (flags & Self::FLAG_RESUME_FROM_OPERATION_ID_PRESENT != 0)
            .then(|| read_ticket_u64(encoded, 20));
        Ok(Self {
            session_id,
            resume_token: encoded[Self::FIXED_PREFIX_BYTES..].to_vec(),
            resume_from_operation_id,
            resume_window_ms,
        })
    }

    pub(crate) fn resume_token(&self) -> &[u8] {
        &self.resume_token
    }
}

fn read_ticket_u16(encoded: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        encoded[offset..offset + 2]
            .try_into()
            .expect("ticket prefix checked"),
    )
}

fn read_ticket_u32(encoded: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        encoded[offset..offset + 4]
            .try_into()
            .expect("ticket prefix checked"),
    )
}

fn read_ticket_u64(encoded: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        encoded[offset..offset + 8]
            .try_into()
            .expect("ticket prefix checked"),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpResult {
    pub operation_id: u64,
    pub terminal_state: ResultTerminalState,
    pub event: NnrpTerminalEvent,
}

impl NnrpResult {
    pub fn from_runtime(operation_id: u64, event: NnrpRuntimeEvent) -> Result<Self, RuntimeError> {
        if operation_id == 0 {
            return Err(RuntimeError::UnexpectedMessage(
                "terminal result requires a non-zero operation id",
            ));
        }
        let terminal_state = match (&event.header.message_type, &event.metadata, &event.tail) {
            (
                MessageType::ResultPush,
                NnrpRuntimeEventMetadata::ResultPush(_),
                NnrpRuntimeEventTail::Body(_),
            ) => ResultTerminalState::Success,
            (
                MessageType::ResultDrop,
                NnrpRuntimeEventMetadata::None,
                NnrpRuntimeEventTail::None,
            ) => ResultTerminalState::Dropped,
            (
                MessageType::ResultDropReason,
                NnrpRuntimeEventMetadata::ResultDropReason(_),
                NnrpRuntimeEventTail::Diagnostic(_),
            ) => ResultTerminalState::Dropped,
            _ => {
                return Err(RuntimeError::UnexpectedMessage(
                    "runtime terminal evidence does not match a terminal result message",
                ));
            }
        };
        Ok(Self {
            operation_id,
            terminal_state,
            event: NnrpTerminalEvent::Runtime(event),
        })
    }

    pub fn from_lifecycle(event: OperationLifecycleEvent) -> Result<Self, RuntimeError> {
        if event.operation_id == 0 {
            return Err(RuntimeError::UnexpectedMessage(
                "terminal result requires a non-zero operation id",
            ));
        }
        let terminal_state = ResultTerminalState::from_operation_state(event.state).ok_or(
            RuntimeError::UnexpectedMessage(
                "operation lifecycle event does not establish a terminal result",
            ),
        )?;
        Ok(Self {
            operation_id: event.operation_id,
            terminal_state,
            event: NnrpTerminalEvent::Lifecycle(event),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NnrpClientEvent {
    Result(NnrpResult),
    PartialResult {
        metadata: PartialResultMetadata,
        body: Vec<u8>,
    },
    Progress {
        metadata: ProgressMetadata,
        body: Vec<u8>,
    },
    Control {
        message_type: MessageType,
        metadata: ControlRequestMetadata,
        body: Vec<u8>,
    },
    Scheduling {
        message_type: MessageType,
        metadata: SchedulingMetadata,
    },
    Supersede {
        metadata: SupersedeMetadata,
        body: Vec<u8>,
    },
    Budget(BudgetMetadata),
    FlowUpdate(FlowUpdateMetadata),
    Backpressure(PressureMetadata),
    CreditUpdate(PressureMetadata),
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
    ResultHint(ResultHintMetadata),
}

impl NnrpClient {
    pub async fn connect<I>(options: NnrpClientOptions, providers: I) -> Result<Self, RuntimeError>
    where
        I: IntoIterator<Item = Arc<dyn NnrpClientProvider>>,
    {
        let (transport, config, selection) = connect_client(options, providers).await?;
        Ok(Self {
            connection: MultiplexedConnection::start(transport),
            config,
            lifecycle: AsyncMutex::new(ConnectionLifecycle::new()),
            hello: AsyncMutex::new(None),
            open_lock: AsyncMutex::new(()),
            transport_selection: Some(selection),
        })
    }

    pub fn transport_selection(&self) -> Option<&TransportSelection> {
        self.transport_selection.as_ref()
    }

    #[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
    pub async fn connect_tcp(
        addr: impl tokio::net::ToSocketAddrs,
        config: NnrpClientConfig,
    ) -> Result<Self, RuntimeError> {
        Self::from_transport(TcpTransport::connect(addr).await?, config)
    }

    pub async fn connect_quic(
        _endpoint: &str,
        _config: NnrpClientConfig,
    ) -> Result<Self, RuntimeError> {
        Err(RuntimeError::UnsupportedTransport(
            "QUIC provider is not installed; use from_transport with a QUIC FramedTransport",
        ))
    }

    pub fn from_transport<T>(transport: T, config: NnrpClientConfig) -> Result<Self, RuntimeError>
    where
        T: FramedTransport + 'static,
    {
        Self::from_boxed_transport(Box::new(transport), config)
    }

    pub fn from_boxed_transport(
        transport: BoxedFramedTransport,
        config: NnrpClientConfig,
    ) -> Result<Self, RuntimeError> {
        Ok(Self {
            connection: MultiplexedConnection::start(transport),
            config,
            lifecycle: AsyncMutex::new(ConnectionLifecycle::new()),
            hello: AsyncMutex::new(None),
            open_lock: AsyncMutex::new(()),
            transport_selection: None,
        })
    }

    pub async fn open_session(&self) -> Result<NnrpClientSession, RuntimeError> {
        self.open_session_with(self.config.clone()).await
    }

    pub async fn open_session_with(
        &self,
        config: NnrpClientConfig,
    ) -> Result<NnrpClientSession, RuntimeError> {
        self.open_session_inner(config, None).await
    }

    pub async fn resume_session(
        &self,
        ticket: NnrpSessionRecoveryTicket,
    ) -> Result<NnrpClientSession, RuntimeError> {
        self.resume_session_with(ticket, self.config.clone()).await
    }

    pub async fn resume_session_with(
        &self,
        ticket: NnrpSessionRecoveryTicket,
        mut config: NnrpClientConfig,
    ) -> Result<NnrpClientSession, RuntimeError> {
        config.requested_session_id = ticket.session_id;
        config.allow_resume = true;
        self.open_session_inner(config, Some(ticket)).await
    }

    async fn open_session_inner(
        &self,
        config: NnrpClientConfig,
        resume_ticket: Option<NnrpSessionRecoveryTicket>,
    ) -> Result<NnrpClientSession, RuntimeError> {
        let _open_guard = self.open_lock.lock().await;
        self.ensure_hello(&config).await?;

        let resume_token = resume_ticket
            .as_ref()
            .map(NnrpSessionRecoveryTicket::resume_token)
            .unwrap_or_default();
        let metadata = Self::session_open_metadata(&config, resume_token.len())?;
        let mut metadata_bytes = vec![0u8; SESSION_OPEN_METADATA_LEN];
        metadata.write(&mut metadata_bytes)?;

        let header = CommonHeader::new(
            MessageType::SessionOpen,
            SESSION_OPEN_METADATA_LEN as u32,
            0,
        );
        self.connection
            .write_packet(RuntimePacket::new(
                header,
                metadata_bytes,
                resume_token.to_vec(),
            )?)
            .await?;

        let ack_packet = self.connection.read_control_packet().await?;
        if ack_packet.header.message_type != MessageType::SessionOpenAck {
            return Err(RuntimeError::UnexpectedMessage(
                "client expected SESSION_OPEN_ACK",
            ));
        }

        let ack = SessionOpenAckMetadata::parse(&ack_packet.metadata)?;
        nnrp_core::validate_session_recovery_ack(&metadata, &ack)?;
        let expected_body_len = usize::try_from(ack.resume_token_bytes)
            .ok()
            .and_then(|token| {
                usize::try_from(ack.session_extension_bytes)
                    .ok()
                    .and_then(|extension| token.checked_add(extension))
            })
            .ok_or(RuntimeError::UnexpectedMessage(
                "SESSION_OPEN_ACK body length overflows",
            ))?;
        if ack_packet.body.len() != expected_body_len {
            return Err(RuntimeError::UnexpectedMessage(
                "SESSION_OPEN_ACK body length does not match token and extension lengths",
            ));
        }
        if !matches!(
            ack.session_status,
            SessionStatus::Opened | SessionStatus::Resumed
        ) {
            return Err(RuntimeError::SessionRejected {
                code: ack.session_error_code,
                diagnostic: String::from_utf8_lossy(&ack_packet.body).into_owned(),
            });
        }
        if ack.session_extension_bytes != 0 {
            return Err(RuntimeError::UnexpectedMessage(
                "accepted SESSION_OPEN_ACK has an unsupported extension body",
            ));
        }
        if ack.resume_token_bytes > 0
            && config.resume_token_bytes > 0
            && ack.resume_token_bytes > config.resume_token_bytes
        {
            return Err(RuntimeError::UnexpectedMessage(
                "SESSION_OPEN_ACK resume token exceeds client capacity",
            ));
        }
        let recovery_ticket = if ack.resume_token_bytes > 0 {
            Some(NnrpSessionRecoveryTicket {
                session_id: ack.session_id,
                resume_token: ack_packet.body,
                resume_from_operation_id: resume_ticket
                    .and_then(|ticket| ticket.resume_from_operation_id),
                resume_window_ms: ack.resume_window_ms,
            })
        } else {
            None
        };
        let lifecycle = {
            let mut lifecycle = self.lifecycle.lock().await;
            lifecycle.apply_session_open_ack(&ack)?;
            lifecycle.clone()
        };
        let transport = self.connection.register_session(ack.session_id).await?;

        Ok(NnrpClientSession {
            session_id: ack.session_id,
            next_frame_id: 1,
            operation_frames: BTreeMap::new(),
            frame_operations: BTreeMap::new(),
            seen_operation_ids: BTreeSet::new(),
            last_operation_id: 0,
            transport: Box::new(transport),
            lifecycle,
            pressure: RuntimePressureState::default(),
            pending_events: VecDeque::new(),
            recovery_ticket,
        })
    }

    async fn ensure_hello(&self, config: &NnrpClientConfig) -> Result<(), RuntimeError> {
        let cache_object_bitmap = cache_object_bitmap(&config.cache_hints)?;
        let mut hello = self.hello.lock().await;
        if let Some(existing) = *hello {
            if cache_object_bitmap & !existing.cache_object_bitmap != 0 {
                return Err(RuntimeError::UnexpectedMessage(
                    "session cache hints exceed the established CLIENT_HELLO capability",
                ));
            }
            return Ok(());
        }

        let metadata = ClientHelloMetadata {
            min_version_major: nnrp_core::CURRENT_VERSION_MAJOR,
            max_version_major: nnrp_core::CURRENT_VERSION_MAJOR,
            supported_wire_format_bitmap: 1u16 << nnrp_core::CURRENT_WIRE_FORMAT,
            supported_profile_bitmap: 0x0000_0003,
            supported_payload_kind_bitmap: nnrp_core::PayloadKindBitmap::TENSOR
                | nnrp_core::PayloadKindBitmap::TOKEN_CHUNK
                | nnrp_core::PayloadKindBitmap::STRUCTURED_EVENT
                | nnrp_core::PayloadKindBitmap::TOOL_DELTA
                | nnrp_core::PayloadKindBitmap::OPAQUE_BYTES,
            supported_codec_bitmap: 0,
            supported_compression_bitmap: 0,
            supported_dtype_bitmap: 0,
            supported_layout_bitmap: 0,
            cache_digest_bitmap: 0,
            cache_object_bitmap,
            cache_namespace_count: 0,
            max_lane_count: 1,
            max_cache_entries: 0,
            max_cache_bytes: 0,
            target_cadence_x100: 0,
            latency_budget_ms: config.default_deadline_ms.min(u16::MAX as u32) as u16,
            quality_tier: 0,
            degrade_policy: 0,
            requested_session_id: config.requested_session_id,
            auth_bytes: 0,
            control_extension_bytes: 0,
        };
        let header = CommonHeader::new(
            MessageType::ClientHello,
            CLIENT_HELLO_METADATA_LEN as u32,
            0,
        );
        self.connection
            .write_packet(RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await?;
        let packet = self.connection.read_control_packet().await?;
        if packet.header.message_type != MessageType::ServerHelloAck {
            return Err(RuntimeError::UnexpectedMessage(
                "client expected SERVER_HELLO_ACK",
            ));
        }
        if !packet.body.is_empty() {
            return Err(RuntimeError::UnexpectedMessage(
                "SERVER_HELLO_ACK body must be empty without extensions",
            ));
        }
        let ack = ServerHelloAckMetadata::parse(&packet.metadata)?;
        ack.validate_against_client_hello(&metadata)?;
        *hello = Some(metadata);
        Ok(())
    }

    fn session_open_metadata(
        config: &NnrpClientConfig,
        resume_token_bytes: usize,
    ) -> Result<SessionOpenMetadata, RuntimeError> {
        let resume_token_bytes = u32::try_from(resume_token_bytes).map_err(|_| {
            RuntimeError::UnexpectedMessage("session resume token exceeds wire length")
        })?;
        Ok(SessionOpenMetadata {
            requested_session_id: config.requested_session_id,
            profile_id: config.profile_id,
            priority_class: config.priority_class,
            session_flags: if config.allow_resume {
                nnrp_core::SESSION_FLAG_ALLOW_RESUME
            } else {
                0
            },
            schema_id: config.schema_id,
            schema_version: config.schema_version,
            default_deadline_ms: config.default_deadline_ms,
            max_in_flight_operations: config.max_in_flight_operations,
            lease_ttl_hint_ms: config.lease_ttl_hint_ms,
            resume_token_bytes,
            auth_bytes: 0,
            session_extension_bytes: 0,
            client_session_tag: config.requested_session_id as u64,
        })
    }
}

fn cache_object_bitmap(cache_hints: &[CacheObjectKind]) -> Result<u16, RuntimeError> {
    cache_hints.iter().try_fold(0u16, |bitmap, kind| {
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

impl NnrpClientSession {
    pub fn session_id(&self) -> u32 {
        self.session_id
    }

    pub fn recovery_ticket(&self) -> Option<NnrpSessionRecoveryTicket> {
        self.recovery_ticket.clone().map(|mut ticket| {
            if self.last_operation_id != 0 {
                ticket.resume_from_operation_id = Some(
                    ticket
                        .resume_from_operation_id
                        .map_or(self.last_operation_id, |existing| {
                            existing.max(self.last_operation_id)
                        }),
                );
            }
            ticket
        })
    }

    pub fn lifecycle(&self) -> &ConnectionLifecycle {
        &self.lifecycle
    }

    pub fn pressure_state(&self) -> RuntimePressureState {
        self.pressure
    }

    pub async fn submit(&mut self, request: NnrpSubmitRequest) -> Result<u32, RuntimeError> {
        self.submit_nowait(request).await
    }

    pub async fn submit_encoded(
        &mut self,
        metadata: FrameSubmitMetadata,
        body: Vec<u8>,
    ) -> Result<u32, RuntimeError> {
        let frame_id = self.next_frame_id;
        self.submit_encoded_with_frame_id(frame_id, metadata, body)
            .await
    }

    pub async fn submit_encoded_nowait(
        &mut self,
        metadata: FrameSubmitMetadata,
        body: Vec<u8>,
    ) -> Result<u32, RuntimeError> {
        self.submit_encoded(metadata, body).await
    }

    pub async fn submit_encoded_with_frame_id(
        &mut self,
        frame_id: u32,
        metadata: FrameSubmitMetadata,
        body: Vec<u8>,
    ) -> Result<u32, RuntimeError> {
        self.submit_nowait(NnrpSubmitRequest {
            operation_id: metadata.operation_id,
            frame_id,
            header: Default::default(),
            metadata,
            body,
        })
        .await
    }

    pub async fn submit_nowait(&mut self, request: NnrpSubmitRequest) -> Result<u32, RuntimeError> {
        let frame_id = request.frame_id;
        let metadata = request.metadata;
        if frame_id == 0 || frame_id < self.next_frame_id {
            return Err(RuntimeError::UnexpectedMessage(
                "client frame id must not be zero, reused, or moved backward",
            ));
        }
        if metadata.operation_id == 0 || self.seen_operation_ids.contains(&metadata.operation_id) {
            return Err(RuntimeError::UnexpectedMessage(
                "client operation id must not be zero or reused",
            ));
        }
        let next_frame_id = frame_id
            .checked_add(1)
            .ok_or(RuntimeError::FrameIdOverflow)?;

        let mut header = CommonHeader::new(
            MessageType::FrameSubmit,
            FRAME_SUBMIT_METADATA_LEN as u32,
            request.body.len() as u32,
        );
        header.session_id = self.session_id;
        header.frame_id = frame_id;
        header.flags = request.header.flags;
        header.view_id = request.header.view_id;
        header.route_id = request.header.route_id;
        header.trace_id = request.header.trace_id;

        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                request.body,
            )?)
            .await?;
        self.next_frame_id = next_frame_id;
        self.operation_frames
            .insert(metadata.operation_id, frame_id);
        self.frame_operations
            .insert(frame_id, metadata.operation_id);
        self.seen_operation_ids.insert(metadata.operation_id);
        self.last_operation_id = self.last_operation_id.max(metadata.operation_id);
        Ok(frame_id)
    }

    pub async fn send_runtime_frame(
        &mut self,
        message_type: MessageType,
        frame_id: u32,
        payload: &[u8],
    ) -> Result<(), RuntimeError> {
        match message_type {
            MessageType::FlowUpdate => {
                self.send_flow_update(FlowUpdateMetadata::parse(payload)?)
                    .await
            }
            MessageType::Progress => {
                let (metadata, body) = ProgressMetadata::parse_with_body(payload)?;
                self.send_progress(metadata, body.to_vec()).await
            }
            MessageType::PartialResult => {
                let (metadata, body) = PartialResultMetadata::parse_with_body(payload)?;
                self.send_partial_result(metadata, body.to_vec()).await
            }
            MessageType::Backpressure => {
                self.send_backpressure(PressureMetadata::parse(payload)?)
                    .await
            }
            MessageType::ResultDropReason => {
                let (metadata, body) = ResultDropReasonMetadata::parse_with_diagnostics(payload)?;
                self.send_result_drop_reason(metadata, body.to_vec()).await
            }
            MessageType::Cancel | MessageType::Abort => {
                let (metadata, body) = ControlRequestMetadata::parse_with_diagnostics(payload)?;
                self.send_control_request_with_diagnostics(message_type, metadata, body.to_vec())
                    .await
            }
            MessageType::PriorityUpdate | MessageType::Deadline | MessageType::ExpireAt => {
                self.send_scheduling_update(message_type, SchedulingMetadata::parse(payload)?)
                    .await
            }
            MessageType::Supersede => {
                let (metadata, body) = SupersedeMetadata::parse_with_diagnostics(payload)?;
                self.supersede_operation(metadata, body.to_vec()).await
            }
            MessageType::BudgetUpdate => self.update_budget(BudgetMetadata::parse(payload)?).await,
            MessageType::CreditUpdate => {
                self.send_credit_update(PressureMetadata::parse(payload)?)
                    .await
            }
            MessageType::CapabilityNegotiation | MessageType::DegradeProfile => {
                let (metadata, body) = CapabilityMetadata::parse_with_body(payload)?;
                self.send_capability(message_type, metadata, body.to_vec())
                    .await
            }
            MessageType::RouteHint | MessageType::ExecutionHint => {
                let (metadata, body) = RouteHintMetadata::parse_with_body(payload)?;
                self.send_route_hint(message_type, metadata, body.to_vec())
                    .await
            }
            MessageType::TraceContext => {
                let (metadata, body) = TraceContextMetadata::parse_with_body(payload)?;
                self.send_trace_context(frame_id, metadata, body.to_vec())
                    .await
            }
            MessageType::ErrorRecoverable => {
                let (metadata, body) = RecoverableErrorMetadata::parse_with_diagnostics(payload)?;
                self.send_recoverable_error(metadata, body.to_vec()).await
            }
            MessageType::RetryAfter => {
                let (metadata, body) = RetryAfterMetadata::parse_with_diagnostics(payload)?;
                self.send_retry_after(metadata, body.to_vec()).await
            }
            MessageType::ObjectDeclare => {
                let (metadata, body) = ObjectDescriptorMetadata::parse_with_extension(payload)?;
                self.send_object_declare(metadata, body.to_vec()).await
            }
            MessageType::ObjectRef => {
                let (metadata, body) = ObjectReferenceMetadata::parse_with_extension(payload)?;
                self.send_object_ref(metadata, body.to_vec()).await
            }
            MessageType::ObjectRelease => {
                let (metadata, body) = ObjectReleaseMetadata::parse_with_diagnostics(payload)?;
                self.send_object_release(metadata, body.to_vec()).await
            }
            MessageType::ObjectPatch | MessageType::ObjectDelta => {
                let metadata = ObjectDeltaMetadata::parse(payload)?;
                self.send_object_delta(
                    message_type,
                    metadata,
                    payload[OBJECT_DELTA_METADATA_LEN..].to_vec(),
                )
                .await
            }
            MessageType::CachePut => {
                if payload.len() < CACHE_PUT_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client CACHE_PUT payload is shorter than metadata",
                    ));
                }
                let metadata = CachePutMetadata::parse(&payload[..CACHE_PUT_METADATA_LEN])?;
                self.send_cache_put(metadata, payload[CACHE_PUT_METADATA_LEN..].to_vec())
                    .await
            }
            MessageType::CacheAck => self.send_cache_ack(CacheAckMetadata::parse(payload)?).await,
            MessageType::CacheReference => {
                let (metadata, body) = CacheReferenceMetadata::parse_with_extension(payload)?;
                self.send_cache_reference(metadata, body.to_vec()).await
            }
            MessageType::CacheMiss => {
                let (metadata, body) = CacheMissMetadata::parse_with_diagnostics(payload)?;
                self.send_cache_miss(metadata, body.to_vec()).await
            }
            MessageType::CacheInvalidate => {
                self.send_cache_invalidate(CacheInvalidateMetadata::parse(payload)?)
                    .await
            }
            _ => Err(RuntimeError::UnexpectedMessage(
                "client runtime frame direction is unsupported",
            )),
        }
    }

    pub async fn await_result(&mut self) -> Result<NnrpResult, RuntimeError> {
        match self.await_client_event_packet().await?.0 {
            NnrpClientEvent::Result(result) => Ok(result),
            _ => Err(RuntimeError::UnexpectedMessage(
                "client expected a terminal result but received another runtime event",
            )),
        }
    }

    pub async fn await_event(&mut self) -> Result<NnrpRuntimeEvent, RuntimeError> {
        Ok(self.await_event_packet().await?.0)
    }

    pub async fn await_event_packet(
        &mut self,
    ) -> Result<(NnrpRuntimeEvent, RuntimePacket), RuntimeError> {
        let (event, packet) = self.await_client_event_packet().await?;
        let header = RuntimeFrameHeader::from(&packet.header);
        Ok((NnrpRuntimeEvent::from_client(header, event), packet))
    }

    async fn await_client_event_packet(
        &mut self,
    ) -> Result<(NnrpClientEvent, RuntimePacket), RuntimeError> {
        if let Some((event, packet)) = self.pending_events.pop_front() {
            return Ok((event, packet));
        }
        let packet = self.transport.read_packet().await?;
        self.decode_event_packet(packet)
    }

    pub async fn await_event_packet_batch(
        &mut self,
        max_events: usize,
    ) -> Result<Vec<(NnrpRuntimeEvent, RuntimePacket)>, RuntimeError> {
        validate_event_batch_limit(max_events)?;
        let mut events = Vec::with_capacity(max_events);
        events.push(self.await_event_packet().await?);
        events.extend(self.poll_event_packet_batch(max_events - 1)?);
        Ok(events)
    }

    pub fn poll_event_packet_batch(
        &mut self,
        max_events: usize,
    ) -> Result<Vec<(NnrpRuntimeEvent, RuntimePacket)>, RuntimeError> {
        let mut events = Vec::with_capacity(max_events);
        while events.len() < max_events {
            let Some((event, packet)) = self.pending_events.pop_front() else {
                break;
            };
            let header = RuntimeFrameHeader::from(&packet.header);
            events.push((NnrpRuntimeEvent::from_client(header, event), packet));
        }
        while events.len() < max_events {
            let Some(packet) = self.transport.try_read_packet()? else {
                break;
            };
            let (event, packet) = self.decode_event_packet(packet)?;
            let header = RuntimeFrameHeader::from(&packet.header);
            events.push((NnrpRuntimeEvent::from_client(header, event), packet));
        }
        Ok(events)
    }

    fn decode_event_packet(
        &mut self,
        packet: RuntimePacket,
    ) -> Result<(NnrpClientEvent, RuntimePacket), RuntimeError> {
        let wire_packet = packet.clone();
        let event = match packet.header.message_type {
            MessageType::ResultPush => {
                self.require_session_packet(&packet, "client received result for another session")?;
                if packet.metadata.len() != RESULT_PUSH_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed RESULT_PUSH metadata length",
                    ));
                }
                let metadata = ResultPushMetadata::parse(&packet.metadata)?;
                let operation_id = self.complete_operation_by_frame(packet.header.frame_id)?;
                Ok(NnrpClientEvent::Result(NnrpResult::from_runtime(
                    operation_id,
                    NnrpRuntimeEvent {
                        header: RuntimeFrameHeader::from(&packet.header),
                        metadata: NnrpRuntimeEventMetadata::ResultPush(metadata),
                        tail: NnrpRuntimeEventTail::Body(packet.body),
                    },
                )?))
            }
            MessageType::ResultDrop => {
                self.require_session_packet(&packet, "client received drop for another session")?;
                validate_result_drop_header(&packet.header)?;
                let operation_id = self.complete_operation_by_frame(packet.header.frame_id)?;
                Ok(NnrpClientEvent::Result(NnrpResult::from_runtime(
                    operation_id,
                    NnrpRuntimeEvent {
                        header: RuntimeFrameHeader::from(&packet.header),
                        metadata: NnrpRuntimeEventMetadata::None,
                        tail: NnrpRuntimeEventTail::None,
                    },
                )?))
            }
            MessageType::ResultDropReason => {
                self.require_session_packet(
                    &packet,
                    "client received drop reason for another session",
                )?;
                if packet.metadata.len() != RESULT_DROP_REASON_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed RESULT_DROP_REASON metadata length",
                    ));
                }
                let metadata = ResultDropReasonMetadata::parse(&packet.metadata)?;
                validate_result_drop_reason_semantics(&metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "client received RESULT_DROP_REASON body length mismatch",
                )?;
                self.complete_operation(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpClientEvent::Result(NnrpResult::from_runtime(
                    metadata.operation_id,
                    NnrpRuntimeEvent {
                        header: RuntimeFrameHeader::from(&packet.header),
                        metadata: NnrpRuntimeEventMetadata::ResultDropReason(metadata),
                        tail: NnrpRuntimeEventTail::Diagnostic(packet.body),
                    },
                )?))
            }
            MessageType::PartialResult => {
                self.require_session_packet(
                    &packet,
                    "client received partial result for another session",
                )?;
                if packet.metadata.len() != PARTIAL_RESULT_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed PARTIAL_RESULT metadata length",
                    ));
                }
                let metadata = PartialResultMetadata::parse(&packet.metadata)?;
                validate_partial_result_semantics(&metadata)?;
                if metadata.body_bytes as usize != packet.body.len() {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received PARTIAL_RESULT body length mismatch",
                    ));
                }
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpClientEvent::PartialResult {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::Progress => {
                self.require_session_packet(
                    &packet,
                    "client received progress for another session",
                )?;
                if packet.metadata.len() != PROGRESS_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed PROGRESS metadata length",
                    ));
                }
                let metadata = ProgressMetadata::parse(&packet.metadata)?;
                validate_progress_semantics(&metadata)?;
                if metadata.body_bytes as usize != packet.body.len() {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received PROGRESS body length mismatch",
                    ));
                }
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpClientEvent::Progress {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::Cancel | MessageType::Abort => {
                self.require_session_packet(
                    &packet,
                    "client received control for another session",
                )?;
                if packet.metadata.len() != CONTROL_REQUEST_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed runtime control metadata length",
                    ));
                }
                let metadata = ControlRequestMetadata::parse(&packet.metadata)?;
                validate_control_request_semantics(packet.header.message_type, &metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "client received runtime control diagnostic body length mismatch",
                )?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpClientEvent::Control {
                    message_type: packet.header.message_type,
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::PriorityUpdate | MessageType::Deadline | MessageType::ExpireAt => {
                self.require_session_packet(
                    &packet,
                    "client received scheduling update for another session",
                )?;
                if packet.metadata.len() != SCHEDULING_METADATA_LEN || !packet.body.is_empty() {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed scheduling metadata length",
                    ));
                }
                let metadata = SchedulingMetadata::parse(&packet.metadata)?;
                validate_scheduling_semantics(packet.header.message_type, &metadata)?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpClientEvent::Scheduling {
                    message_type: packet.header.message_type,
                    metadata,
                })
            }
            MessageType::Supersede => {
                self.require_session_packet(
                    &packet,
                    "client received supersede for another session",
                )?;
                if packet.metadata.len() != nnrp_core::SUPERSEDE_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed SUPERSEDE metadata length",
                    ));
                }
                let metadata = SupersedeMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "client received SUPERSEDE diagnostic body length mismatch",
                )?;
                self.require_operation_frame(metadata.old_operation_id, packet.header.frame_id)?;
                Ok(NnrpClientEvent::Supersede {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::BudgetUpdate => {
                self.require_session_packet(
                    &packet,
                    "client received budget update for another session",
                )?;
                if packet.metadata.len() != nnrp_core::BUDGET_METADATA_LEN
                    || !packet.body.is_empty()
                {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed BUDGET_UPDATE lengths",
                    ));
                }
                let metadata = BudgetMetadata::parse(&packet.metadata)?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpClientEvent::Budget(metadata))
            }
            MessageType::FlowUpdate => {
                if packet.metadata.len() != nnrp_core::FLOW_UPDATE_METADATA_LEN
                    || !packet.body.is_empty()
                {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed FLOW_UPDATE lengths",
                    ));
                }
                let metadata = FlowUpdateMetadata::parse(&packet.metadata)?;
                self.lifecycle
                    .validate_flow_update(&packet.header, &metadata)?;
                Ok(NnrpClientEvent::FlowUpdate(metadata))
            }
            MessageType::Backpressure | MessageType::CreditUpdate => {
                self.require_optional_session_packet(
                    &packet,
                    "client received pressure update for another session",
                )?;
                if packet.metadata.len() != PRESSURE_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed pressure metadata length",
                    ));
                }
                let metadata = PressureMetadata::parse(&packet.metadata)?;
                validate_pressure_semantics(packet.header.message_type, &metadata)?;
                self.pressure
                    .apply_inbound(packet.header.message_type, metadata)?;
                match packet.header.message_type {
                    MessageType::Backpressure => Ok(NnrpClientEvent::Backpressure(metadata)),
                    MessageType::CreditUpdate => Ok(NnrpClientEvent::CreditUpdate(metadata)),
                    _ => unreachable!("message type was already matched"),
                }
            }
            MessageType::CapabilityNegotiation | MessageType::DegradeProfile => {
                self.require_optional_session_packet(
                    &packet,
                    "client received capability update for another session",
                )?;
                if packet.metadata.len() != CAPABILITY_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed capability metadata length",
                    ));
                }
                let metadata = CapabilityMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.body_bytes as usize,
                    "client received capability body length mismatch",
                )?;
                Ok(NnrpClientEvent::Capability {
                    message_type: packet.header.message_type,
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::RouteHint | MessageType::ExecutionHint => {
                self.require_optional_session_packet(
                    &packet,
                    "client received route hint for another session",
                )?;
                if packet.metadata.len() != ROUTE_HINT_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed route hint metadata length",
                    ));
                }
                let metadata = RouteHintMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.body_bytes as usize,
                    "client received route hint body length mismatch",
                )?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpClientEvent::RouteHint {
                    message_type: packet.header.message_type,
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::ObjectDeclare => {
                self.require_session_packet(
                    &packet,
                    "client received object declaration for another session",
                )?;
                if packet.metadata.len() != OBJECT_DESCRIPTOR_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed OBJECT_DECLARE metadata length",
                    ));
                }
                let metadata = ObjectDescriptorMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.metadata_bytes as usize,
                    "client received OBJECT_DECLARE body length mismatch",
                )?;
                Ok(NnrpClientEvent::ObjectDeclare {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::ObjectRef => {
                self.require_session_packet(
                    &packet,
                    "client received object reference for another session",
                )?;
                if packet.metadata.len() != OBJECT_REFERENCE_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed OBJECT_REF metadata length",
                    ));
                }
                let metadata = ObjectReferenceMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.metadata_bytes as usize,
                    "client received OBJECT_REF body length mismatch",
                )?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpClientEvent::ObjectRef {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::ObjectRelease => {
                self.require_session_packet(
                    &packet,
                    "client received object release for another session",
                )?;
                if packet.metadata.len() != OBJECT_RELEASE_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed OBJECT_RELEASE metadata length",
                    ));
                }
                let metadata = ObjectReleaseMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "client received OBJECT_RELEASE body length mismatch",
                )?;
                self.require_operation_frame(metadata.operation_id, packet.header.frame_id)?;
                Ok(NnrpClientEvent::ObjectRelease {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::ObjectPatch | MessageType::ObjectDelta => {
                self.require_session_packet(
                    &packet,
                    "client received object delta for another session",
                )?;
                if packet.metadata.len() != OBJECT_DELTA_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed object delta metadata length",
                    ));
                }
                let metadata = ObjectDeltaMetadata::parse(&packet.metadata)?;
                let expected_body_len =
                    metadata.metadata_bytes.saturating_add(metadata.delta_bytes) as usize;
                require_body_len(
                    packet.body.len(),
                    expected_body_len,
                    "client received object delta body length mismatch",
                )?;
                Ok(NnrpClientEvent::ObjectDelta {
                    message_type: packet.header.message_type,
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::CacheReference => {
                self.require_session_packet(
                    &packet,
                    "client received cache reference for another session",
                )?;
                if packet.metadata.len() != CACHE_REFERENCE_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed CACHE_REFERENCE metadata length",
                    ));
                }
                let metadata = CacheReferenceMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.metadata_bytes as usize,
                    "client received CACHE_REFERENCE body length mismatch",
                )?;
                Ok(NnrpClientEvent::CacheReference {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::CacheMiss => {
                self.require_session_packet(
                    &packet,
                    "client received cache miss for another session",
                )?;
                if packet.metadata.len() != CACHE_MISS_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed CACHE_MISS metadata length",
                    ));
                }
                let metadata = CacheMissMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "client received CACHE_MISS body length mismatch",
                )?;
                Ok(NnrpClientEvent::CacheMiss {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::CacheInvalidate => {
                self.require_session_packet(
                    &packet,
                    "client received cache invalidate for another session",
                )?;
                if packet.metadata.len() != CACHE_INVALIDATE_METADATA_LEN || !packet.body.is_empty()
                {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed CACHE_INVALIDATE lengths",
                    ));
                }
                Ok(NnrpClientEvent::CacheInvalidate(
                    CacheInvalidateMetadata::parse(&packet.metadata)?,
                ))
            }
            MessageType::TraceContext => {
                self.require_optional_session_packet(
                    &packet,
                    "client received trace context for another session",
                )?;
                if packet.metadata.len() != TRACE_CONTEXT_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed TRACE_CONTEXT metadata length",
                    ));
                }
                let metadata = TraceContextMetadata::parse(&packet.metadata)?;
                validate_trace_context_semantics(&metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.body_bytes as usize,
                    "client received TRACE_CONTEXT body length mismatch",
                )?;
                Ok(NnrpClientEvent::TraceContext {
                    frame_id: packet.header.frame_id,
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::ErrorRecoverable => {
                self.require_optional_session_packet(
                    &packet,
                    "client received recoverable error for another session",
                )?;
                if packet.metadata.len() != RECOVERABLE_ERROR_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed ERROR_RECOVERABLE metadata length",
                    ));
                }
                let metadata = RecoverableErrorMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "client received ERROR_RECOVERABLE diagnostic body length mismatch",
                )?;
                Ok(NnrpClientEvent::RecoverableError {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::RetryAfter => {
                self.require_optional_session_packet(
                    &packet,
                    "client received retry-after for another session",
                )?;
                if packet.metadata.len() != RETRY_AFTER_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed RETRY_AFTER metadata length",
                    ));
                }
                let metadata = RetryAfterMetadata::parse(&packet.metadata)?;
                require_body_len(
                    packet.body.len(),
                    metadata.diagnostic_bytes as usize,
                    "client received RETRY_AFTER diagnostic body length mismatch",
                )?;
                Ok(NnrpClientEvent::RetryAfter {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::ResultHint => {
                self.require_session_packet(
                    &packet,
                    "client received result hint for another session",
                )?;
                if packet.metadata.len() != RESULT_HINT_METADATA_LEN || !packet.body.is_empty() {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed RESULT_HINT payload",
                    ));
                }
                Ok(NnrpClientEvent::ResultHint(ResultHintMetadata::parse(
                    &packet.metadata,
                )?))
            }
            _ => Err(RuntimeError::UnexpectedMessage(
                "client expected a runtime result or control event",
            )),
        }?;
        Ok((event, wire_packet))
    }

    pub async fn cancel_operation(
        &mut self,
        operation_id: u64,
        reason_code: u16,
    ) -> Result<(), RuntimeError> {
        self.send_control_request(
            MessageType::Cancel,
            ControlRequestMetadata {
                operation_id,
                control_sequence: operation_id,
                reason_code,
                source_role: 1,
                flags: CONTROL_REQUEST_FLAG_COOPERATIVE_ALLOWED,
                diagnostic_bytes: 0,
            },
        )
        .await
    }

    fn correlated_frame_id(&self, operation_id: u64) -> Result<u32, RuntimeError> {
        self.operation_frames
            .get(&operation_id)
            .copied()
            .ok_or(RuntimeError::UnexpectedMessage(
                "client runtime event references an unknown operation",
            ))
    }

    fn require_operation_frame(
        &self,
        operation_id: u64,
        frame_id: u32,
    ) -> Result<(), RuntimeError> {
        if self.correlated_frame_id(operation_id)? != frame_id {
            return Err(RuntimeError::UnexpectedMessage(
                "client runtime event frame id does not match its operation",
            ));
        }
        Ok(())
    }

    fn complete_operation_by_frame(&mut self, frame_id: u32) -> Result<u64, RuntimeError> {
        let operation_id =
            self.frame_operations
                .remove(&frame_id)
                .ok_or(RuntimeError::UnexpectedMessage(
                    "client terminal event references an unknown frame",
                ))?;
        self.operation_frames.remove(&operation_id);
        Ok(operation_id)
    }

    fn complete_operation(&mut self, operation_id: u64, frame_id: u32) -> Result<(), RuntimeError> {
        self.require_operation_frame(operation_id, frame_id)?;
        self.operation_frames.remove(&operation_id);
        self.frame_operations.remove(&frame_id);
        Ok(())
    }

    pub async fn abort_operation(
        &mut self,
        operation_id: u64,
        reason_code: u16,
    ) -> Result<(), RuntimeError> {
        self.send_control_request(
            MessageType::Abort,
            ControlRequestMetadata {
                operation_id,
                control_sequence: operation_id,
                reason_code,
                source_role: 1,
                flags: CONTROL_REQUEST_FLAG_HARD_ABORT_ALLOWED,
                diagnostic_bytes: 0,
            },
        )
        .await
    }

    pub async fn send_progress(
        &mut self,
        metadata: ProgressMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        validate_progress_semantics(&metadata)?;
        require_body_len(
            body.len(),
            metadata.body_bytes as usize,
            "client PROGRESS body length mismatch",
        )?;
        let frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.write_runtime_packet(
            MessageType::Progress,
            frame_id,
            metadata.to_bytes()?.to_vec(),
            body,
        )
        .await
    }

    pub async fn send_partial_result(
        &mut self,
        metadata: PartialResultMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        validate_partial_result_semantics(&metadata)?;
        require_body_len(
            body.len(),
            metadata.body_bytes as usize,
            "client PARTIAL_RESULT body length mismatch",
        )?;
        let frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.write_runtime_packet(
            MessageType::PartialResult,
            frame_id,
            metadata.to_bytes()?.to_vec(),
            body,
        )
        .await
    }

    pub async fn send_result_drop_reason(
        &mut self,
        metadata: ResultDropReasonMetadata,
        diagnostics: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        validate_result_drop_reason_semantics(&metadata)?;
        require_body_len(
            diagnostics.len(),
            metadata.diagnostic_bytes as usize,
            "client RESULT_DROP_REASON diagnostic body length mismatch",
        )?;
        let frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.write_runtime_packet(
            MessageType::ResultDropReason,
            frame_id,
            metadata.to_bytes()?.to_vec(),
            diagnostics,
        )
        .await
    }

    pub async fn send_backpressure(
        &mut self,
        metadata: PressureMetadata,
    ) -> Result<(), RuntimeError> {
        validate_pressure_semantics(MessageType::Backpressure, &metadata)?;
        self.pressure
            .apply_outbound(MessageType::Backpressure, metadata)?;
        self.write_runtime_packet(
            MessageType::Backpressure,
            0,
            metadata.to_bytes()?.to_vec(),
            Vec::new(),
        )
        .await
    }

    pub async fn send_flow_update(
        &mut self,
        metadata: FlowUpdateMetadata,
    ) -> Result<(), RuntimeError> {
        let mut header = CommonHeader::new(
            MessageType::FlowUpdate,
            nnrp_core::FLOW_UPDATE_METADATA_LEN as u32,
            0,
        );
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

    pub async fn send_control_request(
        &mut self,
        message_type: MessageType,
        metadata: ControlRequestMetadata,
    ) -> Result<(), RuntimeError> {
        self.send_control_request_with_diagnostics(message_type, metadata, Vec::new())
            .await
    }

    pub async fn send_control_request_with_diagnostics(
        &mut self,
        message_type: MessageType,
        metadata: ControlRequestMetadata,
        diagnostics: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        validate_control_request_semantics(message_type, &metadata)?;
        require_body_len(
            diagnostics.len(),
            metadata.diagnostic_bytes as usize,
            "client runtime control diagnostic body length mismatch",
        )?;
        let mut header = CommonHeader::new(
            message_type,
            CONTROL_REQUEST_METADATA_LEN as u32,
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

    pub async fn update_priority(
        &mut self,
        operation_id: u64,
        priority_class: u16,
        priority_delta: i16,
    ) -> Result<(), RuntimeError> {
        self.send_scheduling_update(
            MessageType::PriorityUpdate,
            SchedulingMetadata {
                operation_id,
                control_sequence: operation_id,
                priority_class,
                priority_delta,
                deadline_unix_ms: 0,
                flags: 0,
            },
        )
        .await
    }

    pub async fn update_deadline(
        &mut self,
        operation_id: u64,
        deadline_unix_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.send_scheduling_update(
            MessageType::Deadline,
            SchedulingMetadata {
                operation_id,
                control_sequence: operation_id,
                priority_class: 0,
                priority_delta: 0,
                deadline_unix_ms,
                flags: SCHEDULING_FLAG_DISCARD_STALE | SCHEDULING_FLAG_EMIT_DROP_REASON,
            },
        )
        .await
    }

    pub async fn expire_at(
        &mut self,
        operation_id: u64,
        deadline_unix_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.send_scheduling_update(
            MessageType::ExpireAt,
            SchedulingMetadata {
                operation_id,
                control_sequence: operation_id,
                priority_class: 0,
                priority_delta: 0,
                deadline_unix_ms,
                flags: SCHEDULING_FLAG_DISCARD_STALE | SCHEDULING_FLAG_EMIT_DROP_REASON,
            },
        )
        .await
    }

    pub async fn send_scheduling_update(
        &mut self,
        message_type: MessageType,
        metadata: SchedulingMetadata,
    ) -> Result<(), RuntimeError> {
        validate_scheduling_semantics(message_type, &metadata)?;
        let mut header = CommonHeader::new(message_type, SCHEDULING_METADATA_LEN as u32, 0);
        header.session_id = self.session_id;
        header.frame_id = self.correlated_frame_id(metadata.operation_id)?;
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
        let mut header =
            CommonHeader::new(MessageType::CreditUpdate, PRESSURE_METADATA_LEN as u32, 0);
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
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
            "client supersede diagnostic body length mismatch",
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
                "client capability send requires CAPABILITY_NEGOTIATION or DEGRADE_PROFILE",
            ));
        }
        require_body_len(
            body.len(),
            metadata.body_bytes as usize,
            "client capability body length mismatch",
        )?;
        self.write_runtime_packet(message_type, 0, metadata.to_bytes()?.to_vec(), body)
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
                "client route hint send requires ROUTE_HINT or EXECUTION_HINT",
            ));
        }
        require_body_len(
            body.len(),
            metadata.body_bytes as usize,
            "client route hint body length mismatch",
        )?;
        let frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.write_runtime_packet(message_type, frame_id, metadata.to_bytes()?.to_vec(), body)
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
            "client trace context body length mismatch",
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
            "client recoverable error diagnostic body length mismatch",
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
            "client retry-after diagnostic body length mismatch",
        )?;
        self.write_runtime_packet(
            MessageType::RetryAfter,
            0,
            metadata.to_bytes()?.to_vec(),
            diagnostics,
        )
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
            "client OBJECT_DECLARE body length mismatch",
        )?;
        self.write_runtime_packet(
            MessageType::ObjectDeclare,
            0,
            metadata.to_bytes()?.to_vec(),
            body,
        )
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
            "client OBJECT_REF body length mismatch",
        )?;
        let frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.write_runtime_packet(
            MessageType::ObjectRef,
            frame_id,
            metadata.to_bytes()?.to_vec(),
            body,
        )
        .await
    }

    pub async fn send_object_release(
        &mut self,
        metadata: ObjectReleaseMetadata,
        diagnostics: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        require_body_len(
            diagnostics.len(),
            metadata.diagnostic_bytes as usize,
            "client OBJECT_RELEASE diagnostic body length mismatch",
        )?;
        let frame_id = self.correlated_frame_id(metadata.operation_id)?;
        self.write_runtime_packet(
            MessageType::ObjectRelease,
            frame_id,
            metadata.to_bytes()?.to_vec(),
            diagnostics,
        )
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
                "client object delta send requires OBJECT_PATCH or OBJECT_DELTA",
            ));
        }
        let expected_body_len =
            metadata.metadata_bytes.saturating_add(metadata.delta_bytes) as usize;
        require_body_len(
            body.len(),
            expected_body_len,
            "client object delta body length mismatch",
        )?;
        self.write_runtime_packet(message_type, 0, metadata.to_bytes()?.to_vec(), body)
            .await
    }

    pub async fn send_cache_put(
        &mut self,
        metadata: CachePutMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        require_body_len(
            body.len(),
            metadata.object_bytes as usize,
            "client CACHE_PUT body length mismatch",
        )?;
        self.write_runtime_packet(
            MessageType::CachePut,
            0,
            metadata.to_bytes()?.to_vec(),
            body,
        )
        .await
    }

    pub async fn send_cache_ack(&mut self, metadata: CacheAckMetadata) -> Result<(), RuntimeError> {
        self.write_runtime_packet(
            MessageType::CacheAck,
            0,
            metadata.to_bytes()?.to_vec(),
            Vec::new(),
        )
        .await
    }

    pub async fn receive_cache_put(&mut self) -> Result<(CachePutMetadata, Vec<u8>), RuntimeError> {
        let packet = self.transport.read_packet().await?;
        self.require_session_packet(&packet, "client received cache put for another session")?;
        if packet.header.message_type != MessageType::CachePut
            || packet.metadata.len() != CACHE_PUT_METADATA_LEN
        {
            return Err(RuntimeError::UnexpectedMessage(
                "client expected a well-formed CACHE_PUT",
            ));
        }
        let metadata = CachePutMetadata::parse(&packet.metadata)?;
        require_body_len(
            packet.body.len(),
            metadata.object_bytes as usize,
            "client received CACHE_PUT body length mismatch",
        )?;
        Ok((metadata, packet.body))
    }

    pub async fn receive_cache_ack(&mut self) -> Result<CacheAckMetadata, RuntimeError> {
        let packet = self.transport.read_packet().await?;
        self.require_session_packet(&packet, "client received cache ack for another session")?;
        if packet.header.message_type != MessageType::CacheAck
            || packet.metadata.len() != CACHE_ACK_METADATA_LEN
            || !packet.body.is_empty()
        {
            return Err(RuntimeError::UnexpectedMessage(
                "client expected a well-formed CACHE_ACK",
            ));
        }
        Ok(CacheAckMetadata::parse(&packet.metadata)?)
    }

    pub async fn send_ping(&mut self) -> Result<(), RuntimeError> {
        self.write_runtime_packet(MessageType::Ping, 0, Vec::new(), Vec::new())
            .await
    }

    pub async fn send_pong(&mut self) -> Result<(), RuntimeError> {
        self.write_runtime_packet(MessageType::Pong, 0, Vec::new(), Vec::new())
            .await
    }

    pub async fn receive_ping(&mut self) -> Result<(), RuntimeError> {
        self.receive_empty_role_message(MessageType::Ping, "client expected PING")
            .await
    }

    pub async fn receive_pong(&mut self) -> Result<(), RuntimeError> {
        self.receive_empty_role_message(MessageType::Pong, "client expected PONG")
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
            "client CACHE_REFERENCE body length mismatch",
        )?;
        self.write_runtime_packet(
            MessageType::CacheReference,
            0,
            metadata.to_bytes()?.to_vec(),
            body,
        )
        .await
    }

    pub async fn send_cache_miss(
        &mut self,
        metadata: CacheMissMetadata,
        diagnostics: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        require_body_len(
            diagnostics.len(),
            metadata.diagnostic_bytes as usize,
            "client CACHE_MISS diagnostic body length mismatch",
        )?;
        self.write_runtime_packet(
            MessageType::CacheMiss,
            0,
            metadata.to_bytes()?.to_vec(),
            diagnostics,
        )
        .await
    }

    pub async fn send_cache_invalidate(
        &mut self,
        metadata: CacheInvalidateMetadata,
    ) -> Result<(), RuntimeError> {
        self.write_runtime_packet(
            MessageType::CacheInvalidate,
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

    pub async fn cancel_frame(&mut self, frame_id: u32) -> Result<(), RuntimeError> {
        let mut header = CommonHeader::new(MessageType::FrameCancel, 0, 0);
        header.session_id = self.session_id;
        header.frame_id = frame_id;
        self.transport
            .write_packet(&RuntimePacket::new(header, Vec::new(), Vec::new())?)
            .await
    }

    pub async fn patch_session(
        &mut self,
        patch: SessionPatchMetadata,
    ) -> Result<SessionPatchAckMetadata, RuntimeError> {
        if patch.profile_patch_bytes != 0 {
            return Err(RuntimeError::UnexpectedMessage(
                "client session patch metadata declares an unsupported profile-specific body",
            ));
        }
        let mut header = CommonHeader::new(
            MessageType::SessionPatch,
            SESSION_PATCH_METADATA_LEN as u32,
            0,
        );
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                patch.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await?;

        loop {
            let ack_packet = self.transport.read_packet().await?;
            if ack_packet.header.message_type != MessageType::SessionPatchAck {
                let event = self.decode_event_packet(ack_packet)?;
                if self.pending_events.len() >= MAX_PENDING_EVENTS_DURING_SESSION_PATCH {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client session patch exceeded the pending event limit before acknowledgement",
                    ));
                }
                self.pending_events.push_back(event);
                continue;
            }
            self.require_session_packet(
                &ack_packet,
                "client received patch ack for another session",
            )?;
            if ack_packet.metadata.len() != SESSION_PATCH_ACK_METADATA_LEN {
                return Err(RuntimeError::UnexpectedMessage(
                    "client received malformed SESSION_PATCH_ACK metadata length",
                ));
            }
            return Ok(SessionPatchAckMetadata::parse(&ack_packet.metadata)?);
        }
    }

    pub async fn migrate_transport(
        &mut self,
        request: SessionMigrateMetadata,
    ) -> Result<SessionMigrateAckMetadata, RuntimeError> {
        let mut header = CommonHeader::new(
            MessageType::SessionMigrate,
            SESSION_MIGRATE_METADATA_LEN as u32,
            0,
        );
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                request.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await?;

        let ack_packet = self.transport.read_packet().await?;
        if ack_packet.header.message_type != MessageType::SessionMigrateAck {
            return Err(RuntimeError::UnexpectedMessage(
                "client expected SESSION_MIGRATE_ACK",
            ));
        }
        self.require_session_packet(
            &ack_packet,
            "client received migrate ack for another session",
        )?;
        if ack_packet.metadata.len() != SESSION_MIGRATE_ACK_METADATA_LEN {
            return Err(RuntimeError::UnexpectedMessage(
                "client received malformed SESSION_MIGRATE_ACK metadata length",
            ));
        }
        let ack = SessionMigrateAckMetadata::parse(&ack_packet.metadata)?;
        nnrp_core::validate_migration_recovery(&request, &ack)?;
        Ok(ack)
    }

    pub fn build_migration_request(
        &self,
        new_transport_id: TransportId,
        last_result_frame_id: u64,
        client_migrate_ts_us: u64,
    ) -> SessionMigrateMetadata {
        SessionMigrateMetadata {
            old_transport_id: self.transport.transport_kind().transport_id(),
            new_transport_id,
            last_result_frame_id,
            client_migrate_ts_us,
        }
    }

    pub async fn close(mut self) -> Result<(), RuntimeError> {
        self.close_in_place().await
    }

    pub async fn close_in_place(&mut self) -> Result<(), RuntimeError> {
        let close = SessionCloseMetadata {
            close_reason: SessionCloseReason::ClientShutdown,
            in_flight_policy: InFlightPolicy::Drain,
            drain_timeout_ms: 0,
            last_operation_id: self.last_operation_id,
            session_error_code: SESSION_ERROR_NONE,
            session_close_tag: self.session_id,
        };
        self.close_with(close).await?;
        self.transport.close().await
    }

    pub async fn close_with(
        &mut self,
        close: SessionCloseMetadata,
    ) -> Result<SessionCloseAckMetadata, RuntimeError> {
        let mut header = CommonHeader::new(
            MessageType::SessionClose,
            SESSION_CLOSE_METADATA_LEN as u32,
            0,
        );
        header.session_id = self.session_id;
        self.lifecycle.begin_session_close(&header, &close)?;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                close.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await?;

        let ack_packet = self.transport.read_packet().await?;
        if ack_packet.header.message_type != MessageType::SessionCloseAck {
            return Err(RuntimeError::UnexpectedMessage(
                "client expected SESSION_CLOSE_ACK",
            ));
        }
        if ack_packet.header.session_id != self.session_id {
            return Err(RuntimeError::UnexpectedMessage(
                "client received close ack for another session",
            ));
        }
        if ack_packet.metadata.len() != SESSION_CLOSE_ACK_METADATA_LEN {
            return Err(RuntimeError::UnexpectedMessage(
                "client received malformed SESSION_CLOSE_ACK metadata length",
            ));
        }

        let ack = SessionCloseAckMetadata::parse(&ack_packet.metadata)?;
        self.lifecycle
            .apply_session_close_ack(&ack_packet.header, &ack)?;
        Ok(ack)
    }

    pub async fn close_transport(mut self) -> Result<(), RuntimeError> {
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
}

fn validate_event_batch_limit(max_events: usize) -> Result<(), RuntimeError> {
    if max_events == 0 {
        Err(RuntimeError::UnexpectedMessage(
            "client event batch limit must be greater than zero",
        ))
    } else {
        Ok(())
    }
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

impl fmt::Debug for NnrpClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NnrpClient")
            .field("transport", &self.connection.transport_kind())
            .field("config", &self.config)
            .field("lifecycle", &self.lifecycle)
            .finish()
    }
}

impl fmt::Debug for NnrpClientSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NnrpClientSession")
            .field("session_id", &self.session_id)
            .field("next_frame_id", &self.next_frame_id)
            .field("transport", &self.transport.transport_kind())
            .field("lifecycle", &self.lifecycle)
            .finish()
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn default_session_options_match_the_frozen_sdk_contract() {
        let config = NnrpClientConfig::default();

        assert_eq!(config.requested_session_id, 0);
        assert_eq!(config.profile_id, STANDARD_PROFILE_TOKEN);
        assert_eq!(config.schema_id, TOKEN_DELTA_SCHEMA_ID);
        assert_eq!(config.schema_version, TOKEN_DELTA_SCHEMA_VERSION);
        assert_eq!(config.priority_class, SessionPriorityClass::Balanced);
        assert_eq!(config.default_deadline_ms, 500);
        assert_eq!(config.max_in_flight_operations, 4);
        assert_eq!(config.lease_ttl_hint_ms, 30_000);
        assert!(!config.allow_resume);
        assert_eq!(config.resume_token_bytes, 0);
        assert!(config.cache_hints.is_empty());
    }

    #[test]
    fn recovery_ticket_uses_the_canonical_nrtk_envelope() {
        let ticket = NnrpSessionRecoveryTicket {
            session_id: 0x1122_3344,
            resume_token: vec![0xaa, 0xbb, 0xcc],
            resume_from_operation_id: Some(0x0102_0304_0506_0708),
            resume_window_ms: 90_000,
        };

        let encoded = ticket.to_bytes();

        assert_eq!(&encoded[..4], b"NRTK");
        assert_eq!(read_ticket_u16(&encoded, 4), 1);
        assert_eq!(read_ticket_u16(&encoded, 6), 1);
        assert_eq!(read_ticket_u32(&encoded, 8), 0x1122_3344);
        assert_eq!(read_ticket_u32(&encoded, 12), 3);
        assert_eq!(read_ticket_u32(&encoded, 16), 90_000);
        assert_eq!(read_ticket_u64(&encoded, 20), 0x0102_0304_0506_0708);
        assert_eq!(&encoded[28..], &[0xaa, 0xbb, 0xcc]);
        assert_eq!(
            NnrpSessionRecoveryTicket::from_bytes(&encoded).unwrap(),
            ticket
        );
    }

    #[test]
    fn recovery_ticket_rejects_every_noncanonical_envelope_shape() {
        let ticket = NnrpSessionRecoveryTicket {
            session_id: 7,
            resume_token: vec![1, 2, 3],
            resume_from_operation_id: None,
            resume_window_ms: 120_000,
        };
        let canonical = ticket.to_bytes();
        let mut invalid = Vec::new();

        invalid.push(canonical[..27].to_vec());
        let mut wrong_magic = canonical.clone();
        wrong_magic[0] = b'X';
        invalid.push(wrong_magic);
        let mut wrong_version = canonical.clone();
        wrong_version[4..6].copy_from_slice(&2u16.to_le_bytes());
        invalid.push(wrong_version);
        let mut reserved_flags = canonical.clone();
        reserved_flags[6..8].copy_from_slice(&2u16.to_le_bytes());
        invalid.push(reserved_flags);
        let mut zero_session = canonical.clone();
        zero_session[8..12].fill(0);
        invalid.push(zero_session);
        let mut empty_token = canonical.clone();
        empty_token[12..16].fill(0);
        empty_token.truncate(28);
        invalid.push(empty_token);
        invalid.push(canonical[..canonical.len() - 1].to_vec());
        let mut trailing = canonical.clone();
        trailing.push(0);
        invalid.push(trailing);

        for encoded in invalid {
            assert!(matches!(
                NnrpSessionRecoveryTicket::from_bytes(&encoded),
                Err(RuntimeError::InvalidRecoveryTicket(_))
            ));
        }
    }

    #[test]
    fn lifecycle_terminal_results_preserve_exact_state_mapping() {
        for (state, terminal_state) in [
            (
                nnrp_core::OperationState::Completed,
                ResultTerminalState::Success,
            ),
            (
                nnrp_core::OperationState::Cancelled,
                ResultTerminalState::Cancelled,
            ),
            (
                nnrp_core::OperationState::Superseded,
                ResultTerminalState::Dropped,
            ),
            (
                nnrp_core::OperationState::Failed,
                ResultTerminalState::Error,
            ),
        ] {
            let lifecycle = OperationLifecycleEvent::new(41, state).unwrap();
            let result = NnrpResult::from_lifecycle(lifecycle).unwrap();

            assert_eq!(result.operation_id, 41);
            assert_eq!(result.terminal_state, terminal_state);
            assert_eq!(result.event.as_lifecycle(), Some(&lifecycle));
            assert!(result.event.as_runtime().is_none());
        }
    }

    #[test]
    fn nonterminal_lifecycle_states_cannot_become_results() {
        for state in [
            nnrp_core::OperationState::Accepted,
            nnrp_core::OperationState::Running,
            nnrp_core::OperationState::Partial,
            nnrp_core::OperationState::WaitingTool,
        ] {
            let lifecycle = OperationLifecycleEvent::new(42, state).unwrap();
            assert!(matches!(
                NnrpResult::from_lifecycle(lifecycle),
                Err(RuntimeError::UnexpectedMessage(_))
            ));
        }
    }

    #[test]
    fn terminal_evidence_rejects_zero_operation_ids_and_nonterminal_wire_events() {
        assert!(OperationLifecycleEvent::new(0, nnrp_core::OperationState::Failed).is_err());
        assert!(matches!(
            NnrpResult::from_lifecycle(OperationLifecycleEvent {
                operation_id: 0,
                state: nnrp_core::OperationState::Failed,
            }),
            Err(RuntimeError::UnexpectedMessage(_))
        ));

        let header = RuntimeFrameHeader::from(CommonHeader::new(MessageType::Progress, 0, 0));
        assert!(matches!(
            NnrpResult::from_runtime(
                43,
                NnrpRuntimeEvent {
                    header,
                    metadata: NnrpRuntimeEventMetadata::None,
                    tail: NnrpRuntimeEventTail::None,
                },
            ),
            Err(RuntimeError::UnexpectedMessage(_))
        ));
    }

    #[test]
    fn runtime_terminal_result_retains_the_complete_wire_event() {
        let mut header = CommonHeader::new(MessageType::ResultDrop, 0, 0);
        header.session_id = 7;
        header.frame_id = 9;
        header.view_id = 11;
        header.route_id = 13;
        header.trace_id = 17;
        let event = NnrpRuntimeEvent {
            header: RuntimeFrameHeader::from(header),
            metadata: NnrpRuntimeEventMetadata::None,
            tail: NnrpRuntimeEventTail::None,
        };

        let result = NnrpResult::from_runtime(44, event.clone()).unwrap();

        assert_eq!(result.operation_id, 44);
        assert_eq!(result.terminal_state, ResultTerminalState::Dropped);
        assert_eq!(result.event, NnrpTerminalEvent::Runtime(event));
        assert!(result.event.as_lifecycle().is_none());
    }
}
