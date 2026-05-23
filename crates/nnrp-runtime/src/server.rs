use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use nnrp_core::{
    validate_profile_assignment, validate_result_drop_header, CacheObjectId, CacheObjectKind,
    CommonHeader, ConnectionLifecycle, FlowUpdateMetadata, FrameSubmitMetadata, MessageType,
    OperationCancelRequest, OperationDescriptor, OperationRegistry, ResultPushMetadata,
    SchemaRegistry, SessionCloseAckMetadata, SessionCloseMetadata, SessionCloseStatus,
    SessionOpenAckMetadata, SessionOpenMetadata, SessionPatchAckMetadata, SessionPatchMetadata,
    SessionStatus, FLOW_UPDATE_METADATA_LEN, FRAME_SUBMIT_METADATA_LEN, RESULT_PUSH_METADATA_LEN,
    SESSION_ACK_FLAG_RESUME_ENABLED, SESSION_CLOSE_ACK_METADATA_LEN, SESSION_ERROR_LIMIT_REACHED,
    SESSION_ERROR_NONE, SESSION_ERROR_PROFILE_UNSUPPORTED, SESSION_ERROR_RESUME_REJECTED,
    SESSION_ERROR_SCHEMA_UNSUPPORTED, SESSION_FLAG_ALLOW_RESUME, SESSION_OPEN_ACK_METADATA_LEN,
    SESSION_PATCH_ACK_METADATA_LEN, SESSION_PATCH_METADATA_LEN,
};
use tokio::net::TcpListener;

use crate::{FramedTransport, RuntimeError, RuntimePacket, RuntimeTransportKind, TcpTransport};

#[derive(Debug, Clone)]
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

        Ok(())
    }
}

#[derive(Debug)]
pub struct NnrpServer {
    listener: TcpListener,
    config: NnrpServerConfig,
    sessions: SharedSessionRegistry,
}

#[derive(Debug)]
pub struct NnrpServerSession {
    session_id: u32,
    client_open: SessionOpenMetadata,
    transport: TcpTransport,
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
        Ok(Self {
            listener: TcpListener::bind(addr).await?,
            config,
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
        })
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
            "QUIC runtime hook is reserved but not implemented",
        ))
    }

    pub fn local_addr(&self) -> Result<std::net::SocketAddr, RuntimeError> {
        Ok(self.listener.local_addr()?)
    }

    pub fn session_count(&self) -> Result<usize, RuntimeError> {
        Ok(self.session_registry()?.len())
    }

    pub async fn accept(&self) -> Result<NnrpServerSession, RuntimeError> {
        let (stream, _) = self.listener.accept().await?;
        let mut transport = TcpTransport::new(stream);
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
