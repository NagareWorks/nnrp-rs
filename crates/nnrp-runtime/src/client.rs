use nnrp_core::{
    CommonHeader, ConnectionLifecycle, MessageType, SessionOpenAckMetadata, SessionOpenMetadata,
    SessionPriorityClass, SessionStatus, SESSION_OPEN_METADATA_LEN, STANDARD_PROFILE_TOKEN,
    TOKEN_DELTA_SCHEMA_ID, TOKEN_DELTA_SCHEMA_VERSION,
};

use crate::{FramedTransport, RuntimeError, RuntimePacket, TcpTransport};

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
}

impl Default for NnrpClientConfig {
    fn default() -> Self {
        Self {
            requested_session_id: 1,
            profile_id: STANDARD_PROFILE_TOKEN,
            schema_id: TOKEN_DELTA_SCHEMA_ID,
            schema_version: TOKEN_DELTA_SCHEMA_VERSION,
            priority_class: SessionPriorityClass::Balanced,
            default_deadline_ms: 500,
            max_in_flight_operations: 4,
            lease_ttl_hint_ms: 30_000,
        }
    }
}

#[derive(Debug)]
pub struct NnrpClient {
    transport: TcpTransport,
    config: NnrpClientConfig,
    lifecycle: ConnectionLifecycle,
}

#[derive(Debug)]
pub struct NnrpClientSession {
    session_id: u32,
    transport: TcpTransport,
    lifecycle: ConnectionLifecycle,
}

impl NnrpClient {
    pub async fn connect_tcp(
        addr: impl tokio::net::ToSocketAddrs,
        config: NnrpClientConfig,
    ) -> Result<Self, RuntimeError> {
        Ok(Self {
            transport: TcpTransport::connect(addr).await?,
            config,
            lifecycle: ConnectionLifecycle::new(),
        })
    }

    pub async fn connect_quic(
        _endpoint: &str,
        _config: NnrpClientConfig,
    ) -> Result<Self, RuntimeError> {
        Err(RuntimeError::UnsupportedTransport(
            "QUIC runtime hook is reserved but not implemented",
        ))
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
            transport: self.transport,
            lifecycle: self.lifecycle,
        })
    }

    fn session_open_metadata(&self) -> SessionOpenMetadata {
        SessionOpenMetadata {
            requested_session_id: self.config.requested_session_id,
            profile_id: self.config.profile_id,
            priority_class: self.config.priority_class,
            session_flags: 0,
            schema_id: self.config.schema_id,
            schema_version: self.config.schema_version,
            default_deadline_ms: self.config.default_deadline_ms,
            max_in_flight_operations: self.config.max_in_flight_operations,
            lease_ttl_hint_ms: self.config.lease_ttl_hint_ms,
            resume_token_bytes: 0,
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

    pub async fn close(mut self) -> Result<(), RuntimeError> {
        self.transport.close().await
    }
}
