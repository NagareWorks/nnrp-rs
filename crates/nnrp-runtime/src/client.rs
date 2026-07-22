use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nnrp_core::{
    validate_control_request_semantics, validate_partial_result_semantics,
    validate_pressure_semantics, validate_progress_semantics, validate_result_drop_header,
    validate_result_drop_reason_semantics, validate_scheduling_semantics,
    validate_trace_context_semantics, BudgetMetadata, CacheInvalidateMetadata, CacheMissMetadata,
    CacheObjectKind, CacheReferenceMetadata, CapabilityMetadata, CommonHeader, ConnectionLifecycle,
    ControlRequestMetadata, FlowUpdateMetadata, FrameSubmitMetadata, InFlightPolicy, MessageType,
    ObjectDeltaMetadata, ObjectDescriptorMetadata, ObjectReferenceMetadata, ObjectReleaseMetadata,
    PartialResultMetadata, PressureMetadata, ProgressMetadata, RecoverableErrorMetadata,
    ResultDropReasonMetadata, ResultHintMetadata, ResultPushMetadata, RetryAfterMetadata,
    RouteHintMetadata, SchedulingMetadata, SessionCloseAckMetadata, SessionCloseMetadata,
    SessionCloseReason, SessionMigrateAckMetadata, SessionMigrateMetadata, SessionOpenAckMetadata,
    SessionOpenMetadata, SessionPatchAckMetadata, SessionPatchMetadata, SessionPriorityClass,
    SessionStatus, SupersedeMetadata, TraceContextMetadata, TransportId,
    CACHE_INVALIDATE_METADATA_LEN, CACHE_MISS_METADATA_LEN, CACHE_REFERENCE_METADATA_LEN,
    CAPABILITY_METADATA_LEN, CONTROL_REQUEST_FLAG_COOPERATIVE_ALLOWED,
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
    BoxedFramedTransport, FramedTransport, RuntimeError, RuntimePacket, RuntimePressureState,
    RuntimeTransportKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpClientConfig {
    pub transport: RuntimeTransportKind,
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
            transport: RuntimeTransportKind::Tcp,
            requested_session_id: 1,
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
    pub fn with_transport(mut self, transport: RuntimeTransportKind) -> Self {
        self.transport = transport;
        self
    }

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
    transport: BoxedFramedTransport,
    config: NnrpClientConfig,
    lifecycle: ConnectionLifecycle,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpResult {
    pub frame_id: u32,
    pub metadata: ResultPushMetadata,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NnrpClientEvent {
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
    ResultDrop {
        frame_id: u32,
    },
    ResultDropReason {
        metadata: ResultDropReasonMetadata,
        body: Vec<u8>,
    },
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
    #[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
    pub async fn connect_tcp(
        addr: impl tokio::net::ToSocketAddrs,
        config: NnrpClientConfig,
    ) -> Result<Self, RuntimeError> {
        if config.transport != RuntimeTransportKind::Tcp {
            return Err(RuntimeError::UnsupportedTransport(
                "client config selected a non-TCP transport for connect_tcp",
            ));
        }
        Self::from_transport(TcpTransport::connect(addr).await?, config)
    }

    pub async fn connect_quic(
        _endpoint: &str,
        config: NnrpClientConfig,
    ) -> Result<Self, RuntimeError> {
        if config.transport != RuntimeTransportKind::Quic {
            return Err(RuntimeError::UnsupportedTransport(
                "client config selected a non-QUIC transport for connect_quic",
            ));
        }
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
        if transport.transport_kind() != config.transport {
            return Err(RuntimeError::UnsupportedTransport(
                "client config transport does not match the provided transport slot",
            ));
        }
        Ok(Self {
            transport,
            config,
            lifecycle: ConnectionLifecycle::new(),
        })
    }

    pub async fn open_session(mut self) -> Result<NnrpClientSession, RuntimeError> {
        let metadata = self.session_open_metadata();
        let mut metadata_bytes = vec![0u8; SESSION_OPEN_METADATA_LEN];
        metadata.write(&mut metadata_bytes)?;

        let header = CommonHeader::new(
            MessageType::SessionOpen,
            SESSION_OPEN_METADATA_LEN as u32,
            0,
        );
        self.transport
            .write_packet(&RuntimePacket::new(header, metadata_bytes, Vec::new())?)
            .await?;

        let ack_packet = self.transport.read_packet().await?;
        if ack_packet.header.message_type != MessageType::SessionOpenAck {
            return Err(RuntimeError::UnexpectedMessage(
                "client expected SESSION_OPEN_ACK",
            ));
        }

        let ack = SessionOpenAckMetadata::parse(&ack_packet.metadata)?;
        nnrp_core::validate_session_recovery_ack(&metadata, &ack)?;
        if !matches!(
            ack.session_status,
            SessionStatus::Opened | SessionStatus::Resumed
        ) {
            return Err(RuntimeError::UnexpectedMessage(
                "client session open was not accepted",
            ));
        }
        self.lifecycle.apply_session_open_ack(&ack)?;

        Ok(NnrpClientSession {
            session_id: ack.session_id,
            next_frame_id: 1,
            operation_frames: BTreeMap::new(),
            frame_operations: BTreeMap::new(),
            seen_operation_ids: BTreeSet::new(),
            last_operation_id: 0,
            transport: self.transport,
            lifecycle: self.lifecycle,
            pressure: RuntimePressureState::default(),
        })
    }

    fn session_open_metadata(&self) -> SessionOpenMetadata {
        SessionOpenMetadata {
            requested_session_id: self.config.requested_session_id,
            profile_id: self.config.profile_id,
            priority_class: self.config.priority_class,
            session_flags: if self.config.allow_resume {
                nnrp_core::SESSION_FLAG_ALLOW_RESUME
            } else {
                0
            },
            schema_id: self.config.schema_id,
            schema_version: self.config.schema_version,
            default_deadline_ms: self.config.default_deadline_ms,
            max_in_flight_operations: self.config.max_in_flight_operations,
            lease_ttl_hint_ms: self.config.lease_ttl_hint_ms,
            resume_token_bytes: self.config.resume_token_bytes,
            auth_bytes: 0,
            session_extension_bytes: 0,
            client_session_tag: self.config.requested_session_id as u64,
        }
    }
}

impl NnrpClientSession {
    pub fn session_id(&self) -> u32 {
        self.session_id
    }

    pub fn lifecycle(&self) -> &ConnectionLifecycle {
        &self.lifecycle
    }

    pub fn pressure_state(&self) -> RuntimePressureState {
        self.pressure
    }

    pub async fn submit(
        &mut self,
        metadata: FrameSubmitMetadata,
        body: Vec<u8>,
    ) -> Result<u32, RuntimeError> {
        self.submit_nowait(metadata, body).await
    }

    pub async fn submit_nowait(
        &mut self,
        metadata: FrameSubmitMetadata,
        body: Vec<u8>,
    ) -> Result<u32, RuntimeError> {
        let frame_id = self.next_frame_id;
        self.submit_with_frame_id(frame_id, metadata, body).await
    }

    pub async fn submit_with_frame_id(
        &mut self,
        frame_id: u32,
        metadata: FrameSubmitMetadata,
        body: Vec<u8>,
    ) -> Result<u32, RuntimeError> {
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
        match self.await_event().await? {
            NnrpClientEvent::Result(result) => Ok(result),
            NnrpClientEvent::ResultDrop { .. } => Err(RuntimeError::UnexpectedMessage(
                "client expected RESULT_PUSH but received RESULT_DROP",
            )),
            NnrpClientEvent::ResultDropReason { .. } => Err(RuntimeError::UnexpectedMessage(
                "client expected RESULT_PUSH but received RESULT_DROP_REASON",
            )),
            NnrpClientEvent::FlowUpdate(_) => Err(RuntimeError::UnexpectedMessage(
                "client expected RESULT_PUSH but received FLOW_UPDATE",
            )),
            NnrpClientEvent::PartialResult { .. } => Err(RuntimeError::UnexpectedMessage(
                "client expected RESULT_PUSH but received PARTIAL_RESULT",
            )),
            NnrpClientEvent::Backpressure(_) => Err(RuntimeError::UnexpectedMessage(
                "client expected RESULT_PUSH but received BACKPRESSURE",
            )),
            NnrpClientEvent::CreditUpdate(_) => Err(RuntimeError::UnexpectedMessage(
                "client expected RESULT_PUSH but received CREDIT_UPDATE",
            )),
            NnrpClientEvent::Progress { .. }
            | NnrpClientEvent::Control { .. }
            | NnrpClientEvent::Scheduling { .. }
            | NnrpClientEvent::Supersede { .. }
            | NnrpClientEvent::Budget(_)
            | NnrpClientEvent::ObjectDeclare { .. }
            | NnrpClientEvent::ObjectRef { .. }
            | NnrpClientEvent::ObjectRelease { .. }
            | NnrpClientEvent::ObjectDelta { .. }
            | NnrpClientEvent::CacheReference { .. }
            | NnrpClientEvent::CacheMiss { .. }
            | NnrpClientEvent::CacheInvalidate(_)
            | NnrpClientEvent::Capability { .. }
            | NnrpClientEvent::RouteHint { .. }
            | NnrpClientEvent::TraceContext { .. }
            | NnrpClientEvent::RecoverableError { .. }
            | NnrpClientEvent::RetryAfter { .. }
            | NnrpClientEvent::ResultHint(_) => Err(RuntimeError::UnexpectedMessage(
                "client expected RESULT_PUSH but received a non-terminal runtime event",
            )),
        }
    }

    pub async fn await_event(&mut self) -> Result<NnrpClientEvent, RuntimeError> {
        Ok(self.await_event_packet().await?.0)
    }

    pub async fn await_event_packet(
        &mut self,
    ) -> Result<(NnrpClientEvent, RuntimePacket), RuntimeError> {
        let packet = self.transport.read_packet().await?;
        self.decode_event_packet(packet)
    }

    pub async fn await_event_packet_batch(
        &mut self,
        max_events: usize,
    ) -> Result<Vec<(NnrpClientEvent, RuntimePacket)>, RuntimeError> {
        if max_events == 0 {
            return Err(RuntimeError::UnexpectedMessage(
                "client event batch limit must be greater than zero",
            ));
        }

        let mut events = Vec::with_capacity(max_events);
        events.push(self.await_event_packet().await?);
        while events.len() < max_events {
            let Some(packet) = self.transport.try_read_packet()? else {
                break;
            };
            events.push(self.decode_event_packet(packet)?);
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
                self.complete_operation_by_frame(packet.header.frame_id)?;
                Ok(NnrpClientEvent::Result(NnrpResult {
                    frame_id: packet.header.frame_id,
                    metadata,
                    body: packet.body,
                }))
            }
            MessageType::ResultDrop => {
                self.require_session_packet(&packet, "client received drop for another session")?;
                validate_result_drop_header(&packet.header)?;
                self.complete_operation_by_frame(packet.header.frame_id)?;
                Ok(NnrpClientEvent::ResultDrop {
                    frame_id: packet.header.frame_id,
                })
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
                Ok(NnrpClientEvent::ResultDropReason {
                    metadata,
                    body: packet.body,
                })
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

    fn complete_operation_by_frame(&mut self, frame_id: u32) -> Result<(), RuntimeError> {
        let operation_id =
            self.frame_operations
                .remove(&frame_id)
                .ok_or(RuntimeError::UnexpectedMessage(
                    "client terminal event references an unknown frame",
                ))?;
        self.operation_frames.remove(&operation_id);
        Ok(())
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
        let mut header = CommonHeader::new(
            MessageType::SessionPatch,
            SESSION_PATCH_METADATA_LEN as u32,
            patch.profile_patch_bytes,
        );
        header.session_id = self.session_id;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                patch.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await?;

        let ack_packet = self.transport.read_packet().await?;
        if ack_packet.header.message_type != MessageType::SessionPatchAck {
            return Err(RuntimeError::UnexpectedMessage(
                "client expected SESSION_PATCH_ACK",
            ));
        }
        self.require_session_packet(&ack_packet, "client received patch ack for another session")?;
        if ack_packet.metadata.len() != SESSION_PATCH_ACK_METADATA_LEN {
            return Err(RuntimeError::UnexpectedMessage(
                "client received malformed SESSION_PATCH_ACK metadata length",
            ));
        }
        Ok(SessionPatchAckMetadata::parse(&ack_packet.metadata)?)
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
            .field("transport", &self.transport.transport_kind())
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
