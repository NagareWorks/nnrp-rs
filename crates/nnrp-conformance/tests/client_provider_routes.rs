use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use nnrp_core::{TransportId, TransportPolicy};
use nnrp_runtime::{
    ClientProviderRoute, ClientProviderRoutes, ClientTransportSecurity, NnrpClient,
    NnrpClientConfig, NnrpClientOptions, NnrpClientProvider, NnrpServer, NnrpServerConfig,
    NnrpServerOptions, NnrpServerProvider, NnrpServerSession, RuntimeError, ServerProviderRoute,
    ServerProviderRoutes, ServerTransportSecurity,
};
use nnrp_transport_ipc::{IpcEndpoint, IpcFramedListener, IpcProvider};
use nnrp_transport_provider::{TransportRejectionReason, TransportSelectionError};
use nnrp_transport_quic::{QuicProvider, QuicServerEndpointConfig};
use nnrp_transport_tcp::TcpProvider;
use nnrp_transport_websocket::WebSocketProvider;
use tokio::task::JoinHandle;

struct SecureProviderCase {
    transport_id: TransportId,
    locator: &'static str,
    policy: TransportPolicy,
    server_provider: Arc<dyn NnrpServerProvider>,
    client_provider: Arc<dyn NnrpClientProvider>,
}

#[tokio::test]
async fn official_client_providers_probe_and_adopt_one_real_carrier() {
    let tcp_server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default())
        .await
        .unwrap();
    let tcp_addr = tcp_server.local_addr().unwrap();
    let ipc_endpoint = unique_ipc_endpoint();
    let ipc_listener = IpcFramedListener::bind(&ipc_endpoint).await.unwrap();
    let ipc_server = NnrpServer::from_listener(ipc_listener, NnrpServerConfig::default()).unwrap();
    let tcp_task = spawn_accept_after_probe(tcp_server);
    let ipc_task = spawn_accept_after_probe(ipc_server);

    let client = NnrpClient::connect(
        NnrpClientOptions {
            endpoint: format!("nnrp://{tcp_addr}/session/default")
                .parse()
                .unwrap(),
            provider_routes: ClientProviderRoutes::from([(
                TransportId::Ipc,
                ClientProviderRoute::at(ipc_endpoint.to_string().parse().unwrap()),
            )]),
            transport_policy: TransportPolicy::Auto,
            session_defaults: NnrpClientConfig::default(),
        },
        [
            Arc::new(TcpProvider) as Arc<dyn NnrpClientProvider>,
            Arc::new(IpcProvider) as Arc<dyn NnrpClientProvider>,
        ],
    )
    .await
    .unwrap();
    let selection = client.transport_selection().unwrap().clone();
    assert_eq!(selection.candidates.len(), 2);
    assert!(selection
        .candidates
        .iter()
        .all(|candidate| candidate.probe.is_some()));

    let client_session = client.open_session().await.unwrap();
    assert_ne!(client_session.session_id(), 0);
    let selected_server_session = match selection.selected_provider.transport_id {
        TransportId::Tcp => {
            ipc_task.abort();
            tcp_task.await.unwrap()
        }
        TransportId::Ipc => {
            tcp_task.abort();
            ipc_task.await.unwrap()
        }
        other => panic!("unexpected selected carrier: {other:?}"),
    };
    assert_eq!(
        selected_server_session.session_id(),
        client_session.session_id()
    );
}

#[tokio::test]
async fn logical_server_accepts_real_tcp_and_ipc_provider_sessions() {
    let ipc_endpoint = unique_ipc_endpoint();
    let server = NnrpServer::listen(
        NnrpServerOptions {
            endpoint: "nnrp://127.0.0.1:4500/session/default".parse().unwrap(),
            provider_routes: ServerProviderRoutes::from([
                (
                    TransportId::Tcp,
                    ServerProviderRoute::at("tcp://127.0.0.1:0".parse().unwrap()),
                ),
                (
                    TransportId::Ipc,
                    ServerProviderRoute::at(ipc_endpoint.to_string().parse().unwrap()),
                ),
            ]),
            transport_policy: TransportPolicy::Auto,
            session_defaults: NnrpServerConfig::default(),
        },
        [
            Arc::new(TcpProvider) as Arc<dyn NnrpServerProvider>,
            Arc::new(IpcProvider) as Arc<dyn NnrpServerProvider>,
        ],
    )
    .await
    .unwrap();
    let endpoints = server.bound_provider_endpoints();
    assert_eq!(endpoints.len(), 2);
    let tcp_endpoint = endpoints.get(&TransportId::Tcp).unwrap().clone();
    assert!(!tcp_endpoint.as_str().ends_with(":0"));
    assert_eq!(
        endpoints.get(&TransportId::Ipc).unwrap().as_str(),
        ipc_endpoint.to_string()
    );

    let (accepted_ipc, client_ipc) = tokio::join!(
        server.accept(),
        open_forced_session(
            1,
            TransportId::Ipc,
            ipc_endpoint.to_string().parse().unwrap(),
            TransportPolicy::ForceIpc,
            Arc::new(IpcProvider) as Arc<dyn NnrpClientProvider>,
        )
    );
    let accepted_ipc = accepted_ipc.unwrap();
    let client_ipc = client_ipc.unwrap();
    assert_eq!(accepted_ipc.active_transport_id(), TransportId::Ipc);
    assert_eq!(accepted_ipc.session_id(), client_ipc.session_id());

    let (accepted_tcp, client_tcp) = tokio::join!(
        server.accept(),
        open_forced_session(
            2,
            TransportId::Tcp,
            tcp_endpoint,
            TransportPolicy::ForceTcp,
            Arc::new(TcpProvider) as Arc<dyn NnrpClientProvider>,
        )
    );
    let accepted_tcp = accepted_tcp.unwrap();
    let client_tcp = client_tcp.unwrap();
    assert_eq!(accepted_tcp.active_transport_id(), TransportId::Tcp);
    assert_eq!(accepted_tcp.session_id(), client_tcp.session_id());
}

#[tokio::test]
async fn logical_server_uses_route_local_security_for_tcp_quic_and_websocket() {
    let (_, certificate) =
        QuicServerEndpointConfig::self_signed_localhost("127.0.0.1:0".parse().unwrap()).unwrap();
    let cases = [
        SecureProviderCase {
            transport_id: TransportId::Tcp,
            locator: "tcp://127.0.0.1:0",
            policy: TransportPolicy::ForceTcp,
            server_provider: Arc::new(TcpProvider),
            client_provider: Arc::new(TcpProvider),
        },
        SecureProviderCase {
            transport_id: TransportId::Quic,
            locator: "quic://127.0.0.1:0",
            policy: TransportPolicy::ForceQuic,
            server_provider: Arc::new(QuicProvider),
            client_provider: Arc::new(QuicProvider),
        },
        SecureProviderCase {
            transport_id: TransportId::WebSocket,
            locator: "wss://localhost:0/nnrp",
            policy: TransportPolicy::ForceWebSocket,
            server_provider: Arc::new(WebSocketProvider),
            client_provider: Arc::new(WebSocketProvider),
        },
    ];

    for (session_index, case) in cases.into_iter().enumerate() {
        let server = NnrpServer::listen(
            NnrpServerOptions {
                endpoint: "nnrps://localhost:443/session/default".parse().unwrap(),
                provider_routes: ServerProviderRoutes::from([(
                    case.transport_id,
                    ServerProviderRoute {
                        provider_endpoint: Some(case.locator.parse().unwrap()),
                        security: Some(ServerTransportSecurity::new(
                            certificate.certificate_der.clone(),
                            certificate.private_key_pkcs8_der.clone(),
                        )),
                    },
                )]),
                transport_policy: case.policy,
                session_defaults: NnrpServerConfig::default(),
            },
            [case.server_provider],
        )
        .await
        .unwrap();
        let provider_endpoint = server
            .bound_provider_endpoints()
            .get(&case.transport_id)
            .unwrap()
            .clone();
        assert!(!provider_endpoint.as_str().contains(":0"));
        let server_task = spawn_accept_after_probe(server);

        let session = NnrpClientConfig {
            requested_session_id: session_index as u32 + 10,
            ..NnrpClientConfig::default()
        };
        let opened = NnrpClient::connect(
            NnrpClientOptions {
                endpoint: "nnrps://localhost:443/session/default".parse().unwrap(),
                provider_routes: ClientProviderRoutes::from([(
                    case.transport_id,
                    ClientProviderRoute {
                        provider_endpoint: Some(provider_endpoint),
                        security: Some(ClientTransportSecurity::new(
                            "localhost",
                            certificate.certificate_der.clone(),
                        )),
                    },
                )]),
                transport_policy: case.policy,
                session_defaults: session,
            },
            [case.client_provider],
        )
        .await
        .unwrap()
        .open_session()
        .await
        .unwrap();
        let accepted = server_task.await.unwrap();
        assert_eq!(accepted.active_transport_id(), case.transport_id);
        assert_eq!(accepted.session_id(), opened.session_id());
    }
}

#[test]
fn route_sets_keep_one_role_specific_route_per_transport() {
    let mut client_routes = ClientProviderRoutes::new();
    assert!(client_routes
        .insert(
            TransportId::Tcp,
            ClientProviderRoute::at("tcp://127.0.0.1:4400".parse().unwrap()),
        )
        .is_none());
    assert!(client_routes
        .insert(
            TransportId::Tcp,
            ClientProviderRoute::at("tcp://127.0.0.1:4401".parse().unwrap()),
        )
        .is_some());
    assert_eq!(client_routes.len(), 1);
    assert_ne!(
        std::any::TypeId::of::<ClientTransportSecurity>(),
        std::any::TypeId::of::<ServerTransportSecurity>()
    );
}

#[tokio::test]
async fn client_route_validation_preserves_frozen_rejection_precedence() {
    let error = NnrpClient::connect(
        NnrpClientOptions {
            endpoint: "nnrps://localhost:443/session/default".parse().unwrap(),
            provider_routes: ClientProviderRoutes::from([(
                TransportId::Tcp,
                ClientProviderRoute {
                    provider_endpoint: Some("ws://localhost/nnrp".parse().unwrap()),
                    security: Some(ClientTransportSecurity::new("", Vec::<u8>::new())),
                },
            )]),
            transport_policy: TransportPolicy::Auto,
            session_defaults: NnrpClientConfig::default(),
        },
        [Arc::new(TcpProvider) as Arc<dyn NnrpClientProvider>],
    )
    .await
    .unwrap_err();

    let RuntimeError::TransportSelection(TransportSelectionError::NoViableTransport {
        candidates,
        ..
    }) = error
    else {
        panic!("unexpected route validation error: {error}");
    };
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].rejection_reason,
        Some(TransportRejectionReason::RouteUnresolved)
    );
}

#[tokio::test]
async fn server_route_validation_preserves_frozen_rejection_precedence() {
    let error = NnrpServer::listen(
        NnrpServerOptions {
            endpoint: "nnrps://localhost:443/session/default".parse().unwrap(),
            provider_routes: ServerProviderRoutes::from([(
                TransportId::Tcp,
                ServerProviderRoute {
                    provider_endpoint: Some("ws://localhost/nnrp".parse().unwrap()),
                    security: Some(ServerTransportSecurity::new(
                        Vec::<u8>::new(),
                        Vec::<u8>::new(),
                    )),
                },
            )]),
            transport_policy: TransportPolicy::Auto,
            session_defaults: NnrpServerConfig::default(),
        },
        [Arc::new(TcpProvider) as Arc<dyn NnrpServerProvider>],
    )
    .await
    .unwrap_err();

    assert!(matches!(
        error,
        RuntimeError::ServerRouteRejected {
            transport_id: TransportId::Tcp,
            reason: TransportRejectionReason::RouteUnresolved,
            ..
        }
    ));
}

async fn open_forced_session(
    session_id: u32,
    transport_id: TransportId,
    provider_endpoint: nnrp_runtime::ProviderEndpoint,
    transport_policy: TransportPolicy,
    provider: Arc<dyn NnrpClientProvider>,
) -> Result<nnrp_runtime::NnrpClientSession, RuntimeError> {
    let session = NnrpClientConfig {
        requested_session_id: session_id,
        ..NnrpClientConfig::default()
    };
    NnrpClient::connect(
        NnrpClientOptions {
            endpoint: "nnrp://127.0.0.1:4500/session/default".parse().unwrap(),
            provider_routes: ClientProviderRoutes::from([(
                transport_id,
                ClientProviderRoute::at(provider_endpoint),
            )]),
            transport_policy,
            session_defaults: session,
        },
        [provider],
    )
    .await?
    .open_session()
    .await
}

fn spawn_accept_after_probe(server: NnrpServer) -> JoinHandle<NnrpServerSession> {
    tokio::spawn(async move {
        loop {
            match server.accept().await {
                Ok(session) => return session,
                Err(RuntimeError::Io(_)) | Err(RuntimeError::TransportClosed { .. }) => continue,
                Err(error) => panic!("provider listener failed during route probe: {error}"),
            }
        }
    })
}

fn unique_ipc_endpoint() -> IpcEndpoint {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    #[cfg(unix)]
    {
        IpcEndpoint::unix(std::env::temp_dir().join(format!("nnrp-routes-{nonce}.sock")))
    }
    #[cfg(windows)]
    {
        IpcEndpoint::named_pipe(format!("nnrp-routes-{nonce}"))
    }
}
