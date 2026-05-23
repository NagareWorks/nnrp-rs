use nnrp_runtime::{NnrpClient, NnrpClientConfig, NnrpServer, NnrpServerConfig, RuntimeError};

#[tokio::test]
async fn tcp_loopback_opens_matching_client_and_server_sessions() -> Result<(), RuntimeError> {
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;

    let server_task = tokio::spawn(async move {
        let session = server.accept().await?;
        let session_id = session.session_id();
        let profile_id = session.client_open().profile_id;
        session.close().await?;
        Ok::<_, RuntimeError>((session_id, profile_id))
    });

    let config = NnrpClientConfig {
        requested_session_id: 42,
        ..Default::default()
    };
    let client = NnrpClient::connect_tcp(addr, config.clone()).await?;
    let client_session = client.open_session().await?;
    assert_eq!(client_session.session_id(), 42);
    assert!(client_session.lifecycle().session(42).is_some());
    client_session.close().await?;

    let (server_session_id, server_profile_id) =
        server_task.await.expect("server task should join")?;
    assert_eq!(server_session_id, 42);
    assert_eq!(server_profile_id, config.profile_id);
    Ok(())
}

#[tokio::test]
async fn quic_hooks_are_reserved_but_not_runtime_backed() {
    assert!(matches!(
        NnrpClient::connect_quic("localhost:4433", NnrpClientConfig::default()).await,
        Err(RuntimeError::UnsupportedTransport(_))
    ));
    assert!(matches!(
        NnrpServer::bind_quic("localhost:4433", NnrpServerConfig::default()).await,
        Err(RuntimeError::UnsupportedTransport(_))
    ));
}
