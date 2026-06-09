use nnrp_core::NnrpError;
use nnrp_core::{
    BackpressureLevel, CacheInvalidateMetadata, CacheInvalidateScope, CacheMissMetadata,
    CacheMissReason, CacheObjectId, CacheObjectKind, CacheReferenceMetadata, CacheReuseScope,
    CommonHeader, FlowScopeKind, FlowUpdateMetadata, FlowUpdateReason, FrameSubmitMetadata,
    InFlightPolicy, InputProfile, MemoryLocationHint, MessageType, ObjectDeltaMetadata,
    ObjectDescriptorMetadata, ObjectReferenceMetadata, ObjectReleaseMetadata, ObjectReleaseReason,
    OperationState, OwnershipHint, PartialResultMetadata, PayloadKindBitmap, PressureMetadata,
    ResultClass, ResultDropReasonMetadata, ResultPushMetadata, RuntimeObjectKind, RuntimeRole,
    SchemaRegistry, SessionCloseMetadata, SessionCloseReason, SessionMigrateAckMetadata,
    SessionOpenAckMetadata, SessionOpenMetadata, SessionPatchAckMetadata, SessionPatchAckStatus,
    SessionPatchMetadata, SessionPatchRejectReason, SessionPriorityClass, SessionStatus,
    SubmitMode, TileIndexMode, TransportId, FLOW_UPDATE_FLAG_CREDIT_VALID,
    RESULT_PUSH_METADATA_LEN, SESSION_CLOSE_ACK_METADATA_LEN, SESSION_ERROR_NONE,
    SESSION_OPEN_ACK_METADATA_LEN, SESSION_OPEN_METADATA_LEN, STANDARD_PROFILE_TOKEN,
    TOKEN_DELTA_SCHEMA_ID, TOKEN_DELTA_SCHEMA_VERSION,
};
use nnrp_runtime::{
    BoxedFramedTransport, FramedListener, FramedTransport, NnrpClient, NnrpClientConfig,
    NnrpClientEvent, NnrpServer, NnrpServerConfig, NnrpServerPolicy, RuntimeError, RuntimePacket,
    RuntimeTransportKind, TcpFramedListener, TcpTransport,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
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
        assert_eq!(
            session
                .operations()
                .operation(submit.frame_id as u64)
                .expect("operation should be registered")
                .state,
            OperationState::Completed
        );
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
        assert!(matches!(
            session
                .send_result(submit.frame_id, token_result(), b"late".to_vec())
                .await,
            Err(RuntimeError::Protocol(_))
        ));

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
async fn tcp_loopback_routes_preview4_runtime_controls() -> Result<(), RuntimeError> {
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;

    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        let submit = session.receive_submit().await?;
        assert_eq!(submit.frame_id, 1);

        let priority = session.receive_scheduling_update().await?;
        assert_eq!(priority.message_type, MessageType::PriorityUpdate);
        assert_eq!(priority.metadata.operation_id, submit.frame_id as u64);
        let schedule = session
            .operations()
            .operation(submit.frame_id as u64)
            .expect("operation should be registered")
            .schedule;
        assert_eq!(schedule.priority_class, 1);
        assert_eq!(schedule.priority_delta, -2);

        let deadline = session.receive_scheduling_update().await?;
        assert_eq!(deadline.message_type, MessageType::Deadline);
        assert_eq!(deadline.metadata.deadline_unix_ms, 1_800_000_000_000);
        let schedule = session
            .operations()
            .operation(submit.frame_id as u64)
            .expect("operation should be registered")
            .schedule;
        assert_eq!(schedule.deadline_unix_ms, 1_800_000_000_000);

        let credit = session.receive_pressure_update().await?;
        assert_eq!(credit.message_type, MessageType::CreditUpdate);
        assert_eq!(credit.metadata.credit_window, 9);

        session.send_backpressure(soft_backpressure()).await?;
        session
            .send_partial_result(partial_result(submit.frame_id as u64), b"partial".to_vec())
            .await?;

        let control = session.receive_runtime_control().await?;
        assert_eq!(control.message_type, MessageType::Cancel);
        assert_eq!(control.metadata.operation_id, submit.frame_id as u64);
        assert_eq!(
            session
                .operations()
                .operation(submit.frame_id as u64)
                .expect("operation should be registered")
                .state,
            OperationState::Cancelled
        );
        session
            .send_result_drop_reason(drop_reason(submit.frame_id as u64))
            .await?;

        let abort_submit = session.receive_submit().await?;
        assert_eq!(abort_submit.frame_id, 2);

        let expire = session.receive_scheduling_update().await?;
        assert_eq!(expire.message_type, MessageType::ExpireAt);
        assert_eq!(expire.metadata.operation_id, abort_submit.frame_id as u64);
        assert_eq!(expire.metadata.deadline_unix_ms, 1_800_000_000_500);
        let schedule = session
            .operations()
            .operation(abort_submit.frame_id as u64)
            .expect("operation should be registered")
            .schedule;
        assert_eq!(schedule.expire_at_unix_ms, 1_800_000_000_500);

        let abort = session.receive_runtime_control().await?;
        assert_eq!(abort.message_type, MessageType::Abort);
        assert_eq!(abort.metadata.operation_id, abort_submit.frame_id as u64);
        assert_eq!(
            session
                .operations()
                .operation(abort_submit.frame_id as u64)
                .expect("operation should be registered")
                .state,
            OperationState::Cancelled
        );
        session.send_backpressure(soft_backpressure()).await?;

        let close = session.receive_close().await?;
        session.ack_close(&close).await?;
        session.close().await
    });

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
    let mut session = client.open_session().await?;
    let frame_id = session
        .submit_nowait(token_submit(), b"prompt".to_vec())
        .await?;
    session.update_priority(frame_id as u64, 1, -2).await?;
    session
        .update_deadline(frame_id as u64, 1_800_000_000_000)
        .await?;
    session.send_credit_update(credit_update()).await?;
    session.cancel_operation(frame_id as u64, 7).await?;

    match session.await_event().await? {
        NnrpClientEvent::Backpressure(pressure) => {
            assert_eq!(pressure.pressure_level, BackpressureLevel::Soft as u16);
            assert_eq!(pressure.retry_after_ms, 25);
        }
        event => panic!("expected backpressure, got {event:?}"),
    }
    match session.await_event().await? {
        NnrpClientEvent::PartialResult { metadata, body } => {
            assert_eq!(metadata.operation_id, frame_id as u64);
            assert_eq!(body, b"partial".to_vec());
        }
        event => panic!("expected partial result, got {event:?}"),
    }
    match session.await_event().await? {
        NnrpClientEvent::ResultDropReason(reason) => {
            assert_eq!(reason.operation_id, frame_id as u64);
            assert_eq!(reason.drop_reason_code, 7);
        }
        event => panic!("expected drop reason, got {event:?}"),
    }

    let abort_frame_id = session
        .submit_nowait(token_submit(), b"abort-prompt".to_vec())
        .await?;
    session
        .expire_at(abort_frame_id as u64, 1_800_000_000_500)
        .await?;
    session.abort_operation(abort_frame_id as u64, 9).await?;

    match session.await_event().await? {
        NnrpClientEvent::Backpressure(pressure) => {
            assert_eq!(pressure.pressure_level, BackpressureLevel::Soft as u16);
            assert_eq!(pressure.retry_after_ms, 25);
        }
        event => panic!("expected abort backpressure, got {event:?}"),
    }

    session.close().await?;
    server_task.await.expect("server task should join")?;
    Ok(())
}

#[tokio::test]
async fn tcp_loopback_routes_preview4_object_and_cache_events() -> Result<(), RuntimeError> {
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;

    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        let submit = session.receive_submit().await?;
        let operation_id = submit.frame_id as u64;

        session
            .send_object_declare(object_descriptor(), b"meta".to_vec())
            .await?;
        session
            .send_object_ref(object_reference(operation_id), Vec::new())
            .await?;
        session
            .send_object_delta(object_delta(), b"abcd".to_vec())
            .await?;
        session
            .send_cache_reference(cache_reference(), b"hint".to_vec())
            .await?;
        session.send_cache_invalidate(cache_invalidate()).await?;
        session
            .send_result(submit.frame_id, token_result(), b"done".to_vec())
            .await?;
        session
            .send_object_release(
                object_release(operation_id, ObjectReleaseReason::Completed, 0),
                Vec::new(),
            )
            .await?;

        let close = session.receive_close().await?;
        session.ack_close(&close).await?;
        session.close().await
    });

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
    let mut session = client.open_session().await?;
    let frame_id = session
        .submit_nowait(token_submit(), b"prompt".to_vec())
        .await?;

    match session.await_event().await? {
        NnrpClientEvent::ObjectDeclare { metadata, body } => {
            assert_eq!(metadata.object_id, 900);
            assert_eq!(metadata.object_kind, RuntimeObjectKind::ImageTile);
            assert_eq!(body, b"meta".to_vec());
        }
        event => panic!("expected object declaration, got {event:?}"),
    }
    match session.await_event().await? {
        NnrpClientEvent::ObjectRef { metadata, body } => {
            assert_eq!(metadata.operation_id, frame_id as u64);
            assert_eq!(metadata.length, 4);
            assert!(body.is_empty());
        }
        event => panic!("expected object reference, got {event:?}"),
    }
    match session.await_event().await? {
        NnrpClientEvent::ObjectDelta { metadata, body } => {
            assert_eq!(metadata.object_id, 900);
            assert_eq!(metadata.delta_bytes, 4);
            assert_eq!(body, b"abcd".to_vec());
        }
        event => panic!("expected object delta, got {event:?}"),
    }
    match session.await_event().await? {
        NnrpClientEvent::CacheReference { metadata, body } => {
            assert_eq!(metadata.cache_key_lo, 0x5678);
            assert_eq!(metadata.reuse_scope, CacheReuseScope::Session);
            assert_eq!(body, b"hint".to_vec());
        }
        event => panic!("expected cache reference, got {event:?}"),
    }
    match session.await_event().await? {
        NnrpClientEvent::CacheInvalidate(metadata) => {
            assert_eq!(metadata.invalidate_scope, CacheInvalidateScope::ObjectKey);
            assert_eq!(metadata.cache_namespace, 42);
            assert_eq!(metadata.cache_key_hi, 0x1234);
            assert_eq!(metadata.cache_key_lo, 0x5678);
            assert_eq!(metadata.reason_code, 77);
        }
        event => panic!("expected cache invalidate, got {event:?}"),
    }
    match session.await_event().await? {
        NnrpClientEvent::Result(result) => {
            assert_eq!(result.frame_id, frame_id);
            assert_eq!(result.body, b"done".to_vec());
        }
        event => panic!("expected result, got {event:?}"),
    }
    match session.await_event().await? {
        NnrpClientEvent::ObjectRelease { metadata, body } => {
            assert_eq!(metadata.object_id, 900);
            assert_eq!(metadata.release_reason, ObjectReleaseReason::Completed);
            assert!(body.is_empty());
        }
        event => panic!("expected object release, got {event:?}"),
    }

    session.close().await?;
    server_task.await.expect("server task should join")?;
    Ok(())
}

#[tokio::test]
async fn tcp_loopback_releases_objects_after_cancel_and_reports_cache_miss(
) -> Result<(), RuntimeError> {
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;

    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        let submit = session.receive_submit().await?;
        let operation_id = submit.frame_id as u64;
        let control = session.receive_runtime_control().await?;
        assert_eq!(control.message_type, MessageType::Cancel);
        session
            .send_object_release(
                object_release(operation_id, ObjectReleaseReason::Cancelled, 6),
                b"cancel".to_vec(),
            )
            .await?;
        session
            .send_cache_miss(cache_miss(), b"schema".to_vec())
            .await?;

        let close = session.receive_close().await?;
        session.ack_close(&close).await?;
        session.close().await
    });

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
    let mut session = client.open_session().await?;
    let frame_id = session
        .submit_nowait(token_submit(), b"prompt".to_vec())
        .await?;
    session.cancel_operation(frame_id as u64, 7).await?;

    match session.await_event().await? {
        NnrpClientEvent::ObjectRelease { metadata, body } => {
            assert_eq!(metadata.operation_id, frame_id as u64);
            assert_eq!(metadata.release_reason, ObjectReleaseReason::Cancelled);
            assert_eq!(body, b"cancel".to_vec());
        }
        event => panic!("expected cancelled object release, got {event:?}"),
    }
    match session.await_event().await? {
        NnrpClientEvent::CacheMiss { metadata, body } => {
            assert_eq!(metadata.miss_reason, CacheMissReason::SchemaMismatch);
            assert_eq!(metadata.profile_id, STANDARD_PROFILE_TOKEN);
            assert_eq!(body, b"schema".to_vec());
        }
        event => panic!("expected cache miss, got {event:?}"),
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
    let debug_text = format!("{server_config:?}");
    assert!(debug_text.contains("NnrpServerConfig"));
    assert!(debug_text.contains("application_policy"));
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
async fn server_policy_hook_accepts_and_rejects_session_open() -> Result<(), RuntimeError> {
    let server = NnrpServer::bind_tcp(
        "127.0.0.1:0",
        NnrpServerConfig::default().with_application_policy(RequireSessionTag(7)),
    )
    .await?;
    let addr = server.local_addr()?;
    let server_task = tokio::spawn(async move {
        assert!(matches!(
            server.accept().await,
            Err(RuntimeError::UnexpectedMessage(_))
        ));
    });

    let rejected_client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
    assert!(matches!(
        rejected_client.open_session().await,
        Err(RuntimeError::UnexpectedMessage(_))
    ));
    server_task.await.expect("reject server task should join");

    let server = NnrpServer::bind_tcp(
        "127.0.0.1:0",
        NnrpServerConfig::default().with_application_policy(RequireSessionTag(7)),
    )
    .await?;
    let addr = server.local_addr()?;
    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        assert_eq!(session.client_open().client_session_tag, 7);
        let close = session.receive_close().await?;
        session.ack_close(&close).await?;
        session.close().await
    });

    let accepted_client = NnrpClient::connect_tcp(
        addr,
        NnrpClientConfig {
            requested_session_id: 7,
            ..Default::default()
        },
    )
    .await?;
    accepted_client.open_session().await?.close().await?;
    server_task.await.expect("accept server task should join")?;
    Ok(())
}

#[tokio::test]
async fn server_registry_tracks_resume_enabled_sessions() -> Result<(), RuntimeError> {
    let server = NnrpServer::bind_tcp(
        "127.0.0.1:0",
        NnrpServerConfig::default().with_resume_token_bytes(24),
    )
    .await?;
    let addr = server.local_addr()?;

    let server_task = tokio::spawn(async move {
        let mut first = server.accept().await?;
        assert_eq!(server.session_count()?, 1);
        assert_eq!(first.session_id(), 77);

        let mut resumed = server.accept().await?;
        assert_eq!(server.session_count()?, 1);
        assert_eq!(resumed.session_id(), 77);
        assert_eq!(resumed.client_open().resume_token_bytes, 24);

        let close = resumed.receive_close().await?;
        resumed.ack_close(&close).await?;
        resumed.close().await?;
        assert_eq!(server.session_count()?, 0);

        let close = first.receive_close().await?;
        first.ack_close(&close).await?;
        first.close().await
    });

    let initial_config = NnrpClientConfig {
        requested_session_id: 77,
        ..Default::default()
    }
    .with_resume(0);
    let initial = NnrpClient::connect_tcp(addr, initial_config)
        .await?
        .open_session()
        .await?;

    let resume_config = NnrpClientConfig {
        requested_session_id: 77,
        ..Default::default()
    }
    .with_resume(24);
    let resumed = NnrpClient::connect_tcp(addr, resume_config)
        .await?
        .open_session()
        .await?;
    assert_eq!(resumed.session_id(), 77);

    resumed.close().await?;
    initial.close().await?;
    server_task.await.expect("server task should join")?;
    Ok(())
}

#[tokio::test]
async fn tcp_loopback_suppresses_expired_final_results() -> Result<(), RuntimeError> {
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;

    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        let submit = session.receive_submit().await?;
        let expire = session.receive_scheduling_update().await?;
        assert_eq!(expire.message_type, MessageType::ExpireAt);
        assert_eq!(expire.metadata.operation_id, submit.frame_id as u64);

        let error = session
            .send_result(submit.frame_id, token_result(), b"expired".to_vec())
            .await
            .expect_err("expired operation should reject final result delivery");
        assert!(matches!(
            error,
            RuntimeError::Protocol(NnrpError::InvalidOperationTransition {
                from: OperationState::Superseded,
                to: OperationState::Completed,
            })
        ));
        assert_eq!(
            session
                .operations()
                .operation(submit.frame_id as u64)
                .expect("operation should be registered")
                .state,
            OperationState::Superseded
        );

        let close = session.receive_close().await?;
        session.ack_close(&close).await?;
        session.close().await
    });

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
    let mut session = client.open_session().await?;
    let frame_id = session
        .submit_nowait(token_submit(), b"prompt".to_vec())
        .await?;
    session.expire_at(frame_id as u64, 1).await?;
    session.close().await?;
    server_task.await.expect("server task should join")?;
    Ok(())
}

#[tokio::test]
async fn tcp_loopback_consumes_transport_migration_recovery() -> Result<(), RuntimeError> {
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;

    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        let migrate = session.receive_migrate().await?;
        assert_eq!(migrate.metadata.old_transport_id, TransportId::Tcp);
        assert_eq!(migrate.metadata.new_transport_id, TransportId::Quic);
        assert_eq!(migrate.metadata.last_result_frame_id, 10);

        let ack = SessionMigrateAckMetadata {
            accept_code: 0,
            resume_from_frame_id: 11,
            grace_window_ms: 500,
            server_migrate_ts_us: 200,
        };
        session.send_migrate_ack(&migrate.metadata, ack).await?;

        let close = session.receive_close().await?;
        session.ack_close(&close).await?;
        session.close().await
    });

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
    let mut session = client.open_session().await?;
    let request = session.build_migration_request(TransportId::Quic, 10, 100);
    let ack = session.migrate_transport(request).await?;
    assert_eq!(ack.resume_from_frame_id, 11);
    assert!(!nnrp_core::should_replay_frame_after_migration(&ack, 10));
    assert!(nnrp_core::should_replay_frame_after_migration(&ack, 11));

    session.close().await?;
    server_task.await.expect("server task should join")?;
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
async fn client_result_helper_rejects_preview4_control_non_result_events(
) -> Result<(), RuntimeError> {
    for packet in [
        control_event_packet(
            MessageType::ResultDropReason,
            1,
            drop_reason(1).to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        control_event_packet(
            MessageType::PartialResult,
            1,
            partial_result(1).to_bytes()?.to_vec(),
            b"partial".to_vec(),
        )?,
        control_event_packet(
            MessageType::Backpressure,
            1,
            soft_backpressure().to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        control_event_packet(
            MessageType::CreditUpdate,
            1,
            credit_update().to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        object_event_packet(
            MessageType::CacheInvalidate,
            1,
            cache_invalidate().to_bytes()?.to_vec(),
            Vec::new(),
        )?,
    ] {
        let mut session = scripted_client_session(packet).await?;
        assert!(matches!(
            session.await_result().await,
            Err(RuntimeError::UnexpectedMessage(_))
        ));
        session.close_transport().await?;
    }
    Ok(())
}

#[tokio::test]
async fn client_preview4_control_event_reader_rejects_malformed_packets() -> Result<(), RuntimeError>
{
    for packet in [
        control_event_packet(
            MessageType::ResultDropReason,
            2,
            drop_reason(1).to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        control_event_packet(MessageType::ResultDropReason, 1, vec![0], Vec::new())?,
        control_event_packet(
            MessageType::PartialResult,
            2,
            partial_result(1).to_bytes()?.to_vec(),
            b"partial".to_vec(),
        )?,
        control_event_packet(MessageType::PartialResult, 1, vec![0], Vec::new())?,
        control_event_packet(
            MessageType::PartialResult,
            1,
            partial_result(1).to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        control_event_packet(
            MessageType::Backpressure,
            2,
            soft_backpressure().to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        control_event_packet(MessageType::Backpressure, 1, vec![0], Vec::new())?,
        control_event_packet(
            MessageType::CreditUpdate,
            2,
            credit_update().to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        control_event_packet(MessageType::CreditUpdate, 1, vec![0], Vec::new())?,
    ] {
        let mut session = scripted_client_session(packet).await?;
        assert!(matches!(
            session.await_event().await,
            Err(RuntimeError::UnexpectedMessage(_))
        ));
        session.close_transport().await?;
    }
    Ok(())
}

#[tokio::test]
async fn client_preview4_event_reader_rejects_malformed_object_cache_packets(
) -> Result<(), RuntimeError> {
    for packet in [
        object_event_packet(
            MessageType::ObjectDeclare,
            2,
            object_descriptor().to_bytes()?.to_vec(),
            b"meta".to_vec(),
        )?,
        object_event_packet(MessageType::ObjectDeclare, 1, vec![0], Vec::new())?,
        object_event_packet(
            MessageType::ObjectDeclare,
            1,
            object_descriptor().to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        object_event_packet(
            MessageType::ObjectRef,
            2,
            object_reference(1).to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        object_event_packet(MessageType::ObjectRef, 1, vec![0], Vec::new())?,
        object_event_packet(
            MessageType::ObjectRef,
            1,
            object_reference(1).to_bytes()?.to_vec(),
            b"x".to_vec(),
        )?,
        object_event_packet(
            MessageType::ObjectRelease,
            2,
            object_release(1, ObjectReleaseReason::Completed, 0)
                .to_bytes()?
                .to_vec(),
            Vec::new(),
        )?,
        object_event_packet(MessageType::ObjectRelease, 1, vec![0], Vec::new())?,
        object_event_packet(
            MessageType::ObjectRelease,
            1,
            object_release(1, ObjectReleaseReason::Completed, 1)
                .to_bytes()?
                .to_vec(),
            Vec::new(),
        )?,
        object_event_packet(
            MessageType::ObjectDelta,
            2,
            object_delta().to_bytes()?.to_vec(),
            b"abcd".to_vec(),
        )?,
        object_event_packet(MessageType::ObjectDelta, 1, vec![0], Vec::new())?,
        object_event_packet(
            MessageType::ObjectDelta,
            1,
            object_delta().to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        object_event_packet(
            MessageType::CacheReference,
            2,
            cache_reference().to_bytes()?.to_vec(),
            b"hint".to_vec(),
        )?,
        object_event_packet(MessageType::CacheReference, 1, vec![0], Vec::new())?,
        object_event_packet(
            MessageType::CacheReference,
            1,
            cache_reference().to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        object_event_packet(
            MessageType::CacheMiss,
            2,
            cache_miss().to_bytes()?.to_vec(),
            b"schema".to_vec(),
        )?,
        object_event_packet(MessageType::CacheMiss, 1, vec![0], Vec::new())?,
        object_event_packet(
            MessageType::CacheMiss,
            1,
            cache_miss().to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        object_event_packet(
            MessageType::CacheInvalidate,
            2,
            cache_invalidate().to_bytes()?.to_vec(),
            Vec::new(),
        )?,
        object_event_packet(MessageType::CacheInvalidate, 1, vec![0], Vec::new())?,
        object_event_packet(
            MessageType::CacheInvalidate,
            1,
            cache_invalidate().to_bytes()?.to_vec(),
            b"body".to_vec(),
        )?,
    ] {
        let mut session = scripted_client_session(packet).await?;
        assert!(matches!(
            session.await_event().await,
            Err(RuntimeError::UnexpectedMessage(_))
        ));
        session.close_transport().await?;
    }
    Ok(())
}

#[tokio::test]
async fn server_preview4_control_readers_and_senders_reject_mismatches() -> Result<(), RuntimeError>
{
    for err in [
        server_receive_runtime_control_error(control_event_packet(
            MessageType::FlowUpdate,
            1,
            session_flow_update().to_bytes()?.to_vec(),
            Vec::new(),
        )?)
        .await,
        server_receive_runtime_control_error(control_event_packet(
            MessageType::Cancel,
            2,
            nnrp_core::ControlRequestMetadata {
                operation_id: 1,
                control_sequence: 1,
                reason_code: 7,
                source_role: 1,
                flags: 0,
                diagnostic_bytes: 0,
            }
            .to_bytes()?
            .to_vec(),
            Vec::new(),
        )?)
        .await,
        server_receive_runtime_control_error(control_event_packet(
            MessageType::Cancel,
            1,
            vec![0],
            Vec::new(),
        )?)
        .await,
        server_receive_scheduling_update_error(control_event_packet(
            MessageType::Cancel,
            1,
            nnrp_core::ControlRequestMetadata {
                operation_id: 1,
                control_sequence: 1,
                reason_code: 7,
                source_role: 1,
                flags: 0,
                diagnostic_bytes: 0,
            }
            .to_bytes()?
            .to_vec(),
            Vec::new(),
        )?)
        .await,
        server_receive_scheduling_update_error(control_event_packet(
            MessageType::Deadline,
            1,
            vec![0],
            Vec::new(),
        )?)
        .await,
        server_receive_pressure_update_error(control_event_packet(
            MessageType::Deadline,
            1,
            nnrp_core::SchedulingMetadata {
                operation_id: 1,
                control_sequence: 1,
                priority_class: 0,
                priority_delta: 0,
                deadline_unix_ms: 100,
                flags: 0,
            }
            .to_bytes()?
            .to_vec(),
            Vec::new(),
        )?)
        .await,
        server_receive_pressure_update_error(control_event_packet(
            MessageType::Backpressure,
            2,
            soft_backpressure().to_bytes()?.to_vec(),
            Vec::new(),
        )?)
        .await,
        server_receive_pressure_update_error(control_event_packet(
            MessageType::Backpressure,
            1,
            vec![0],
            Vec::new(),
        )?)
        .await,
        server_send_control_error(|mut session| async move {
            session
                .send_partial_result(partial_result(1), Vec::new())
                .await
        })
        .await,
        server_send_control_error(|mut session| async move {
            session
                .send_result_drop_reason(nnrp_core::ResultDropReasonMetadata {
                    operation_id: 1,
                    result_sequence: 1,
                    drop_reason_code: 0,
                    source_role: 2,
                    flags: 0,
                    diagnostic_bytes: 0,
                })
                .await
        })
        .await,
        server_send_control_error(|mut session| async move {
            session
                .send_backpressure(nnrp_core::PressureMetadata {
                    scope_id: 1,
                    credit_window: 1,
                    pressure_level: 0,
                    pressure_reason: 0,
                    retry_after_ms: 0,
                    flags: 0,
                })
                .await
        })
        .await,
    ] {
        assert!(matches!(
            err,
            RuntimeError::UnexpectedMessage(_) | RuntimeError::Protocol(_)
        ));
    }
    Ok(())
}

#[tokio::test]
async fn server_preview4_object_cache_senders_reject_body_mismatches() -> Result<(), RuntimeError> {
    for err in [
        server_send_object_error(|mut session| async move {
            session
                .send_object_declare(object_descriptor(), Vec::new())
                .await
        })
        .await,
        server_send_object_error(|mut session| async move {
            session
                .send_object_ref(object_reference(1), b"x".to_vec())
                .await
        })
        .await,
        server_send_object_error(|mut session| async move {
            session
                .send_object_release(
                    object_release(1, ObjectReleaseReason::Completed, 1),
                    Vec::new(),
                )
                .await
        })
        .await,
        server_send_object_error(|mut session| async move {
            session.send_object_delta(object_delta(), Vec::new()).await
        })
        .await,
        server_send_object_error(|mut session| async move {
            session
                .send_cache_reference(cache_reference(), Vec::new())
                .await
        })
        .await,
        server_send_object_error(|mut session| async move {
            session.send_cache_miss(cache_miss(), Vec::new()).await
        })
        .await,
    ] {
        assert!(matches!(err, RuntimeError::UnexpectedMessage(_)));
    }
    Ok(())
}

#[tokio::test]
async fn client_migration_rejects_wrong_ack_session_shape_and_cursor() -> Result<(), RuntimeError> {
    let wrong_message = {
        let mut header = CommonHeader::new(MessageType::ResultDrop, 0, 0);
        header.session_id = 1;
        RuntimePacket::new(header, Vec::new(), Vec::new())?
    };
    assert!(matches!(
        client_migrate_error(wrong_message).await,
        RuntimeError::UnexpectedMessage(_)
    ));

    let wrong_session = {
        let mut header = CommonHeader::new(
            MessageType::SessionMigrateAck,
            nnrp_core::SESSION_MIGRATE_ACK_METADATA_LEN as u32,
            0,
        );
        header.session_id = 2;
        RuntimePacket::new(header, migrate_ack(11).to_bytes()?.to_vec(), Vec::new())?
    };
    assert!(matches!(
        client_migrate_error(wrong_session).await,
        RuntimeError::UnexpectedMessage(_)
    ));

    let malformed = {
        let mut header = CommonHeader::new(MessageType::SessionMigrateAck, 1, 0);
        header.session_id = 1;
        RuntimePacket::new(header, vec![0], Vec::new())?
    };
    assert!(matches!(
        client_migrate_error(malformed).await,
        RuntimeError::UnexpectedMessage(_)
    ));

    let stale_cursor = {
        let mut header = CommonHeader::new(
            MessageType::SessionMigrateAck,
            nnrp_core::SESSION_MIGRATE_ACK_METADATA_LEN as u32,
            0,
        );
        header.session_id = 1;
        RuntimePacket::new(header, migrate_ack(9).to_bytes()?.to_vec(), Vec::new())?
    };
    assert!(matches!(
        client_migrate_error(stale_cursor).await,
        RuntimeError::Protocol(_)
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
async fn server_migration_reader_rejects_wrong_session_and_shape() -> Result<(), RuntimeError> {
    let wrong_message = {
        let mut header = CommonHeader::new(MessageType::FrameCancel, 0, 0);
        header.session_id = 1;
        RuntimePacket::new(header, Vec::new(), Vec::new())?
    };
    assert!(matches!(
        server_receive_migrate_error(wrong_message).await,
        RuntimeError::UnexpectedMessage(_)
    ));

    let wrong_session = {
        let mut header = CommonHeader::new(
            MessageType::SessionMigrate,
            nnrp_core::SESSION_MIGRATE_METADATA_LEN as u32,
            0,
        );
        header.session_id = 2;
        RuntimePacket::new(header, migration_request().to_bytes()?.to_vec(), Vec::new())?
    };
    assert!(matches!(
        server_receive_migrate_error(wrong_session).await,
        RuntimeError::UnexpectedMessage(_)
    ));

    let malformed = {
        let mut header = CommonHeader::new(MessageType::SessionMigrate, 1, 0);
        header.session_id = 1;
        RuntimePacket::new(header, vec![0], Vec::new())?
    };
    assert!(matches!(
        server_receive_migrate_error(malformed).await,
        RuntimeError::UnexpectedMessage(_)
    ));

    let same_transport = {
        let mut request = migration_request();
        request.new_transport_id = request.old_transport_id;
        let mut header = CommonHeader::new(
            MessageType::SessionMigrate,
            nnrp_core::SESSION_MIGRATE_METADATA_LEN as u32,
            0,
        );
        header.session_id = 1;
        RuntimePacket::new(header, request.to_bytes()?.to_vec(), Vec::new())?
    };
    assert!(matches!(
        server_send_migrate_ack_error(same_transport).await,
        RuntimeError::Protocol(_)
    ));
    Ok(())
}

#[tokio::test]
async fn quic_convenience_hooks_require_provider_crate() {
    assert!(matches!(
        NnrpClient::connect_quic("localhost:4433", NnrpClientConfig::default()).await,
        Err(RuntimeError::UnsupportedTransport(_))
    ));
    assert!(matches!(
        NnrpServer::bind_quic("localhost:4433", NnrpServerConfig::default()).await,
        Err(RuntimeError::UnsupportedTransport(_))
    ));
}

#[tokio::test]
async fn client_accepts_custom_quic_transport_slot() -> Result<(), RuntimeError> {
    let config = NnrpClientConfig {
        transport: RuntimeTransportKind::Quic,
        requested_session_id: 9,
        ..Default::default()
    };
    let ack = open_ack(&SessionOpenMetadata {
        requested_session_id: config.requested_session_id,
        profile_id: config.profile_id,
        priority_class: config.priority_class,
        session_flags: 0,
        schema_id: config.schema_id,
        schema_version: config.schema_version,
        default_deadline_ms: config.default_deadline_ms,
        max_in_flight_operations: config.max_in_flight_operations,
        lease_ttl_hint_ms: config.lease_ttl_hint_ms,
        resume_token_bytes: 0,
        auth_bytes: 0,
        session_extension_bytes: 0,
        client_session_tag: config.requested_session_id as u64,
    });
    let mut ack_header = CommonHeader::new(
        MessageType::SessionOpenAck,
        SESSION_OPEN_ACK_METADATA_LEN as u32,
        0,
    );
    ack_header.session_id = ack.session_id;
    let writes = Arc::new(Mutex::new(Vec::new()));
    let transport = ScriptedTransport::new(
        RuntimeTransportKind::Quic,
        vec![RuntimePacket::new(
            ack_header,
            ack.to_bytes()?.to_vec(),
            Vec::new(),
        )?],
        Arc::clone(&writes),
    );

    let client = NnrpClient::from_transport(transport, config)?;
    let client_debug = format!("{client:?}");
    assert!(client_debug.contains("NnrpClient"));
    assert!(client_debug.contains("Quic"));
    let session = client.open_session().await?;
    let session_debug = format!("{session:?}");
    assert!(session_debug.contains("NnrpClientSession"));
    assert!(session_debug.contains("Quic"));
    assert_eq!(session.session_id(), 9);
    assert_eq!(
        session
            .build_migration_request(TransportId::Tcp, 0, 100)
            .old_transport_id,
        TransportId::Quic
    );
    session.close_transport().await?;

    let writes = writes.lock().expect("writes should lock");
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].header.message_type, MessageType::SessionOpen);
    Ok(())
}

#[tokio::test]
async fn server_accepts_custom_quic_listener_slot() -> Result<(), RuntimeError> {
    let open = session_open();
    let mut open_header = CommonHeader::new(
        MessageType::SessionOpen,
        SESSION_OPEN_METADATA_LEN as u32,
        0,
    );
    open_header.session_id = 0;
    let writes = Arc::new(Mutex::new(Vec::new()));
    let listener = ScriptedListener::new(
        RuntimeTransportKind::Quic,
        ScriptedTransport::new(
            RuntimeTransportKind::Quic,
            vec![RuntimePacket::new(
                open_header,
                open.to_bytes()?.to_vec(),
                Vec::new(),
            )?],
            Arc::clone(&writes),
        ),
    );
    let server = NnrpServer::from_listener(
        listener,
        NnrpServerConfig::default().with_transport(RuntimeTransportKind::Quic),
    )?;
    assert_eq!(server.local_addr()?.ip().to_string(), "127.0.0.1");
    let server_debug = format!("{server:?}");
    assert!(server_debug.contains("NnrpServer"));
    assert!(server_debug.contains("Quic"));

    let session = server.accept().await?;
    let session_debug = format!("{session:?}");
    assert!(session_debug.contains("NnrpServerSession"));
    assert!(session_debug.contains("Quic"));
    assert_eq!(session.session_id(), open.requested_session_id);
    assert_eq!(session.client_open().schema_id, TOKEN_DELTA_SCHEMA_ID);
    session.close().await?;

    let writes = writes.lock().expect("writes should lock");
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].header.message_type, MessageType::SessionOpenAck);
    assert_eq!(writes[0].header.session_id, open.requested_session_id);
    Ok(())
}

#[tokio::test]
async fn tcp_framed_listener_slot_exposes_local_addr() -> Result<(), RuntimeError> {
    let listener = TcpFramedListener::bind("127.0.0.1:0").await?;
    assert_eq!(listener.transport_kind(), RuntimeTransportKind::Tcp);
    assert_eq!(listener.local_addr()?.ip().to_string(), "127.0.0.1");
    Ok(())
}

#[tokio::test]
async fn transport_slot_rejects_config_mismatches() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    assert!(matches!(
        NnrpClient::from_transport(
            ScriptedTransport::new(RuntimeTransportKind::Quic, Vec::new(), Arc::clone(&writes)),
            NnrpClientConfig::default(),
        ),
        Err(RuntimeError::UnsupportedTransport(_))
    ));
    assert!(matches!(
        NnrpServer::from_listener(
            ScriptedListener::new(
                RuntimeTransportKind::Quic,
                ScriptedTransport::new(RuntimeTransportKind::Quic, Vec::new(), writes),
            ),
            NnrpServerConfig::default(),
        ),
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

struct ScriptedTransport {
    kind: RuntimeTransportKind,
    reads: VecDeque<RuntimePacket>,
    writes: Arc<Mutex<Vec<RuntimePacket>>>,
}

impl ScriptedTransport {
    fn new(
        kind: RuntimeTransportKind,
        reads: impl Into<Vec<RuntimePacket>>,
        writes: Arc<Mutex<Vec<RuntimePacket>>>,
    ) -> Self {
        Self {
            kind,
            reads: reads.into().into(),
            writes,
        }
    }
}

#[async_trait::async_trait]
impl FramedTransport for ScriptedTransport {
    fn transport_kind(&self) -> RuntimeTransportKind {
        self.kind
    }

    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
        self.reads
            .pop_front()
            .ok_or(RuntimeError::UnexpectedMessage(
                "scripted transport is empty",
            ))
    }

    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
        self.writes
            .lock()
            .expect("writes should lock")
            .push(packet.clone());
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }
}

struct ScriptedListener {
    kind: RuntimeTransportKind,
    transport: Mutex<Option<BoxedFramedTransport>>,
}

impl ScriptedListener {
    fn new(kind: RuntimeTransportKind, transport: ScriptedTransport) -> Self {
        Self {
            kind,
            transport: Mutex::new(Some(Box::new(transport))),
        }
    }
}

#[async_trait::async_trait]
impl FramedListener for ScriptedListener {
    fn transport_kind(&self) -> RuntimeTransportKind {
        self.kind
    }

    fn local_addr(&self) -> Result<std::net::SocketAddr, RuntimeError> {
        Ok("127.0.0.1:0".parse().expect("socket addr should parse"))
    }

    async fn accept(&self) -> Result<BoxedFramedTransport, RuntimeError> {
        self.transport
            .lock()
            .expect("transport should lock")
            .take()
            .ok_or(RuntimeError::UnexpectedMessage(
                "scripted listener has no transport",
            ))
    }
}

struct RequireSessionTag(u64);

impl NnrpServerPolicy for RequireSessionTag {
    fn validate_session_open(&self, open: &SessionOpenMetadata) -> Result<(), u32> {
        if open.client_session_tag == self.0 {
            Ok(())
        } else {
            Err(nnrp_core::SESSION_ERROR_LIMIT_REACHED)
        }
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

async fn client_migrate_error(packet: RuntimePacket) -> RuntimeError {
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
        let _migrate = transport.read_packet().await?;
        transport.write_packet(&packet).await
    });

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default())
        .await
        .expect("client should connect");
    let mut session = client.open_session().await.expect("session should open");
    let err = session
        .migrate_transport(migration_request())
        .await
        .expect_err("migration should reject scripted response");
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

async fn server_receive_migrate_error(packet: RuntimePacket) -> RuntimeError {
    server_receive_error(packet, |mut session| async move {
        session.receive_migrate().await.map(|_| ())
    })
    .await
}

async fn server_receive_runtime_control_error(packet: RuntimePacket) -> RuntimeError {
    server_receive_error(packet, |mut session| async move {
        session.receive_runtime_control().await.map(|_| ())
    })
    .await
}

async fn server_receive_scheduling_update_error(packet: RuntimePacket) -> RuntimeError {
    server_receive_error(packet, |mut session| async move {
        session.receive_scheduling_update().await.map(|_| ())
    })
    .await
}

async fn server_receive_pressure_update_error(packet: RuntimePacket) -> RuntimeError {
    server_receive_error(packet, |mut session| async move {
        session.receive_pressure_update().await.map(|_| ())
    })
    .await
}

async fn server_send_control_error<F, Fut>(send: F) -> RuntimeError
where
    F: FnOnce(nnrp_runtime::NnrpServerSession) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), RuntimeError>> + Send + 'static,
{
    let mut header = CommonHeader::new(
        MessageType::SessionClose,
        nnrp_core::SESSION_CLOSE_METADATA_LEN as u32,
        0,
    );
    header.session_id = 1;
    server_receive_error(
        RuntimePacket::new(
            header,
            close_request().to_bytes().unwrap().to_vec(),
            Vec::new(),
        )
        .expect("close packet should build"),
        send,
    )
    .await
}

async fn server_send_object_error<F, Fut>(send: F) -> RuntimeError
where
    F: FnOnce(nnrp_runtime::NnrpServerSession) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), RuntimeError>> + Send + 'static,
{
    let mut header = CommonHeader::new(
        MessageType::SessionClose,
        nnrp_core::SESSION_CLOSE_METADATA_LEN as u32,
        0,
    );
    header.session_id = 1;
    server_receive_error(
        RuntimePacket::new(
            header,
            close_request().to_bytes().unwrap().to_vec(),
            Vec::new(),
        )
        .expect("close packet should build"),
        send,
    )
    .await
}

async fn server_send_migrate_ack_error(packet: RuntimePacket) -> RuntimeError {
    server_receive_error(packet, |mut session| async move {
        let migrate = session.receive_migrate().await?;
        session
            .send_migrate_ack(&migrate.metadata, migrate_ack(11))
            .await
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

fn credit_update() -> PressureMetadata {
    PressureMetadata {
        scope_id: 1,
        credit_window: 9,
        pressure_level: BackpressureLevel::None as u16,
        pressure_reason: 0,
        retry_after_ms: 0,
        flags: 0,
    }
}

fn soft_backpressure() -> PressureMetadata {
    PressureMetadata {
        scope_id: 1,
        credit_window: 2,
        pressure_level: BackpressureLevel::Soft as u16,
        pressure_reason: 1,
        retry_after_ms: 25,
        flags: 0,
    }
}

fn partial_result(operation_id: u64) -> PartialResultMetadata {
    PartialResultMetadata {
        operation_id,
        result_sequence: 1,
        object_id: 0,
        delta_sequence: 0,
        body_bytes: 7,
        flags: 0,
    }
}

fn drop_reason(operation_id: u64) -> ResultDropReasonMetadata {
    ResultDropReasonMetadata {
        operation_id,
        result_sequence: 1,
        drop_reason_code: 7,
        source_role: 2,
        flags: 0,
        diagnostic_bytes: 0,
    }
}

fn object_descriptor() -> ObjectDescriptorMetadata {
    ObjectDescriptorMetadata {
        object_id: 900,
        object_kind: RuntimeObjectKind::ImageTile,
        producer_role: RuntimeRole::Server,
        consumer_role: RuntimeRole::Client,
        session_id: 1,
        byte_size: 4,
        compute_cost_units: 2,
        memory_location_hint: MemoryLocationHint::HostMemory,
        ownership_hint: OwnershipHint::SessionOwned,
        lifetime_hint_ms: 1_000,
        metadata_bytes: 4,
    }
}

fn object_reference(operation_id: u64) -> ObjectReferenceMetadata {
    ObjectReferenceMetadata {
        object_id: 900,
        operation_id,
        object_version: 1,
        offset: 0,
        length: 4,
        flags: 0,
        metadata_bytes: 0,
    }
}

fn object_delta() -> ObjectDeltaMetadata {
    ObjectDeltaMetadata {
        object_id: 900,
        delta_sequence: 1,
        region_offset: 0,
        region_bytes: 4,
        delta_bytes: 4,
        flags: 0,
        metadata_bytes: 0,
    }
}

fn object_release(
    operation_id: u64,
    release_reason: ObjectReleaseReason,
    diagnostic_bytes: u32,
) -> ObjectReleaseMetadata {
    ObjectReleaseMetadata {
        object_id: 900,
        operation_id,
        release_reason,
        source_role: RuntimeRole::Server,
        flags: 0,
        diagnostic_bytes,
    }
}

fn cache_reference() -> CacheReferenceMetadata {
    CacheReferenceMetadata {
        cache_key_hi: 0x1234,
        cache_key_lo: 0x5678,
        profile_id: STANDARD_PROFILE_TOKEN,
        reuse_scope: CacheReuseScope::Session,
        lease_id: 0,
        producer_trace_id: 99,
        expiration_hint_ms: 1_000,
        metadata_bytes: 4,
        flags: 0,
    }
}

fn cache_miss() -> CacheMissMetadata {
    CacheMissMetadata {
        cache_key_hi: 0x1234,
        cache_key_lo: 0x5678,
        miss_reason: CacheMissReason::SchemaMismatch,
        profile_id: STANDARD_PROFILE_TOKEN,
        diagnostic_bytes: 6,
    }
}

fn cache_invalidate() -> CacheInvalidateMetadata {
    CacheInvalidateMetadata {
        invalidate_scope: CacheInvalidateScope::ObjectKey,
        cache_namespace: 42,
        cache_key_hi: 0x1234,
        cache_key_lo: 0x5678,
        reason_code: 77,
    }
}

fn control_event_packet(
    message_type: MessageType,
    session_id: u32,
    metadata: Vec<u8>,
    body: Vec<u8>,
) -> Result<RuntimePacket, RuntimeError> {
    let mut header = CommonHeader::new(message_type, metadata.len() as u32, body.len() as u32);
    header.session_id = session_id;
    Ok(RuntimePacket::new(header, metadata, body)?)
}

fn object_event_packet(
    message_type: MessageType,
    session_id: u32,
    metadata: Vec<u8>,
    body: Vec<u8>,
) -> Result<RuntimePacket, RuntimeError> {
    let mut header = CommonHeader::new(message_type, metadata.len() as u32, body.len() as u32);
    header.session_id = session_id;
    Ok(RuntimePacket::new(header, metadata, body)?)
}

fn migration_request() -> nnrp_core::SessionMigrateMetadata {
    nnrp_core::SessionMigrateMetadata {
        old_transport_id: TransportId::Tcp,
        new_transport_id: TransportId::Quic,
        last_result_frame_id: 10,
        client_migrate_ts_us: 100,
    }
}

fn migrate_ack(resume_from_frame_id: u64) -> SessionMigrateAckMetadata {
    SessionMigrateAckMetadata {
        accept_code: 0,
        resume_from_frame_id,
        grace_window_ms: 500,
        server_migrate_ts_us: 200,
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
