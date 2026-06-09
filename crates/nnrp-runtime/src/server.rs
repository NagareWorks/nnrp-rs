use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use nnrp_core::{
    validate_control_request_semantics, validate_partial_result_semantics,
    validate_pressure_semantics, validate_profile_assignment, validate_result_drop_header,
    validate_result_drop_reason_semantics, validate_scheduling_semantics, CacheInvalidateMetadata,
    CacheMissMetadata, CacheObjectId, CacheObjectKind, CacheReferenceMetadata, CommonHeader,
    ConnectionLifecycle, ControlRequestMetadata, FlowUpdateMetadata, FrameSubmitMetadata,
    MessageType, ObjectDeltaMetadata, ObjectDescriptorMetadata, ObjectReferenceMetadata,
    ObjectReleaseMetadata, OperationCancelRequest, OperationDescriptor, OperationRegistry,
    PartialResultMetadata, PressureMetadata, ResultDropReasonMetadata, ResultPushMetadata,
    RuntimeRole, SchedulingMetadata, SchemaRegistry, SessionCloseAckMetadata, SessionCloseMetadata,
    SessionCloseStatus, SessionMigrateAckMetadata, SessionMigrateMetadata, SessionOpenAckMetadata,
    SessionOpenMetadata, SessionPatchAckMetadata, SessionPatchMetadata, SessionStatus,
    CACHE_INVALIDATE_METADATA_LEN, CACHE_MISS_METADATA_LEN, CACHE_REFERENCE_METADATA_LEN,
    CONTROL_REQUEST_METADATA_LEN, FLOW_UPDATE_METADATA_LEN, FRAME_SUBMIT_METADATA_LEN,
    OBJECT_DELTA_METADATA_LEN, OBJECT_DESCRIPTOR_METADATA_LEN, OBJECT_REFERENCE_METADATA_LEN,
    OBJECT_RELEASE_METADATA_LEN, PARTIAL_RESULT_METADATA_LEN, PRESSURE_METADATA_LEN,
    RESULT_DROP_REASON_DEADLINE_EXPIRED, RESULT_DROP_REASON_METADATA_LEN, RESULT_PUSH_METADATA_LEN,
    SCHEDULING_FLAG_EMIT_DROP_REASON, SCHEDULING_METADATA_LEN, SESSION_ACK_FLAG_RESUME_ENABLED,
    SESSION_CLOSE_ACK_METADATA_LEN, SESSION_ERROR_LIMIT_REACHED, SESSION_ERROR_NONE,
    SESSION_ERROR_PROFILE_UNSUPPORTED, SESSION_ERROR_RESUME_REJECTED,
    SESSION_ERROR_SCHEMA_UNSUPPORTED, SESSION_FLAG_ALLOW_RESUME, SESSION_MIGRATE_ACK_METADATA_LEN,
    SESSION_MIGRATE_METADATA_LEN, SESSION_OPEN_ACK_METADATA_LEN, SESSION_PATCH_ACK_METADATA_LEN,
    SESSION_PATCH_METADATA_LEN,
};
use tokio::net::TcpListener;

use crate::{
    BoxedFramedListener, BoxedFramedTransport, FramedListener, RuntimeError, RuntimePacket,
    RuntimeTransportKind, TcpFramedListener,
};

#[derive(Clone)]
pub struct NnrpServerConfig {
    pub transport: RuntimeTransportKind,
    pub supported_profiles: Vec<u16>,
    pub supported_cache_objects: Vec<CacheObjectKind>,
    pub max_cache_objects: usize,
    pub max_cache_object_bytes: u32,
    pub schema_registry: SchemaRegistry,
    pub resume_token_bytes: u32,
    pub max_in_flight_operations: u16,
    pub granted_operation_credit: u16,
    pub lease_ttl_ms: u32,
    pub resume_window_ms: u32,
    pub application_policy: Arc<dyn NnrpServerPolicy>,
}

impl fmt::Debug for NnrpServerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NnrpServerConfig")
            .field("transport", &self.transport)
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

pub trait NnrpServerPolicy: Send + Sync {
    fn validate_session_open(&self, open: &SessionOpenMetadata) -> Result<(), u32>;
}

#[derive(Debug, Default)]
pub struct AllowAllServerPolicy;

impl NnrpServerPolicy for AllowAllServerPolicy {
    fn validate_session_open(&self, _open: &SessionOpenMetadata) -> Result<(), u32> {
        Ok(())
    }
}

impl Default for NnrpServerConfig {
    fn default() -> Self {
        Self {
            transport: RuntimeTransportKind::Tcp,
            supported_profiles: vec![nnrp_core::PROFILE_TOKEN],
            supported_cache_objects: Vec::new(),
            max_cache_objects: 0,
            max_cache_object_bytes: 0,
            schema_registry: SchemaRegistry::with_standard_preview3_profiles(),
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
    pub fn with_transport(mut self, transport: RuntimeTransportKind) -> Self {
        self.transport = transport;
        self
    }

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

    pub fn with_cache_limits(mut self, max_objects: usize, max_object_bytes: u32) -> Self {
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

    fn validate_client_open(&self, open: &SessionOpenMetadata) -> Result<(), u32> {
        if !self.supported_profiles.contains(&open.profile_id)
            || validate_profile_assignment(open.profile_id).is_err()
        {
            return Err(SESSION_ERROR_PROFILE_UNSUPPORTED);
        }

        if self
            .schema_registry
            .get(open.schema_id, open.schema_version)
            .is_none()
        {
            return Err(SESSION_ERROR_SCHEMA_UNSUPPORTED);
        }

        if open.max_in_flight_operations > self.max_in_flight_operations {
            return Err(SESSION_ERROR_LIMIT_REACHED);
        }

        self.application_policy.validate_session_open(open)?;

        Ok(())
    }
}

pub struct NnrpServer {
    listener: BoxedFramedListener,
    config: NnrpServerConfig,
    sessions: SharedSessionRegistry,
}

pub struct NnrpServerSession {
    session_id: u32,
    client_open: SessionOpenMetadata,
    transport: BoxedFramedTransport,
    lifecycle: ConnectionLifecycle,
    operations: OperationRegistry,
    cache_objects: Vec<CacheObjectId>,
    max_cache_objects: usize,
    sessions: SharedSessionRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSessionRecord {
    pub session_id: u32,
    pub profile_id: u16,
    pub schema_id: u32,
    pub schema_version: u32,
    pub resume_enabled: bool,
    pub resume_token_bytes: u32,
    pub last_operation_id: u64,
}

type SharedSessionRegistry = Arc<Mutex<BTreeMap<u32, RuntimeSessionRecord>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpSubmit {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpRuntimeControl {
    pub message_type: MessageType,
    pub metadata: ControlRequestMetadata,
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

impl NnrpServer {
    pub async fn bind_tcp(
        addr: impl tokio::net::ToSocketAddrs,
        config: NnrpServerConfig,
    ) -> Result<Self, RuntimeError> {
        if config.transport != RuntimeTransportKind::Tcp {
            return Err(RuntimeError::UnsupportedTransport(
                "server config selected a non-TCP transport for bind_tcp",
            ));
        }
        Self::from_listener(
            TcpFramedListener::new(TcpListener::bind(addr).await?),
            config,
        )
    }

    pub async fn bind_quic(
        _endpoint: &str,
        config: NnrpServerConfig,
    ) -> Result<Self, RuntimeError> {
        if config.transport != RuntimeTransportKind::Quic {
            return Err(RuntimeError::UnsupportedTransport(
                "server config selected a non-QUIC transport for bind_quic",
            ));
        }
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
        if listener.transport_kind() != config.transport {
            return Err(RuntimeError::UnsupportedTransport(
                "server config transport does not match the provided listener slot",
            ));
        }
        Ok(Self {
            listener,
            config,
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub fn local_addr(&self) -> Result<std::net::SocketAddr, RuntimeError> {
        self.listener.local_addr()
    }

    pub fn session_count(&self) -> Result<usize, RuntimeError> {
        Ok(self.session_registry()?.len())
    }

    pub async fn accept(&self) -> Result<NnrpServerSession, RuntimeError> {
        let mut transport = self.listener.accept().await?;
        let packet = transport.read_packet().await?;
        if packet.header.message_type != MessageType::SessionOpen {
            return Err(RuntimeError::UnexpectedMessage(
                "server expected SESSION_OPEN",
            ));
        }

        let open = SessionOpenMetadata::parse(&packet.metadata)?;
        nnrp_core::validate_session_recovery_request(&open)?;
        let ack = self.accept_ack(&open);
        let mut ack_bytes = vec![0u8; SESSION_OPEN_ACK_METADATA_LEN];
        ack.write(&mut ack_bytes)?;

        let mut ack_header = CommonHeader::new(
            MessageType::SessionOpenAck,
            SESSION_OPEN_ACK_METADATA_LEN as u32,
            0,
        );
        ack_header.session_id = ack.session_id;
        transport
            .write_packet(&RuntimePacket::new(ack_header, ack_bytes, Vec::new())?)
            .await?;

        if !matches!(
            ack.session_status,
            SessionStatus::Opened | SessionStatus::Resumed
        ) {
            return Err(RuntimeError::UnexpectedMessage(
                "server rejected SESSION_OPEN",
            ));
        }

        let mut lifecycle = ConnectionLifecycle::new();
        lifecycle.apply_session_open_ack(&ack)?;
        self.session_registry()?.insert(
            ack.session_id,
            RuntimeSessionRecord {
                session_id: ack.session_id,
                profile_id: ack.accepted_profile_id,
                schema_id: ack.schema_id,
                schema_version: ack.schema_version,
                resume_enabled: ack.session_flags_ack & SESSION_ACK_FLAG_RESUME_ENABLED != 0,
                resume_token_bytes: ack.resume_token_bytes,
                last_operation_id: 0,
            },
        );

        Ok(NnrpServerSession {
            session_id: ack.session_id,
            client_open: open,
            transport,
            lifecycle,
            operations: OperationRegistry::new(),
            cache_objects: Vec::new(),
            max_cache_objects: self.config.max_cache_objects,
            sessions: Arc::clone(&self.sessions),
        })
    }

    fn accept_ack(&self, open: &SessionOpenMetadata) -> SessionOpenAckMetadata {
        let validation_error = self.config.validate_client_open(open).err();
        let resume_attempt = open.resume_token_bytes > 0;
        let existing_session = self
            .session_registry()
            .ok()
            .and_then(|registry| registry.get(&open.requested_session_id).cloned());
        let known_resume = resume_attempt
            && existing_session
                .as_ref()
                .filter(|record| record.resume_enabled)
                .is_some();
        let recovery_error = if resume_attempt && !known_resume {
            Some(SESSION_ERROR_RESUME_REJECTED)
        } else if !resume_attempt && existing_session.is_some() {
            Some(SESSION_ERROR_LIMIT_REACHED)
        } else {
            None
        };
        let accepted = validation_error.is_none() && recovery_error.is_none();
        let session_id = if accepted {
            open.requested_session_id.max(1)
        } else {
            0
        };
        let resume_enabled = open.session_flags & SESSION_FLAG_ALLOW_RESUME != 0;
        let ack_resume_token_bytes = if accepted && resume_enabled {
            self.config.resume_token_bytes
        } else {
            0
        };
        SessionOpenAckMetadata {
            session_id,
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
            granted_operation_credit: self.config.granted_operation_credit,
            max_in_flight_operations: self.config.max_in_flight_operations,
            lease_ttl_ms: self.config.lease_ttl_ms,
            resume_window_ms: self.config.resume_window_ms,
            resume_token_bytes: ack_resume_token_bytes,
            session_extension_bytes: 0,
            server_session_tag: session_id as u64,
            route_scope_id: 0,
            session_error_code: validation_error
                .or(recovery_error)
                .unwrap_or(SESSION_ERROR_NONE),
            session_flags_ack: if ack_resume_token_bytes > 0 {
                SESSION_ACK_FLAG_RESUME_ENABLED
            } else {
                0
            },
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

impl fmt::Debug for NnrpServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NnrpServer")
            .field("transport", &self.listener.transport_kind())
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for NnrpServerSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NnrpServerSession")
            .field("session_id", &self.session_id)
            .field("client_open", &self.client_open)
            .field("transport", &self.transport.transport_kind())
            .field("lifecycle", &self.lifecycle)
            .field("operations", &self.operations)
            .field("cache_objects", &self.cache_objects)
            .field("max_cache_objects", &self.max_cache_objects)
            .finish_non_exhaustive()
    }
}

impl NnrpServerSession {
    pub fn session_id(&self) -> u32 {
        self.session_id
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

    pub fn cache_object_count(&self) -> usize {
        self.cache_objects.len()
    }

    pub async fn receive_submit(&mut self) -> Result<NnrpSubmit, RuntimeError> {
        let packet = self.transport.read_packet().await?;
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

        self.operations.register(OperationDescriptor::new(
            self.session_id,
            packet.header.frame_id as u64,
        ))?;
        self.update_registry_last_operation(packet.header.frame_id as u64)?;

        Ok(NnrpSubmit {
            frame_id: packet.header.frame_id,
            metadata: FrameSubmitMetadata::parse(&packet.metadata)?,
            body: packet.body,
        })
    }

    pub async fn send_result(
        &mut self,
        frame_id: u32,
        metadata: ResultPushMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        if let Some(schedule) = self
            .operations
            .expire_if_stale(frame_id as u64, current_unix_ms())?
        {
            if schedule.flags & SCHEDULING_FLAG_EMIT_DROP_REASON != 0 {
                self.send_result_drop_reason(ResultDropReasonMetadata {
                    operation_id: frame_id as u64,
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
        self.operations.complete(frame_id as u64)?;
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
            .await
    }

    pub async fn send_result_drop(&mut self, frame_id: u32) -> Result<(), RuntimeError> {
        let mut header = CommonHeader::new(MessageType::ResultDrop, 0, 0);
        header.session_id = self.session_id;
        header.frame_id = frame_id;
        validate_result_drop_header(&header)?;
        self.transport
            .write_packet(&RuntimePacket::new(header, Vec::new(), Vec::new())?)
            .await
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
        header.frame_id = metadata.operation_id as u32;
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
        validate_result_drop_reason_semantics(&metadata)?;
        let mut header = CommonHeader::new(
            MessageType::ResultDropReason,
            RESULT_DROP_REASON_METADATA_LEN as u32,
            0,
        );
        header.session_id = self.session_id;
        header.frame_id = metadata.operation_id as u32;
        self.transport
            .write_packet(&RuntimePacket::new(
                header,
                metadata.to_bytes()?.to_vec(),
                Vec::new(),
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
        header.frame_id = metadata.operation_id as u32;
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
        header.frame_id = metadata.operation_id as u32;
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
        metadata: ObjectDeltaMetadata,
        body: Vec<u8>,
    ) -> Result<(), RuntimeError> {
        let expected_body_len =
            metadata.metadata_bytes.saturating_add(metadata.delta_bytes) as usize;
        require_body_len(
            body.len(),
            expected_body_len,
            "server object delta body length mismatch",
        )?;
        let mut header = CommonHeader::new(
            MessageType::ObjectDelta,
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
        self.operations.cancel(OperationCancelRequest {
            session_id: self.session_id,
            operation_id: packet.header.frame_id as u64,
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
        if packet.metadata.len() != CONTROL_REQUEST_METADATA_LEN || !packet.body.is_empty() {
            return Err(RuntimeError::UnexpectedMessage(
                "server received malformed runtime control lengths",
            ));
        }

        let metadata = ControlRequestMetadata::parse(&packet.metadata)?;
        validate_control_request_semantics(packet.header.message_type, &metadata)?;
        self.operations.cancel(OperationCancelRequest {
            session_id: self.session_id,
            operation_id: metadata.operation_id,
            cancel_scope: nnrp_core::CancelScope::Operation,
        })?;
        Ok(NnrpRuntimeControl {
            message_type: packet.header.message_type,
            metadata,
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

    pub fn track_cache_object(&mut self, object_id: CacheObjectId) -> Result<(), RuntimeError> {
        if self.cache_objects.contains(&object_id) {
            return Ok(());
        }
        if self.max_cache_objects != 0 && self.cache_objects.len() >= self.max_cache_objects {
            return Err(RuntimeError::UnexpectedMessage(
                "server cache object limit reached",
            ));
        }
        self.cache_objects.push(object_id);
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
            .await
    }

    pub async fn close(mut self) -> Result<(), RuntimeError> {
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

    fn update_registry_last_operation(&self, operation_id: u64) -> Result<(), RuntimeError> {
        let mut sessions = self.session_registry()?;
        if let Some(record) = sessions.get_mut(&self.session_id) {
            record.last_operation_id = record.last_operation_id.max(operation_id);
        }
        Ok(())
    }

    fn remove_from_registry(&self) -> Result<(), RuntimeError> {
        self.session_registry()?.remove(&self.session_id);
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
