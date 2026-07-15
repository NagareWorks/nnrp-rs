use std::{
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use async_trait::async_trait;
use nnrp_core::{CommonHeader, TransportId, COMMON_HEADER_LEN};
use nnrp_runtime::{
    BoxedFramedListener, BoxedFramedTransport, FramedListener, FramedTransport, NnrpClient,
    NnrpClientConfig, NnrpServer, NnrpServerConfig, RuntimeError, RuntimeFrameLimits,
    RuntimePacket, RuntimeTransportKind,
};
use nnrp_transport_provider::{
    TransportProviderDescriptor, TransportProviderKind, TransportProviderRegistry,
};
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};

#[derive(Debug, Clone)]
pub struct QuicClientEndpointConfig {
    pub bind_addr: SocketAddr,
    pub server_name: String,
    pub root_certificates_der: Vec<Vec<u8>>,
}

impl QuicClientEndpointConfig {
    pub fn localhost_with_root_certificate(certificate_der: impl Into<Vec<u8>>) -> Self {
        Self::with_root_certificate(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            "localhost",
            certificate_der,
        )
    }

    pub fn with_root_certificate(
        bind_addr: SocketAddr,
        server_name: impl Into<String>,
        certificate_der: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            bind_addr,
            server_name: server_name.into(),
            root_certificates_der: vec![certificate_der.into()],
        }
    }

    pub fn with_root_certificates(
        bind_addr: SocketAddr,
        server_name: impl Into<String>,
        root_certificates_der: impl IntoIterator<Item = Vec<u8>>,
    ) -> Self {
        Self {
            bind_addr,
            server_name: server_name.into(),
            root_certificates_der: root_certificates_der.into_iter().collect(),
        }
    }

    pub fn client_config(&self) -> Result<ClientConfig, RuntimeError> {
        let mut roots = rustls::RootCertStore::empty();
        for certificate in &self.root_certificates_der {
            roots
                .add(CertificateDer::from(certificate.clone()))
                .map_err(runtime_io)?;
        }
        ClientConfig::with_root_certificates(Arc::new(roots)).map_err(runtime_io)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuicSelfSignedCertificate {
    pub certificate_der: Vec<u8>,
    pub private_key_pkcs8_der: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct QuicServerEndpointConfig {
    pub bind_addr: SocketAddr,
    pub certificate_chain_der: Vec<Vec<u8>>,
    pub private_key_pkcs8_der: Vec<u8>,
}

impl QuicServerEndpointConfig {
    pub fn with_single_certificate(
        bind_addr: SocketAddr,
        certificate_der: impl Into<Vec<u8>>,
        private_key_pkcs8_der: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            bind_addr,
            certificate_chain_der: vec![certificate_der.into()],
            private_key_pkcs8_der: private_key_pkcs8_der.into(),
        }
    }

    pub fn self_signed_localhost(
        bind_addr: SocketAddr,
    ) -> Result<(Self, QuicSelfSignedCertificate), RuntimeError> {
        let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
            .map_err(runtime_io)?;
        let certificate_der = certified.cert.der().to_vec();
        let private_key_pkcs8_der = certified.signing_key.serialize_der();
        let config = Self::with_single_certificate(
            bind_addr,
            certificate_der.clone(),
            private_key_pkcs8_der.clone(),
        );

        Ok((
            config,
            QuicSelfSignedCertificate {
                certificate_der,
                private_key_pkcs8_der,
            },
        ))
    }

    pub fn server_config(&self) -> Result<ServerConfig, RuntimeError> {
        let certificate_chain = self
            .certificate_chain_der
            .iter()
            .cloned()
            .map(CertificateDer::from)
            .collect();
        let private_key = PrivatePkcs8KeyDer::from(self.private_key_pkcs8_der.clone());
        let mut server_config =
            ServerConfig::with_single_cert(certificate_chain, private_key.into())
                .map_err(runtime_io)?;
        if let Some(transport_config) = Arc::get_mut(&mut server_config.transport) {
            transport_config.max_concurrent_uni_streams(0_u8.into());
        }
        Ok(server_config)
    }
}

#[derive(Debug)]
pub struct QuicTransport {
    _endpoint: Endpoint,
    connection: Connection,
    send: Option<SendStream>,
    recv: Option<RecvStream>,
    initiator: bool,
    limits: RuntimeFrameLimits,
}

impl QuicTransport {
    pub async fn connect(
        addr: SocketAddr,
        config: &QuicClientEndpointConfig,
    ) -> Result<Self, RuntimeError> {
        Self::connect_with_limits(addr, config, RuntimeFrameLimits::default()).await
    }

    pub async fn connect_with_limits(
        addr: SocketAddr,
        config: &QuicClientEndpointConfig,
        limits: RuntimeFrameLimits,
    ) -> Result<Self, RuntimeError> {
        let mut endpoint = Endpoint::client(config.bind_addr)?;
        endpoint.set_default_client_config(config.client_config()?);
        let connection = endpoint
            .connect(addr, &config.server_name)
            .map_err(runtime_io)?
            .await
            .map_err(runtime_io)?;
        Ok(Self {
            _endpoint: endpoint,
            connection,
            send: None,
            recv: None,
            initiator: true,
            limits,
        })
    }

    fn new(endpoint: Endpoint, connection: Connection, limits: RuntimeFrameLimits) -> Self {
        Self {
            _endpoint: endpoint,
            connection,
            send: None,
            recv: None,
            initiator: false,
            limits,
        }
    }

    async fn ensure_streams(&mut self) -> Result<(), RuntimeError> {
        if self.send.is_some() && self.recv.is_some() {
            return Ok(());
        }
        let (send, recv) = if self.initiator {
            self.connection.open_bi().await.map_err(runtime_io)?
        } else {
            self.connection.accept_bi().await.map_err(runtime_io)?
        };
        self.send = Some(send);
        self.recv = Some(recv);
        Ok(())
    }
}

#[async_trait]
impl FramedTransport for QuicTransport {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::Quic
    }

    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
        self.ensure_streams().await?;
        let recv = self
            .recv
            .as_mut()
            .expect("QUIC receive stream is initialized");
        let mut header_bytes = [0u8; COMMON_HEADER_LEN];
        recv.read_exact(&mut header_bytes)
            .await
            .map_err(runtime_io)?;
        let header = CommonHeader::parse(&header_bytes)?;
        self.limits.validate_packet_len(header.packet_len()?)?;

        let mut metadata = vec![0u8; header.meta_len as usize];
        if !metadata.is_empty() {
            recv.read_exact(&mut metadata).await.map_err(runtime_io)?;
        }

        let mut body = vec![0u8; header.body_len as usize];
        if !body.is_empty() {
            recv.read_exact(&mut body).await.map_err(runtime_io)?;
        }

        Ok(RuntimePacket::from_parts(header, metadata, body)?)
    }

    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
        self.ensure_streams().await?;
        let bytes = packet.to_bytes()?;
        self.limits.validate_packet_len(bytes.len())?;
        self.send
            .as_mut()
            .expect("QUIC send stream is initialized")
            .write_all(&bytes)
            .await
            .map_err(runtime_io)?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RuntimeError> {
        if let Some(send) = &mut self.send {
            let _ = send.finish();
            let _ = send.stopped().await;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct QuicFramedListener {
    endpoint: Endpoint,
    limits: RuntimeFrameLimits,
}

impl QuicFramedListener {
    pub fn bind(config: &QuicServerEndpointConfig) -> Result<Self, RuntimeError> {
        Self::bind_with_limits(config, RuntimeFrameLimits::default())
    }

    pub fn bind_with_limits(
        config: &QuicServerEndpointConfig,
        limits: RuntimeFrameLimits,
    ) -> Result<Self, RuntimeError> {
        Ok(Self {
            endpoint: Endpoint::server(config.server_config()?, config.bind_addr)?,
            limits,
        })
    }
}

#[async_trait]
impl FramedListener for QuicFramedListener {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::Quic
    }

    fn local_addr(&self) -> Result<SocketAddr, RuntimeError> {
        Ok(self.endpoint.local_addr()?)
    }

    async fn accept(&self) -> Result<BoxedFramedTransport, RuntimeError> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or(RuntimeError::Internal("QUIC endpoint closed"))?;
        let connection = incoming.await.map_err(runtime_io)?;
        Ok(Box::new(QuicTransport::new(
            self.endpoint.clone(),
            connection,
            self.limits,
        )))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct QuicProvider;

impl QuicProvider {
    pub const NAME: &'static str = "nnrp-transport-quic";

    pub fn descriptor() -> TransportProviderDescriptor {
        TransportProviderDescriptor::available(
            Self::NAME,
            env!("CARGO_PKG_VERSION"),
            TransportId::Quic,
            TransportProviderKind::PureRust,
        )
    }

    pub fn backend_descriptor(
        name: impl Into<String>,
        version: impl Into<String>,
        kind: TransportProviderKind,
    ) -> TransportProviderDescriptor {
        TransportProviderDescriptor::available(name, version, TransportId::Quic, kind)
    }

    pub fn register(registry: &mut TransportProviderRegistry) {
        registry.register(Self::descriptor());
    }

    pub async fn connect(
        endpoint: &str,
        endpoint_config: QuicClientEndpointConfig,
        config: NnrpClientConfig,
    ) -> Result<NnrpClient, RuntimeError> {
        let addr = resolve_endpoint(endpoint)?;
        Self::connect_addr(addr, endpoint_config, config).await
    }

    pub async fn connect_addr(
        addr: SocketAddr,
        endpoint_config: QuicClientEndpointConfig,
        config: NnrpClientConfig,
    ) -> Result<NnrpClient, RuntimeError> {
        NnrpClient::from_transport(
            QuicTransport::connect(addr, &endpoint_config).await?,
            config,
        )
    }

    pub fn from_transport<T>(
        transport: T,
        config: NnrpClientConfig,
    ) -> Result<NnrpClient, RuntimeError>
    where
        T: FramedTransport + 'static,
    {
        NnrpClient::from_transport(transport, config)
    }

    pub fn from_boxed_transport(
        transport: BoxedFramedTransport,
        config: NnrpClientConfig,
    ) -> Result<NnrpClient, RuntimeError> {
        NnrpClient::from_boxed_transport(transport, config)
    }

    pub async fn bind(
        endpoint_config: QuicServerEndpointConfig,
        config: NnrpServerConfig,
    ) -> Result<NnrpServer, RuntimeError> {
        NnrpServer::from_listener(QuicFramedListener::bind(&endpoint_config)?, config)
    }

    pub fn from_listener<L>(
        listener: L,
        config: NnrpServerConfig,
    ) -> Result<NnrpServer, RuntimeError>
    where
        L: FramedListener + 'static,
    {
        NnrpServer::from_listener(listener, config)
    }

    pub fn from_boxed_listener(
        listener: BoxedFramedListener,
        config: NnrpServerConfig,
    ) -> Result<NnrpServer, RuntimeError> {
        NnrpServer::from_boxed_listener(listener, config)
    }
}

pub fn register_quic_provider(registry: &mut TransportProviderRegistry) {
    QuicProvider::register(registry);
}

pub fn quic_client_config(config: NnrpClientConfig) -> NnrpClientConfig {
    config.with_transport(RuntimeTransportKind::Quic)
}

pub fn quic_server_config(config: NnrpServerConfig) -> NnrpServerConfig {
    config.with_transport(RuntimeTransportKind::Quic)
}

fn resolve_endpoint(endpoint: &str) -> Result<SocketAddr, RuntimeError> {
    endpoint
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "endpoint did not resolve"))
        .map_err(RuntimeError::Io)
}

fn runtime_io(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> RuntimeError {
    RuntimeError::Io(io::Error::other(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use nnrp_core::{
        CommonHeader, FrameSubmitMetadata, InputProfile, MessageType, PayloadKindBitmap,
        ResultClass, ResultPushMetadata, SessionCloseReason, SubmitMode, TileIndexMode,
    };
    use nnrp_runtime::{RuntimePacket, RuntimeTransportKind};
    use nnrp_transport_provider::{RemoteTransportSupport, TransportPolicy};

    #[test]
    fn quic_provider_registers_available_backend_descriptor() {
        let mut registry = TransportProviderRegistry::new();
        register_quic_provider(&mut registry);

        assert_eq!(registry.providers().len(), 1);
        assert_eq!(registry.providers()[0].name, QuicProvider::NAME);
        assert_eq!(registry.providers()[0].transport_id, TransportId::Quic);
        assert_eq!(
            registry.providers()[0].kind,
            TransportProviderKind::PureRust
        );
        assert!(registry.providers()[0].available);
        assert_eq!(registry.providers()[0].diagnostic, None);
    }

    #[test]
    fn quic_backend_descriptor_participates_in_policy_selection() {
        let registry =
            TransportProviderRegistry::new().with_provider(QuicProvider::backend_descriptor(
                "quic-custom",
                "0.0.0",
                TransportProviderKind::NativeDynamic,
            ));
        let remote = RemoteTransportSupport::new([TransportId::Quic]);
        let selection = registry
            .select(&remote, TransportPolicy::ForceQuic, None)
            .expect("available quic backend should satisfy force quic");

        assert_eq!(selection.selected.transport_id, TransportId::Quic);
        assert_eq!(selection.selected.name, "quic-custom");
    }

    #[tokio::test]
    async fn quic_provider_runs_loopback_session_with_self_signed_certificate(
    ) -> Result<(), RuntimeError> {
        let (endpoint_config, certificate) =
            QuicServerEndpointConfig::self_signed_localhost(stub_addr())?;
        let server = QuicProvider::bind(
            endpoint_config,
            quic_server_config(NnrpServerConfig::default()),
        )
        .await?;
        let addr = server.local_addr()?;

        let server_task = tokio::spawn(async move {
            let mut session = server.accept().await?;
            let submit = session.receive_submit().await?;
            assert_eq!(submit.frame_id, 1);
            assert_eq!(submit.body, b"prompt".to_vec());
            session
                .send_result(submit.frame_id, token_result(), b"delta".to_vec())
                .await?;
            let close = session.receive_close().await?;
            assert_eq!(close.close_reason, SessionCloseReason::ClientShutdown);
            session.ack_close(&close).await?;
            session.close().await
        });

        let endpoint_config =
            QuicClientEndpointConfig::localhost_with_root_certificate(certificate.certificate_der);
        let client = QuicProvider::connect_addr(
            addr,
            endpoint_config,
            quic_client_config(NnrpClientConfig::default()),
        )
        .await?;
        let mut session = client.open_session().await?;
        let frame_id = session.submit(token_submit(), b"prompt".to_vec()).await?;
        let result = session.await_result().await?;
        assert_eq!(result.frame_id, frame_id);
        assert_eq!(result.body, b"delta".to_vec());
        session.close().await?;
        server_task.await.expect("server task should join")?;
        Ok(())
    }

    #[test]
    fn quic_self_signed_config_exposes_client_material() -> Result<(), RuntimeError> {
        let (config, certificate) = QuicServerEndpointConfig::self_signed_localhost(stub_addr())?;

        assert_eq!(config.bind_addr, stub_addr());
        assert_eq!(
            config.certificate_chain_der,
            vec![certificate.certificate_der]
        );
        assert!(!certificate.private_key_pkcs8_der.is_empty());

        let client_config = QuicClientEndpointConfig::localhost_with_root_certificate(
            config.certificate_chain_der[0].clone(),
        );
        assert_eq!(client_config.server_name, "localhost");
        assert_eq!(client_config.root_certificates_der.len(), 1);
        Ok(())
    }

    #[test]
    fn quic_injection_helpers_validate_transport_kind() {
        let client =
            QuicProvider::from_transport(StubQuicTransport, quic_client_config(Default::default()))
                .expect("quic transport injection should be accepted");
        assert!(format!("{client:?}").contains("Quic"));

        let boxed_client = QuicProvider::from_boxed_transport(
            Box::new(StubQuicTransport),
            quic_client_config(Default::default()),
        )
        .expect("boxed quic transport injection should be accepted");
        assert!(format!("{boxed_client:?}").contains("Quic"));

        let mismatch = QuicProvider::from_transport(StubQuicTransport, NnrpClientConfig::default())
            .expect_err("quic transport should not bind to tcp config");
        assert!(matches!(
            mismatch,
            RuntimeError::UnsupportedTransport(
                "client config transport does not match the provided transport slot"
            )
        ));
    }

    #[test]
    fn quic_listener_injection_helpers_validate_transport_kind() {
        let server = QuicProvider::from_listener(
            StubQuicListener,
            quic_server_config(NnrpServerConfig::default()),
        )
        .expect("quic listener injection should be accepted");
        assert_eq!(
            server.local_addr().expect("stub listener has an address"),
            stub_addr()
        );

        let boxed_server = QuicProvider::from_boxed_listener(
            Box::new(StubQuicListener),
            quic_server_config(NnrpServerConfig::default()),
        )
        .expect("boxed quic listener injection should be accepted");
        assert_eq!(
            boxed_server
                .local_addr()
                .expect("boxed stub listener has an address"),
            stub_addr()
        );

        let mismatch = QuicProvider::from_listener(StubQuicListener, NnrpServerConfig::default())
            .expect_err("quic listener should not bind to tcp config");
        assert!(matches!(
            mismatch,
            RuntimeError::UnsupportedTransport(
                "server config transport does not match the provided listener slot"
            )
        ));
    }

    #[tokio::test]
    async fn stub_quic_transport_reports_scripted_errors_and_closes() {
        let mut transport = StubQuicTransport;
        let write_packet = RuntimePacket::new(
            CommonHeader::new(MessageType::Ping, 0, 0),
            Vec::new(),
            Vec::new(),
        )
        .expect("packet shape is valid");

        assert!(matches!(
            transport.read_packet().await,
            Err(RuntimeError::Internal("stub read"))
        ));
        assert!(matches!(
            transport.write_packet(&write_packet).await,
            Err(RuntimeError::Internal("stub write"))
        ));
        transport.close().await.expect("stub close succeeds");
    }

    fn stub_addr() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)
    }

    fn token_submit() -> FrameSubmitMetadata {
        FrameSubmitMetadata {
            src_width: 0,
            src_height: 0,
            tile_width: 0,
            tile_height: 0,
            tile_count: 0,
            section_count: 0,
            frame_class: 0,
            input_profile: InputProfile::Unspecified,
            tile_index_mode: TileIndexMode::DenseRange,
            latency_budget_ms: 25,
            target_fps_x100: 0,
            retry_of_frame: 0,
            tile_base_id: 0,
            camera_bytes: 0,
            tile_index_bytes: 0,
            submit_mode: SubmitMode::Inline,
            budget_policy: 0,
            loss_tolerance_policy: 0,
            object_ref_mask: 0,
            dependency_frame_id: 0,
            payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
            payload_frame_count: 1,
        }
    }

    fn token_result() -> ResultPushMetadata {
        ResultPushMetadata {
            status_code: 200,
            result_flags: 0,
            section_count: 0,
            tile_count: 0,
            active_profile_id: nnrp_core::STANDARD_PROFILE_TOKEN,
            inference_ms: 3,
            queue_ms: 1,
            server_total_ms: 4,
            tile_base_id: 0,
            tile_index_bytes: 0,
            result_class: ResultClass::Complete,
            applied_budget_policy: 0,
            reused_frame_id: 0,
            covered_tile_count: 0,
            dropped_tile_count: 0,
            payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
            payload_frame_count: 1,
        }
    }

    struct StubQuicTransport;

    #[async_trait]
    impl FramedTransport for StubQuicTransport {
        fn transport_kind(&self) -> RuntimeTransportKind {
            RuntimeTransportKind::Quic
        }

        async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
            Err(RuntimeError::Internal("stub read"))
        }

        async fn write_packet(&mut self, _packet: &RuntimePacket) -> Result<(), RuntimeError> {
            Err(RuntimeError::Internal("stub write"))
        }

        async fn close(&mut self) -> Result<(), RuntimeError> {
            Ok(())
        }
    }

    struct StubQuicListener;

    #[async_trait]
    impl FramedListener for StubQuicListener {
        fn transport_kind(&self) -> RuntimeTransportKind {
            RuntimeTransportKind::Quic
        }

        fn local_addr(&self) -> Result<SocketAddr, RuntimeError> {
            Ok(stub_addr())
        }

        async fn accept(&self) -> Result<BoxedFramedTransport, RuntimeError> {
            Ok(Box::new(StubQuicTransport))
        }
    }
}
