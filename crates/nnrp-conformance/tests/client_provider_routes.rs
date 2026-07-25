use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use nnrp_core::{TransportId, TransportPolicy};
use nnrp_runtime::{
    ClientProviderRoute, ClientProviderRoutes, NnrpClient, NnrpClientConfig, NnrpClientOptions,
    NnrpClientProvider, NnrpServer, NnrpServerConfig, NnrpServerSession, RuntimeError,
    RuntimeTransportKind,
};
use nnrp_transport_ipc::{IpcEndpoint, IpcFramedListener, IpcProvider};
use nnrp_transport_tcp::TcpProvider;
use tokio::task::JoinHandle;

#[tokio::test]
async fn official_client_providers_probe_and_adopt_one_real_carrier() {
    let tcp_server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default())
        .await
        .unwrap();
    let tcp_addr = tcp_server.local_addr().unwrap();
    let ipc_endpoint = unique_ipc_endpoint();
    let ipc_listener = IpcFramedListener::bind(&ipc_endpoint).await.unwrap();
    let ipc_server = NnrpServer::from_listener(
        ipc_listener,
        NnrpServerConfig::default().with_transport(RuntimeTransportKind::Ipc),
    )
    .unwrap();
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
            session: NnrpClientConfig::default(),
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
    let selected_server_session = match selection.selected.transport_id {
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
