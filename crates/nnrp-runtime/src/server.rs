use nnrp_core::{
    validate_result_drop_header, CommonHeader, ConnectionLifecycle, FlowUpdateMetadata,
    FrameSubmitMetadata, MessageType, ResultPushMetadata, SessionCloseAckMetadata,
    SessionCloseMetadata, SessionCloseStatus, SessionOpenAckMetadata, SessionOpenMetadata,
    SessionPatchAckMetadata, SessionPatchMetadata, SessionStatus, FLOW_UPDATE_METADATA_LEN,
    FRAME_SUBMIT_METADATA_LEN, RESULT_PUSH_METADATA_LEN, SESSION_CLOSE_ACK_METADATA_LEN,
    SESSION_ERROR_NONE, SESSION_OPEN_ACK_METADATA_LEN, SESSION_PATCH_ACK_METADATA_LEN,
    SESSION_PATCH_METADATA_LEN,
};
use tokio::net::TcpListener;

use crate::{FramedTransport, RuntimeError, RuntimePacket, TcpTransport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpServerConfig {
    pub max_in_flight_operations: u16,
    pub granted_operation_credit: u16,
    pub lease_ttl_ms: u32,
    pub resume_window_ms: u32,
}

impl Default for NnrpServerConfig {
    fn default() -> Self {
        Self {
            max_in_flight_operations: 4,
            granted_operation_credit: 2,
            lease_ttl_ms: 30_000,
            resume_window_ms: 120_000,
        }
    }
}

#[derive(Debug)]
pub struct NnrpServer {
    listener: TcpListener,
    config: NnrpServerConfig,
}

#[derive(Debug)]
pub struct NnrpServerSession {
    session_id: u32,
    client_open: SessionOpenMetadata,
    transport: TcpTransport,
    lifecycle: ConnectionLifecycle,
}

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
        Ok(Self {
            listener: TcpListener::bind(addr).await?,
            config,
        })
    }

    pub async fn bind_quic(
        _endpoint: &str,
        _config: NnrpServerConfig,
    ) -> Result<Self, RuntimeError> {
        Err(RuntimeError::UnsupportedTransport(
            "QUIC runtime hook is reserved but not implemented",
        ))
    }

    pub fn local_addr(&self) -> Result<std::net::SocketAddr, RuntimeError> {
        Ok(self.listener.local_addr()?)
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

        let mut lifecycle = ConnectionLifecycle::new();
        lifecycle.apply_session_open_ack(&ack)?;

        Ok(NnrpServerSession {
            session_id: ack.session_id,
            client_open: open,
            transport,
            lifecycle,
        })
    }

    fn accept_ack(&self, open: &SessionOpenMetadata) -> SessionOpenAckMetadata {
        let session_id = open.requested_session_id.max(1);
        SessionOpenAckMetadata {
            session_id,
            accepted_profile_id: open.profile_id,
            accepted_priority_class: open.priority_class,
            session_status: SessionStatus::Opened,
            schema_id: open.schema_id,
            schema_version: open.schema_version,
            granted_operation_credit: self.config.granted_operation_credit,
            max_in_flight_operations: self.config.max_in_flight_operations,
            lease_ttl_ms: self.config.lease_ttl_ms,
            resume_window_ms: self.config.resume_window_ms,
            resume_token_bytes: 0,
            session_extension_bytes: 0,
            server_session_tag: session_id as u64,
            route_scope_id: 0,
            session_error_code: SESSION_ERROR_NONE,
            session_flags_ack: 0,
        }
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
        Ok(NnrpCancel {
            frame_id: packet.header.frame_id,
        })
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
}
