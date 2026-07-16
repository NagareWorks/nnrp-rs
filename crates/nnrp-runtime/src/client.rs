use std::fmt;

use nnrp_core::{
    validate_control_request_semantics, validate_partial_result_semantics,
    validate_pressure_semantics, validate_progress_semantics, validate_result_drop_header,
    validate_result_drop_reason_semantics, validate_scheduling_semantics, CacheInvalidateMetadata,
    CacheMissMetadata, CacheObjectKind, CacheReferenceMetadata, CapabilityMetadata, CommonHeader,
    ConnectionLifecycle, ControlRequestMetadata, FlowUpdateMetadata, FrameSubmitMetadata,
    InFlightPolicy, MessageType, ObjectDeltaMetadata, ObjectDescriptorMetadata,
    ObjectReferenceMetadata, ObjectReleaseMetadata, PartialResultMetadata, PressureMetadata,
    ProgressMetadata, ResultDropReasonMetadata, ResultPushMetadata, RouteHintMetadata,
    SchedulingMetadata, SessionCloseAckMetadata, SessionCloseMetadata, SessionCloseReason,
    SessionMigrateAckMetadata, SessionMigrateMetadata, SessionOpenAckMetadata, SessionOpenMetadata,
    SessionPatchAckMetadata, SessionPatchMetadata, SessionPriorityClass, SessionStatus,
    TransportId, CACHE_INVALIDATE_METADATA_LEN, CACHE_MISS_METADATA_LEN,
    CACHE_REFERENCE_METADATA_LEN, CAPABILITY_METADATA_LEN,
    CONTROL_REQUEST_FLAG_COOPERATIVE_ALLOWED, CONTROL_REQUEST_FLAG_HARD_ABORT_ALLOWED,
    CONTROL_REQUEST_METADATA_LEN, FRAME_SUBMIT_METADATA_LEN, OBJECT_DELTA_METADATA_LEN,
    OBJECT_DESCRIPTOR_METADATA_LEN, OBJECT_REFERENCE_METADATA_LEN, OBJECT_RELEASE_METADATA_LEN,
    PARTIAL_RESULT_METADATA_LEN, PRESSURE_METADATA_LEN, PROGRESS_METADATA_LEN,
    RESULT_DROP_REASON_METADATA_LEN, RESULT_PUSH_METADATA_LEN, ROUTE_HINT_METADATA_LEN,
    SCHEDULING_FLAG_DISCARD_STALE, SCHEDULING_FLAG_EMIT_DROP_REASON, SCHEDULING_METADATA_LEN,
    SESSION_CLOSE_ACK_METADATA_LEN, SESSION_CLOSE_METADATA_LEN, SESSION_ERROR_NONE,
    SESSION_MIGRATE_ACK_METADATA_LEN, SESSION_MIGRATE_METADATA_LEN, SESSION_OPEN_METADATA_LEN,
    SESSION_PATCH_ACK_METADATA_LEN, SESSION_PATCH_METADATA_LEN, STANDARD_PROFILE_TOKEN,
    TOKEN_DELTA_SCHEMA_ID, TOKEN_DELTA_SCHEMA_VERSION,
};

use crate::{
    BoxedFramedTransport, FramedTransport, RuntimeError, RuntimePacket, RuntimePressureState,
    RuntimeTransportKind, TcpTransport,
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
}

impl NnrpClient {
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
        Ok(frame_id)
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
            | NnrpClientEvent::ObjectDeclare { .. }
            | NnrpClientEvent::ObjectRef { .. }
            | NnrpClientEvent::ObjectRelease { .. }
            | NnrpClientEvent::ObjectDelta { .. }
            | NnrpClientEvent::CacheReference { .. }
            | NnrpClientEvent::CacheMiss { .. }
            | NnrpClientEvent::CacheInvalidate(_)
            | NnrpClientEvent::Capability { .. }
            | NnrpClientEvent::RouteHint { .. } => Err(RuntimeError::UnexpectedMessage(
                "client expected RESULT_PUSH but received object/cache event",
            )),
        }
    }

    pub async fn await_event(&mut self) -> Result<NnrpClientEvent, RuntimeError> {
        let packet = self.transport.read_packet().await?;
        match packet.header.message_type {
            MessageType::ResultPush => {
                self.require_session_packet(&packet, "client received result for another session")?;
                if packet.metadata.len() != RESULT_PUSH_METADATA_LEN {
                    return Err(RuntimeError::UnexpectedMessage(
                        "client received malformed RESULT_PUSH metadata length",
                    ));
                }
                Ok(NnrpClientEvent::Result(NnrpResult {
                    frame_id: packet.header.frame_id,
                    metadata: ResultPushMetadata::parse(&packet.metadata)?,
                    body: packet.body,
                }))
            }
            MessageType::ResultDrop => {
                self.require_session_packet(&packet, "client received drop for another session")?;
                validate_result_drop_header(&packet.header)?;
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
                Ok(NnrpClientEvent::Progress {
                    metadata,
                    body: packet.body,
                })
            }
            MessageType::FlowUpdate => {
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
            _ => Err(RuntimeError::UnexpectedMessage(
                "client expected a runtime result or control event",
            )),
        }
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
        let close = SessionCloseMetadata {
            close_reason: SessionCloseReason::ClientShutdown,
            in_flight_policy: InFlightPolicy::Drain,
            drain_timeout_ms: 0,
            last_operation_id: self.next_frame_id.saturating_sub(1) as u64,
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
