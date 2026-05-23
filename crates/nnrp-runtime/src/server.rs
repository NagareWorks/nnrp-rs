use nnrp_core::{
    CommonHeader, ConnectionLifecycle, MessageType, SessionOpenAckMetadata, SessionOpenMetadata,
    SessionStatus, SESSION_ERROR_NONE, SESSION_OPEN_ACK_METADATA_LEN,
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

    pub async fn close(mut self) -> Result<(), RuntimeError> {
        self.transport.close().await
    }
}
