use nnrp_core::TransportId;
use nnrp_runtime::{
    BoxedFramedListener, BoxedFramedTransport, FramedListener, FramedTransport, NnrpClient,
    NnrpClientConfig, NnrpServer, NnrpServerConfig, RuntimeError, RuntimeTransportKind,
};
use nnrp_transport_provider::{
    TransportProviderDescriptor, TransportProviderKind, TransportProviderRegistry,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct QuicProvider;

impl QuicProvider {
    pub const NAME: &'static str = "nnrp-transport-quic";
    pub const MISSING_BACKEND_DIAGNOSTIC: &'static str =
        "QUIC transport backend is not selected; inject a FramedTransport/FramedListener or register a native/WASM provider";

    pub fn descriptor() -> TransportProviderDescriptor {
        TransportProviderDescriptor::missing(
            Self::NAME,
            env!("CARGO_PKG_VERSION"),
            TransportId::Quic,
            TransportProviderKind::PureRust,
            Self::MISSING_BACKEND_DIAGNOSTIC,
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
        config: NnrpClientConfig,
    ) -> Result<NnrpClient, RuntimeError> {
        NnrpClient::connect_quic(endpoint, config).await
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
        endpoint: &str,
        config: NnrpServerConfig,
    ) -> Result<NnrpServer, RuntimeError> {
        NnrpServer::bind_quic(endpoint, config).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use nnrp_runtime::{RuntimePacket, RuntimeTransportKind};
    use nnrp_transport_provider::{RemoteTransportSupport, TransportPolicy};

    #[test]
    fn quic_provider_registers_missing_backend_descriptor() {
        let mut registry = TransportProviderRegistry::new();
        register_quic_provider(&mut registry);

        assert_eq!(registry.providers().len(), 1);
        assert_eq!(registry.providers()[0].name, QuicProvider::NAME);
        assert_eq!(registry.providers()[0].transport_id, TransportId::Quic);
        assert_eq!(
            registry.providers()[0].kind,
            TransportProviderKind::PureRust
        );
        assert!(!registry.providers()[0].available);
        assert_eq!(
            registry.providers()[0].diagnostic.as_deref(),
            Some(QuicProvider::MISSING_BACKEND_DIAGNOSTIC)
        );
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
            .select(&remote, TransportPolicy::ForceQuic)
            .expect("available quic backend should satisfy force quic");

        assert_eq!(selection.selected.transport_id, TransportId::Quic);
        assert_eq!(selection.selected.name, "quic-custom");
    }

    #[tokio::test]
    async fn quic_connect_and_bind_report_missing_backend() {
        let client_error =
            QuicProvider::connect("localhost:4433", quic_client_config(Default::default()))
                .await
                .expect_err("default quic package has no concrete backend");
        assert!(matches!(
            client_error,
            RuntimeError::UnsupportedTransport(
                "QUIC provider is not installed; use from_transport with a QUIC FramedTransport"
            )
        ));

        let server_error = QuicProvider::bind(
            "localhost:4433",
            quic_server_config(NnrpServerConfig::default()),
        )
        .await
        .expect_err("default quic package has no concrete backend");
        assert!(matches!(
            server_error,
            RuntimeError::UnsupportedTransport(
                "QUIC provider is not installed; use from_listener with a QUIC FramedListener"
            )
        ));
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
            nnrp_core::CommonHeader::new(nnrp_core::MessageType::Ping, 0, 0),
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
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4433)
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
