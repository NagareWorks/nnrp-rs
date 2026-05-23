use nnrp_core::{
    BackpressureLevel, CacheObjectId, CacheObjectKind, CommonHeader, FlowScopeKind,
    FlowUpdateMetadata, FlowUpdateReason, FrameSubmitMetadata, InFlightPolicy, InputProfile,
    MessageType, OperationState, PayloadKindBitmap, ResultClass, ResultPushMetadata,
    SchemaRegistry, SessionCloseMetadata, SessionCloseReason, SessionOpenAckMetadata,
    SessionOpenMetadata, SessionPatchAckMetadata, SessionPatchAckStatus, SessionPatchMetadata,
    SessionPatchRejectReason, SessionPriorityClass, SessionStatus, SubmitMode, TileIndexMode,
    FLOW_UPDATE_FLAG_CREDIT_VALID, RESULT_PUSH_METADATA_LEN, SESSION_CLOSE_ACK_METADATA_LEN,
    SESSION_ERROR_NONE, SESSION_OPEN_ACK_METADATA_LEN, SESSION_OPEN_METADATA_LEN,
    STANDARD_PROFILE_TOKEN, TOKEN_DELTA_SCHEMA_ID, TOKEN_DELTA_SCHEMA_VERSION,
};
use nnrp_runtime::{
    FramedTransport, NnrpClient, NnrpClientConfig, NnrpClientEvent, NnrpServer, NnrpServerConfig,
    RuntimeError, RuntimePacket, RuntimeTransportKind, TcpTransport,
};
use tokio::net::TcpListener;

#[tokio::test]
async fn tcp_loopback_opens_matching_client_and_server_sessions() -> Result<(), RuntimeError> {
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;

    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        let session_id = session.session_id();
        let profile_id = session.client_open().profile_id;
        let close = session.receive_close().await?;
        session.ack_close(&close).await?;
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
async fn tcp_loopback_submits_frame_receives_result_and_closes() -> Result<(), RuntimeError> {
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;

    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        let submit = session.receive_submit().await?;
        assert_eq!(submit.frame_id, 1);
        assert_eq!(
            submit.metadata.payload_kind_bitmap.0,
            PayloadKindBitmap::TOKEN_CHUNK
        );
        assert_eq!(submit.body, b"prompt".to_vec());

        session
            .send_result(submit.frame_id, token_result(), b"delta".to_vec())
            .await?;
        let close = session.receive_close().await?;
        assert_eq!(close.last_operation_id, 1);
        session.ack_close(&close).await?;
        session.close().await
    });

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
    let mut session = client.open_session().await?;
    let frame_id = session.submit(token_submit(), b"prompt".to_vec()).await?;
    assert_eq!(frame_id, 1);

    let result = session.await_result().await?;
    assert_eq!(result.frame_id, frame_id);
    assert_eq!(result.metadata.status_code, 200);
    assert_eq!(result.body, b"delta".to_vec());
    session.close().await?;
    server_task.await.expect("server task should join")?;
    Ok(())
}

#[tokio::test]
async fn tcp_loopback_handles_cancel_drop_flow_and_patch() -> Result<(), RuntimeError> {
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;

    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        let submit = session.receive_submit().await?;
        assert_eq!(submit.frame_id, 1);
        assert_eq!(session.operations().operation_count(), 1);

        let cancel = session.receive_cancel().await?;
        assert_eq!(cancel.frame_id, submit.frame_id);
        assert_eq!(
            session
                .operations()
                .operation(submit.frame_id as u64)
                .expect("operation should be registered")
                .state,
            OperationState::Cancelled
        );

        let patch = session.receive_patch().await?;
        assert_eq!(patch.patch_mask, 1);
        session.send_patch_ack(patch_ack()).await?;

        session.send_flow_update(session_flow_update()).await?;
        session.send_result_drop(submit.frame_id).await?;

        let close = session.receive_close().await?;
        session.ack_close(&close).await?;
        session.close().await
    });

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
    let mut session = client.open_session().await?;
    let frame_id = session
        .submit_nowait(token_submit(), b"prompt".to_vec())
        .await?;
    session.cancel_frame(frame_id).await?;

    let patch_ack = session.patch_session(session_patch()).await?;
    assert_eq!(patch_ack.ack_status, SessionPatchAckStatus::Accepted);

    match session.await_event().await? {
        NnrpClientEvent::FlowUpdate(flow) => {
            assert_eq!(flow.scope_kind, FlowScopeKind::Session);
            assert_eq!(flow.session_credit, 7);
        }
        event => panic!("expected flow update, got {event:?}"),
    }
    match session.await_event().await? {
        NnrpClientEvent::ResultDrop { frame_id: dropped } => assert_eq!(dropped, frame_id),
        event => panic!("expected result drop, got {event:?}"),
    }

    session.close().await?;
    server_task.await.expect("server task should join")?;
    Ok(())
}

#[tokio::test]
async fn runtime_configs_select_transport_cache_hints_and_server_state() -> Result<(), RuntimeError>
{
    let client_config = NnrpClientConfig::default()
        .with_transport(RuntimeTransportKind::Quic)
        .with_cache_hints(vec![CacheObjectKind::PromptSegment]);
    assert_eq!(client_config.transport, RuntimeTransportKind::Quic);
    assert_eq!(
        client_config.cache_hints,
        vec![CacheObjectKind::PromptSegment]
    );
    assert!(matches!(
        NnrpClient::connect_tcp("127.0.0.1:0", client_config).await,
        Err(RuntimeError::UnsupportedTransport(_))
    ));
    assert!(matches!(
        NnrpClient::connect_quic(
            "localhost:4433",
            NnrpClientConfig::default().with_transport(RuntimeTransportKind::Quic),
        )
        .await,
        Err(RuntimeError::UnsupportedTransport(_))
    ));

    let server_config = NnrpServerConfig::default()
        .with_transport(RuntimeTransportKind::Quic)
        .with_supported_profiles(vec![STANDARD_PROFILE_TOKEN])
        .with_supported_cache_objects(vec![CacheObjectKind::PromptSegment])
        .with_schema_registry(SchemaRegistry::with_standard_preview3_profiles())
        .with_cache_limits(1, 1024);
    assert_eq!(server_config.transport, RuntimeTransportKind::Quic);
    assert_eq!(
        server_config.supported_cache_objects,
        vec![CacheObjectKind::PromptSegment]
    );
    assert_eq!(server_config.max_cache_object_bytes, 1024);
    assert!(matches!(
        NnrpServer::bind_tcp("127.0.0.1:0", server_config).await,
        Err(RuntimeError::UnsupportedTransport(_))
    ));
    assert!(matches!(
        NnrpServer::bind_quic(
            "localhost:4433",
            NnrpServerConfig::default().with_transport(RuntimeTransportKind::Quic),
        )
        .await,
        Err(RuntimeError::UnsupportedTransport(_))
    ));

    let server = NnrpServer::bind_tcp(
        "127.0.0.1:0",
        NnrpServerConfig::default().with_cache_limits(1, 1024),
    )
    .await?;
    let addr = server.local_addr()?;
    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        session.track_cache_object(cache_object_id(1))?;
        assert_eq!(session.cache_object_count(), 1);
        session.track_cache_object(cache_object_id(1))?;
        assert_eq!(session.cache_object_count(), 1);
        assert!(matches!(
            session.track_cache_object(cache_object_id(2)),
            Err(RuntimeError::UnexpectedMessage(_))
        ));
        let close = session.receive_close().await?;
        session.ack_close(&close).await?;
        session.close().await
    });

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
    client.open_session().await?.close().await?;
    server_task.await.expect("server task should join")?;
    Ok(())
}

#[tokio::test]
async fn server_rejects_unsupported_profile_before_session_install() -> Result<(), RuntimeError> {
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;
    let server_task = tokio::spawn(async move {
        assert!(matches!(
            server.accept().await,
            Err(RuntimeError::UnexpectedMessage(_))
        ));
    });

    let config = NnrpClientConfig {
        profile_id: 0xffff,
        ..Default::default()
    };
    let client = NnrpClient::connect_tcp(addr, config).await?;
    assert!(matches!(
        client.open_session().await,
        Err(RuntimeError::UnexpectedMessage(_))
    ));
    server_task.await.expect("server task should join");
    Ok(())
}

#[tokio::test]
async fn client_result_reader_rejects_wrong_session_and_metadata_shape() -> Result<(), RuntimeError>
{
    let wrong_message = {
        let mut header = CommonHeader::new(
            MessageType::SessionCloseAck,
            SESSION_CLOSE_ACK_METADATA_LEN as u32,
            0,
        );
        header.session_id = 1;
        RuntimePacket::new(header, vec![0; SESSION_CLOSE_ACK_METADATA_LEN], Vec::new())?
    };
    let mut session = scripted_client_session(wrong_message).await?;
    assert!(matches!(
        session.await_result().await,
        Err(RuntimeError::UnexpectedMessage(_))
    ));
    session.close_transport().await?;

    let wrong_session = {
        let mut header =
            CommonHeader::new(MessageType::ResultPush, RESULT_PUSH_METADATA_LEN as u32, 0);
        header.session_id = 2;
        header.frame_id = 1;
        RuntimePacket::new(header, token_result().to_bytes()?.to_vec(), Vec::new())?
    };
    let mut session = scripted_client_session(wrong_session).await?;
    assert!(matches!(
        session.await_result().await,
        Err(RuntimeError::UnexpectedMessage(_))
    ));
    session.close_transport().await?;

    let malformed = {
        let mut header = CommonHeader::new(MessageType::ResultPush, 1, 0);
        header.session_id = 1;
        header.frame_id = 1;
        RuntimePacket::new(header, vec![0], Vec::new())?
    };
    let mut session = scripted_client_session(malformed).await?;
    assert!(matches!(
        session.await_result().await,
        Err(RuntimeError::UnexpectedMessage(_))
    ));
    session.close_transport().await
}

#[tokio::test]
async fn client_close_rejects_wrong_ack_session_and_shape() -> Result<(), RuntimeError> {
    let wrong_session = {
        let mut header = CommonHeader::new(
            MessageType::SessionCloseAck,
            SESSION_CLOSE_ACK_METADATA_LEN as u32,
            0,
        );
        header.session_id = 2;
        RuntimePacket::new(header, vec![0; SESSION_CLOSE_ACK_METADATA_LEN], Vec::new())?
    };
    let mut session = scripted_client_session(wrong_session).await?;
    let result = session.close_with(close_request()).await;
    assert!(result.is_err(), "close should reject wrong ack session");
    session.close_transport().await?;

    let malformed = {
        let mut header = CommonHeader::new(MessageType::SessionCloseAck, 1, 0);
        header.session_id = 1;
        RuntimePacket::new(header, vec![0], Vec::new())?
    };
    let mut session = scripted_client_session(malformed).await?;
    let result = session.close_with(close_request()).await;
    assert!(
        result.is_err(),
        "close should reject malformed ack metadata"
    );
    session.close_transport().await
}

#[tokio::test]
async fn client_result_and_patch_helpers_reject_control_mismatches() -> Result<(), RuntimeError> {
    let drop_packet = {
        let mut header = CommonHeader::new(MessageType::ResultDrop, 0, 0);
        header.session_id = 1;
        header.frame_id = 1;
        RuntimePacket::new(header, Vec::new(), Vec::new())?
    };
    let mut session = scripted_client_session(drop_packet).await?;
    assert!(matches!(
        session.await_result().await,
        Err(RuntimeError::UnexpectedMessage(_))
    ));
    session.close_transport().await?;

    let flow_packet = {
        let mut header = CommonHeader::new(
            MessageType::FlowUpdate,
            nnrp_core::FLOW_UPDATE_METADATA_LEN as u32,
            0,
        );
        header.session_id = 1;
        RuntimePacket::new(
            header,
            session_flow_update().to_bytes()?.to_vec(),
            Vec::new(),
        )?
    };
    let mut session = scripted_client_session(flow_packet).await?;
    assert!(matches!(
        session.await_result().await,
        Err(RuntimeError::UnexpectedMessage(_))
    ));
    session.close_transport().await?;

    let wrong_patch_ack = {
        let mut header = CommonHeader::new(MessageType::ResultDrop, 0, 0);
        header.session_id = 1;
        RuntimePacket::new(header, Vec::new(), Vec::new())?
    };
    assert!(matches!(
        client_patch_error(wrong_patch_ack).await,
        RuntimeError::UnexpectedMessage(_)
    ));

    let malformed_patch_ack = {
        let mut header = CommonHeader::new(MessageType::SessionPatchAck, 1, 0);
        header.session_id = 1;
        RuntimePacket::new(header, vec![0], Vec::new())?
    };
    assert!(matches!(
        client_patch_error(malformed_patch_ack).await,
        RuntimeError::UnexpectedMessage(_)
    ));
    Ok(())
}

#[tokio::test]
async fn server_submit_reader_rejects_wrong_message_and_session() -> Result<(), RuntimeError> {
    let wrong_message = {
        let mut header =
            CommonHeader::new(MessageType::ResultPush, RESULT_PUSH_METADATA_LEN as u32, 0);
        header.session_id = 1;
        RuntimePacket::new(header, token_result().to_bytes()?.to_vec(), Vec::new())?
    };
    let err = server_receive_submit_error(wrong_message).await;
    assert!(matches!(err, RuntimeError::UnexpectedMessage(_)));

    let wrong_session = {
        let mut header = CommonHeader::new(
            MessageType::FrameSubmit,
            SESSION_OPEN_METADATA_LEN as u32,
            0,
        );
        header.session_id = 2;
        RuntimePacket::new(header, vec![0; SESSION_OPEN_METADATA_LEN], Vec::new())?
    };
    let err = server_receive_submit_error(wrong_session).await;
    assert!(matches!(err, RuntimeError::UnexpectedMessage(_)));

    let malformed = {
        let mut header = CommonHeader::new(MessageType::FrameSubmit, 1, 0);
        header.session_id = 1;
        RuntimePacket::new(header, vec![0], Vec::new())?
    };
    let err = server_receive_submit_error(malformed).await;
    assert!(matches!(err, RuntimeError::UnexpectedMessage(_)));
    Ok(())
}

#[tokio::test]
async fn server_close_reader_rejects_wrong_message_and_session() -> Result<(), RuntimeError> {
    let wrong_message = {
        let mut header = CommonHeader::new(
            MessageType::FrameSubmit,
            SESSION_OPEN_METADATA_LEN as u32,
            0,
        );
        header.session_id = 1;
        RuntimePacket::new(header, vec![0; SESSION_OPEN_METADATA_LEN], Vec::new())?
    };
    let err = server_receive_close_error(wrong_message).await;
    assert!(matches!(err, RuntimeError::UnexpectedMessage(_)));

    let wrong_session = {
        let mut header = CommonHeader::new(
            MessageType::SessionClose,
            nnrp_core::SESSION_CLOSE_METADATA_LEN as u32,
            0,
        );
        header.session_id = 2;
        RuntimePacket::new(header, close_request().to_bytes()?.to_vec(), Vec::new())?
    };
    let err = server_receive_close_error(wrong_session).await;
    assert!(matches!(err, RuntimeError::UnexpectedMessage(_)));
    Ok(())
}

#[tokio::test]
async fn server_cancel_and_patch_readers_reject_malformed_packets() -> Result<(), RuntimeError> {
    let cancel_wrong_message = {
        let mut header = CommonHeader::new(MessageType::SessionPatch, 1, 0);
        header.session_id = 1;
        RuntimePacket::new(header, vec![0], Vec::new())?
    };
    assert!(matches!(
        server_receive_cancel_error(cancel_wrong_message).await,
        RuntimeError::UnexpectedMessage(_)
    ));

    let cancel_wrong_session = {
        let mut header = CommonHeader::new(MessageType::FrameCancel, 0, 0);
        header.session_id = 2;
        RuntimePacket::new(header, Vec::new(), Vec::new())?
    };
    assert!(matches!(
        server_receive_cancel_error(cancel_wrong_session).await,
        RuntimeError::UnexpectedMessage(_)
    ));

    let cancel_malformed = {
        let mut header = CommonHeader::new(MessageType::FrameCancel, 1, 0);
        header.session_id = 1;
        RuntimePacket::new(header, vec![0], Vec::new())?
    };
    assert!(matches!(
        server_receive_cancel_error(cancel_malformed).await,
        RuntimeError::UnexpectedMessage(_)
    ));

    let patch_wrong_message = {
        let mut header = CommonHeader::new(MessageType::FrameCancel, 0, 0);
        header.session_id = 1;
        RuntimePacket::new(header, Vec::new(), Vec::new())?
    };
    assert!(matches!(
        server_receive_patch_error(patch_wrong_message).await,
        RuntimeError::UnexpectedMessage(_)
    ));

    let patch_wrong_session = {
        let mut header = CommonHeader::new(
            MessageType::SessionPatch,
            nnrp_core::SESSION_PATCH_METADATA_LEN as u32,
            0,
        );
        header.session_id = 2;
        RuntimePacket::new(header, session_patch().to_bytes()?.to_vec(), Vec::new())?
    };
    assert!(matches!(
        server_receive_patch_error(patch_wrong_session).await,
        RuntimeError::UnexpectedMessage(_)
    ));

    let patch_malformed = {
        let mut header = CommonHeader::new(MessageType::SessionPatch, 1, 0);
        header.session_id = 1;
        RuntimePacket::new(header, vec![0], Vec::new())?
    };
    assert!(matches!(
        server_receive_patch_error(patch_malformed).await,
        RuntimeError::UnexpectedMessage(_)
    ));
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

async fn scripted_client_session(
    packet: RuntimePacket,
) -> Result<nnrp_runtime::NnrpClientSession, RuntimeError> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let server_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut transport = TcpTransport::new(stream);
        let open_packet = transport.read_packet().await?;
        let open = SessionOpenMetadata::parse(&open_packet.metadata)?;
        let ack = open_ack(&open);
        let mut header = CommonHeader::new(
            MessageType::SessionOpenAck,
            SESSION_OPEN_ACK_METADATA_LEN as u32,
            0,
        );
        header.session_id = ack.session_id;
        transport
            .write_packet(&RuntimePacket::new(
                header,
                ack.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await?;
        transport.write_packet(&packet).await
    });

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
    let session = client.open_session().await?;
    server_task.await.expect("scripted server should join")?;
    Ok(session)
}

async fn client_patch_error(packet: RuntimePacket) -> RuntimeError {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should expose address");
    let server_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut transport = TcpTransport::new(stream);
        let open_packet = transport.read_packet().await?;
        let open = SessionOpenMetadata::parse(&open_packet.metadata)?;
        let ack = open_ack(&open);
        let mut header = CommonHeader::new(
            MessageType::SessionOpenAck,
            SESSION_OPEN_ACK_METADATA_LEN as u32,
            0,
        );
        header.session_id = ack.session_id;
        transport
            .write_packet(&RuntimePacket::new(
                header,
                ack.to_bytes()?.to_vec(),
                Vec::new(),
            )?)
            .await?;
        let _patch = transport.read_packet().await?;
        transport.write_packet(&packet).await
    });

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default())
        .await
        .expect("client should connect");
    let mut session = client.open_session().await.expect("session should open");
    let err = session
        .patch_session(session_patch())
        .await
        .expect_err("patch should reject scripted response");
    session
        .close_transport()
        .await
        .expect("transport should close");
    server_task.await.expect("scripted server should join").ok();
    err
}

async fn server_receive_submit_error(packet: RuntimePacket) -> RuntimeError {
    server_receive_error(packet, |mut session| async move {
        session.receive_submit().await.map(|_| ())
    })
    .await
}

async fn server_receive_close_error(packet: RuntimePacket) -> RuntimeError {
    server_receive_error(packet, |mut session| async move {
        session.receive_close().await.map(|_| ())
    })
    .await
}

async fn server_receive_cancel_error(packet: RuntimePacket) -> RuntimeError {
    server_receive_error(packet, |mut session| async move {
        session.receive_cancel().await.map(|_| ())
    })
    .await
}

async fn server_receive_patch_error(packet: RuntimePacket) -> RuntimeError {
    server_receive_error(packet, |mut session| async move {
        session.receive_patch().await.map(|_| ())
    })
    .await
}

async fn server_receive_error<F, Fut>(packet: RuntimePacket, receive: F) -> RuntimeError
where
    F: FnOnce(nnrp_runtime::NnrpServerSession) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), RuntimeError>> + Send + 'static,
{
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default())
        .await
        .expect("server should bind");
    let addr = server.local_addr().expect("server should expose address");
    let server_task = tokio::spawn(async move {
        let session = server.accept().await?;
        receive(session).await
    });

    let mut transport = TcpTransport::connect(addr)
        .await
        .expect("client should connect");
    let open = session_open();
    let mut open_header = CommonHeader::new(
        MessageType::SessionOpen,
        SESSION_OPEN_METADATA_LEN as u32,
        0,
    );
    open_header.session_id = 0;
    transport
        .write_packet(
            &RuntimePacket::new(open_header, open.to_bytes().unwrap().to_vec(), Vec::new())
                .expect("open packet should build"),
        )
        .await
        .expect("open should write");
    let _ack = transport.read_packet().await.expect("ack should read");
    transport
        .write_packet(&packet)
        .await
        .expect("packet should write");

    server_task
        .await
        .expect("server task should join")
        .expect_err("server should reject scripted packet")
}

fn close_request() -> SessionCloseMetadata {
    SessionCloseMetadata {
        close_reason: SessionCloseReason::ClientShutdown,
        in_flight_policy: InFlightPolicy::Drain,
        drain_timeout_ms: 0,
        last_operation_id: 1,
        session_error_code: SESSION_ERROR_NONE,
        session_close_tag: 1,
    }
}

fn cache_object_id(cache_key_lo: u32) -> CacheObjectId {
    CacheObjectId {
        cache_namespace: 7,
        cache_key_hi: 0,
        cache_key_lo,
        object_kind: CacheObjectKind::PromptSegment,
    }
}

fn session_patch() -> SessionPatchMetadata {
    SessionPatchMetadata {
        profile_id: STANDARD_PROFILE_TOKEN,
        patch_mask: 1,
        target_cadence_x100: 6_000,
        quality_tier: 2,
        degrade_policy: 0,
        active_lane_mask: 1,
        preferred_codec_bitmap: 0,
        preferred_compression_bitmap: 0,
        profile_patch_bytes: 0,
    }
}

fn patch_ack() -> SessionPatchAckMetadata {
    SessionPatchAckMetadata {
        ack_status: SessionPatchAckStatus::Accepted,
        reject_reason: SessionPatchRejectReason::None,
        applied_patch_mask: 1,
        rejected_patch_mask: 0,
        retry_after_ms: 0,
        effective_profile_id: STANDARD_PROFILE_TOKEN,
        effective_target_cadence_x100: 6_000,
        effective_quality_tier: 2,
        effective_degrade_policy: 0,
        effective_lane_mask: 1,
        effective_codec_bitmap: 0,
        effective_compression_bitmap: 0,
        profile_patch_ack_bytes: 0,
    }
}

fn session_flow_update() -> FlowUpdateMetadata {
    FlowUpdateMetadata {
        scope_kind: FlowScopeKind::Session,
        update_reason: FlowUpdateReason::Grant,
        backpressure_level: BackpressureLevel::None,
        connection_credit: 0,
        session_credit: 7,
        operation_credit: 0,
        operation_id: 0,
        retry_after_ms: 0,
        credit_epoch: 1,
        flow_flags: FLOW_UPDATE_FLAG_CREDIT_VALID,
    }
}

fn session_open() -> SessionOpenMetadata {
    SessionOpenMetadata {
        requested_session_id: 1,
        profile_id: STANDARD_PROFILE_TOKEN,
        priority_class: SessionPriorityClass::Balanced,
        session_flags: 0,
        schema_id: TOKEN_DELTA_SCHEMA_ID,
        schema_version: TOKEN_DELTA_SCHEMA_VERSION,
        default_deadline_ms: 500,
        max_in_flight_operations: 4,
        lease_ttl_hint_ms: 30_000,
        resume_token_bytes: 0,
        auth_bytes: 0,
        session_extension_bytes: 0,
        client_session_tag: 1,
    }
}

fn open_ack(open: &SessionOpenMetadata) -> SessionOpenAckMetadata {
    SessionOpenAckMetadata {
        session_id: open.requested_session_id,
        accepted_profile_id: open.profile_id,
        accepted_priority_class: open.priority_class,
        session_status: SessionStatus::Opened,
        schema_id: open.schema_id,
        schema_version: open.schema_version,
        granted_operation_credit: 2,
        max_in_flight_operations: open.max_in_flight_operations,
        lease_ttl_ms: open.lease_ttl_hint_ms,
        resume_window_ms: 120_000,
        resume_token_bytes: 0,
        session_extension_bytes: 0,
        server_session_tag: open.client_session_tag,
        route_scope_id: 0,
        session_error_code: SESSION_ERROR_NONE,
        session_flags_ack: 0,
    }
}

fn token_result() -> ResultPushMetadata {
    ResultPushMetadata {
        status_code: 200,
        result_flags: 0,
        section_count: 0,
        tile_count: 0,
        active_profile_id: STANDARD_PROFILE_TOKEN,
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
