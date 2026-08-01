use std::{io, net::SocketAddr, sync::Arc};

use async_trait::async_trait;
use nnrp_core::TransportId;
use nnrp_runtime::{
    BoundServerProvider, BoxedFramedTransport, ClientTransportSecurity, FramedListener,
    FramedTransport, NnrpClientProvider, NnrpServerProvider, ProviderEndpoint, RuntimeError,
    RuntimeFrameLimits, RuntimePacket, RuntimeTransportKind, ServerTransportSecurity,
    StreamPacketReader, TcpFramedListener, TcpTransport,
};
use nnrp_transport_provider::{
    TransportProviderDescriptor, TransportProviderKind, TransportProviderRegistry,
};
use rustls::{
    pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName},
    ClientConfig, RootCertStore, ServerConfig,
};
use tokio::{
    io::{AsyncWriteExt, ReadHalf, WriteHalf},
    net::{TcpListener, TcpStream, ToSocketAddrs},
};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};

#[derive(Debug)]
pub struct TcpTlsTransport {
    reader: StreamPacketReader,
    read_half: ReadHalf<TlsStream<TcpStream>>,
    write_half: WriteHalf<TlsStream<TcpStream>>,
    limits: RuntimeFrameLimits,
}

impl TcpTlsTransport {
    pub async fn connect(
        addr: impl ToSocketAddrs,
        security: &ClientTransportSecurity,
        limits: RuntimeFrameLimits,
    ) -> Result<Self, RuntimeError> {
        security.validate()?;
        let mut roots = RootCertStore::empty();
        roots
            .add(CertificateDer::from(
                security.trusted_certificate_der.clone(),
            ))
            .map_err(runtime_io)?;
        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let server_name = ServerName::try_from(security.server_name.clone()).map_err(runtime_io)?;
        let stream = TcpStream::connect(addr).await?;
        let stream = TlsConnector::from(Arc::new(config))
            .connect(server_name, stream)
            .await
            .map_err(runtime_io)?;
        Ok(Self::new(stream.into(), limits))
    }

    fn new(stream: TlsStream<TcpStream>, limits: RuntimeFrameLimits) -> Self {
        let (read_half, write_half) = tokio::io::split(stream);
        Self {
            reader: StreamPacketReader::new(),
            read_half,
            write_half,
            limits,
        }
    }
}

#[async_trait]
impl FramedTransport for TcpTlsTransport {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::Tcp
    }

    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
        self.reader
            .read_packet(&mut self.read_half, self.limits)
            .await
    }

    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
        let bytes = packet.to_bytes()?;
        self.limits.validate_packet_len(bytes.len())?;
        self.write_half.write_all(&bytes).await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RuntimeError> {
        normalize_shutdown(self.write_half.shutdown().await)
    }
}

pub struct TcpTlsFramedListener {
    listener: TcpListener,
    acceptor: TlsAcceptor,
    limits: RuntimeFrameLimits,
}

impl std::fmt::Debug for TcpTlsFramedListener {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TcpTlsFramedListener")
            .field("listener", &self.listener)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl TcpTlsFramedListener {
    pub async fn bind(
        addr: impl ToSocketAddrs,
        certificate_der: impl Into<Vec<u8>>,
        private_key_pkcs8_der: impl Into<Vec<u8>>,
        limits: RuntimeFrameLimits,
    ) -> Result<Self, RuntimeError> {
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(certificate_der.into())],
                PrivatePkcs8KeyDer::from(private_key_pkcs8_der.into()).into(),
            )
            .map_err(runtime_io)?;
        Ok(Self {
            listener: TcpListener::bind(addr).await?,
            acceptor: TlsAcceptor::from(Arc::new(config)),
            limits,
        })
    }
}

#[async_trait]
impl FramedListener for TcpTlsFramedListener {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::Tcp
    }

    fn local_addr(&self) -> Result<SocketAddr, RuntimeError> {
        Ok(self.listener.local_addr()?)
    }

    async fn accept(&self) -> Result<BoxedFramedTransport, RuntimeError> {
        let (stream, _) = self.listener.accept().await?;
        let stream = self.acceptor.accept(stream).await.map_err(runtime_io)?;
        Ok(Box::new(TcpTlsTransport::new(stream.into(), self.limits)))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TcpProvider;

impl TcpProvider {
    pub const NAME: &'static str = "nnrp-transport-tcp";

    pub fn descriptor() -> TransportProviderDescriptor {
        TransportProviderDescriptor::available(
            Self::NAME,
            env!("CARGO_PKG_VERSION"),
            TransportId::Tcp,
            TransportProviderKind::PureRust,
        )
    }

    pub fn register(
        registry: &mut TransportProviderRegistry,
    ) -> Result<(), nnrp_transport_provider::TransportProviderRegistryError> {
        registry.register(Self::descriptor())
    }

    pub async fn connect(
        addr: impl tokio::net::ToSocketAddrs,
    ) -> Result<TcpTransport, RuntimeError> {
        TcpTransport::connect(addr).await
    }

    pub async fn bind(
        addr: impl tokio::net::ToSocketAddrs,
    ) -> Result<TcpFramedListener, RuntimeError> {
        TcpFramedListener::bind(addr).await
    }
}

#[async_trait]
impl NnrpClientProvider for TcpProvider {
    fn descriptor(&self) -> TransportProviderDescriptor {
        TcpProvider::descriptor()
    }

    async fn connect(
        &self,
        endpoint: &ProviderEndpoint,
        security: Option<&ClientTransportSecurity>,
        limits: RuntimeFrameLimits,
    ) -> Result<BoxedFramedTransport, RuntimeError> {
        let addr =
            endpoint
                .as_str()
                .strip_prefix("tcp://")
                .ok_or(RuntimeError::UnsupportedTransport(
                    "TCP provider endpoint must use tcp://",
                ))?;
        match security {
            Some(security) => Ok(Box::new(
                TcpTlsTransport::connect(addr, security, limits).await?,
            )),
            None => Ok(Box::new(
                TcpTransport::connect_with_limits(addr, limits).await?,
            )),
        }
    }
}

#[async_trait]
impl NnrpServerProvider for TcpProvider {
    fn descriptor(&self) -> TransportProviderDescriptor {
        TcpProvider::descriptor()
    }

    async fn bind(
        &self,
        endpoint: &ProviderEndpoint,
        security: Option<&ServerTransportSecurity>,
        limits: RuntimeFrameLimits,
    ) -> Result<BoundServerProvider, RuntimeError> {
        let addr =
            endpoint
                .as_str()
                .strip_prefix("tcp://")
                .ok_or(RuntimeError::UnsupportedTransport(
                    "TCP provider endpoint must use tcp://",
                ))?;
        let listener: Box<dyn FramedListener> = match security {
            Some(security) => Box::new(
                TcpTlsFramedListener::bind(
                    addr,
                    security.certificate_der.clone(),
                    security.private_key_pkcs8_der.clone(),
                    limits,
                )
                .await?,
            ),
            None => Box::new(TcpFramedListener::bind_with_limits(addr, limits).await?),
        };
        let endpoint = format!("tcp://{}", listener.local_addr()?).parse()?;
        BoundServerProvider::new(endpoint, listener)
    }
}

pub fn register_tcp_provider(
    registry: &mut TransportProviderRegistry,
) -> Result<(), nnrp_transport_provider::TransportProviderRegistryError> {
    TcpProvider::register(registry)
}

fn runtime_io(error: impl std::error::Error + Send + Sync + 'static) -> RuntimeError {
    io::Error::other(error).into()
}

fn normalize_shutdown(result: io::Result<()>) -> Result<(), RuntimeError> {
    match result {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::BrokenPipe
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::NotConnected
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnrp_core::{
        CommonHeader, MessageType, TransportPolicy, TransportProbeAckMetadata,
        TransportProbeMetadata,
    };
    use nnrp_runtime::{
        ClientProviderRoute, ClientProviderRoutes, NnrpClient, NnrpClientConfig, NnrpClientOptions,
        NnrpServer, NnrpServerConfig, RuntimePacket,
    };
    use std::sync::Arc;

    #[test]
    fn tcp_provider_registers_available_descriptor() {
        let mut registry = TransportProviderRegistry::new();
        register_tcp_provider(&mut registry).expect("tcp provider should register");
        assert_eq!(registry.providers().len(), 1);
        assert_eq!(registry.providers()[0].name, TcpProvider::NAME);
        assert_eq!(registry.providers()[0].transport_id, TransportId::Tcp);
        assert!(registry.providers()[0].available);
    }

    #[test]
    fn tcp_provider_participates_in_policy_selection() {
        let registry = TransportProviderRegistry::new()
            .with_provider(TcpProvider::descriptor())
            .expect("tcp provider should register");
        let readiness = [nnrp_transport_provider::TransportCandidateReadiness::ready(
            TransportId::Tcp,
            TcpProvider::descriptor().metadata.id,
        )];
        let selection = registry
            .select(&nnrp_transport_provider::TransportSelectionOptions {
                peer_supported_transports: vec![TransportId::Tcp],
                policy: TransportPolicy::ForceTcp,
                requested_max_frame_bytes: None,
                candidate_readiness: readiness.to_vec(),
                probe_observations: Vec::new(),
            })
            .expect("tcp provider should satisfy force tcp");
        assert_eq!(selection.selected_provider.name, TcpProvider::NAME);
    }

    #[tokio::test]
    async fn tcp_tls_transport_round_trips_probe_packets() {
        let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
        let certificate_der = certified.cert.der().to_vec();
        let private_key_der = certified.signing_key.serialize_der();
        let listener = TcpTlsFramedListener::bind(
            "127.0.0.1:0",
            certificate_der.clone(),
            private_key_der,
            RuntimeFrameLimits::default(),
        )
        .await
        .unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut transport = listener.accept().await.unwrap();
            let packet = transport.read_packet().await.unwrap();
            let probe = TransportProbeMetadata::parse(&packet.metadata).unwrap();
            transport
                .write_packet(
                    &RuntimePacket::new(
                        CommonHeader::new(MessageType::TransportProbeAck, 0, 0),
                        TransportProbeAckMetadata {
                            probe_id: probe.probe_id,
                            server_recv_ts_us: 1,
                        }
                        .to_bytes()
                        .unwrap()
                        .to_vec(),
                        Vec::new(),
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
        });

        let security = ClientTransportSecurity::new("localhost", certificate_der);
        let mut client = TcpTlsTransport::connect(addr, &security, RuntimeFrameLimits::default())
            .await
            .unwrap();
        client
            .write_packet(
                &RuntimePacket::new(
                    CommonHeader::new(MessageType::TransportProbe, 0, 0),
                    TransportProbeMetadata {
                        probe_id: 7,
                        probe_payload_bytes: 0,
                        client_send_ts_us: 1,
                    }
                    .to_bytes()
                    .unwrap()
                    .to_vec(),
                    Vec::new(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let ack = client.read_packet().await.unwrap();
        assert_eq!(
            TransportProbeAckMetadata::parse(&ack.metadata)
                .unwrap()
                .probe_id,
            7
        );
        client.close().await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn tcp_provider_opens_high_level_secure_client_session() {
        let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
        let certificate_der = certified.cert.der().to_vec();
        let listener = TcpTlsFramedListener::bind(
            "127.0.0.1:0",
            certificate_der.clone(),
            certified.signing_key.serialize_der(),
            RuntimeFrameLimits::default(),
        )
        .await
        .unwrap();
        let addr = listener.local_addr().unwrap();
        let server = NnrpServer::from_listener(listener, NnrpServerConfig::default()).unwrap();
        let server_task = tokio::spawn(async move { server.accept().await.unwrap() });
        let client = NnrpClient::connect(
            NnrpClientOptions {
                endpoint: format!("nnrps://localhost:{}/session", addr.port())
                    .parse()
                    .unwrap(),
                provider_routes: ClientProviderRoutes::from([(
                    TransportId::Tcp,
                    ClientProviderRoute {
                        provider_endpoint: Some(format!("tcp://{addr}").parse().unwrap()),
                        security: Some(ClientTransportSecurity::new("localhost", certificate_der)),
                    },
                )]),
                transport_policy: TransportPolicy::Auto,
                session_defaults: NnrpClientConfig::default(),
            },
            [Arc::new(TcpProvider) as Arc<dyn NnrpClientProvider>],
        )
        .await
        .unwrap();
        assert_eq!(
            client
                .transport_selection()
                .unwrap()
                .selected_provider
                .transport_id,
            TransportId::Tcp
        );
        let client_session = client.open_session().await.unwrap();
        let server_session = server_task.await.unwrap();
        assert_eq!(client_session.session_id(), server_session.session_id());
    }
}
