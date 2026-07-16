use std::ptr;
use std::slice;
use std::thread;

use nnrp_core::{
    FrameSubmitMetadata, InputProfile, PayloadKindBitmap, ResultClass, ResultPushMetadata,
    SubmitMode, TileIndexMode, TransportId, PROFILE_TOKEN, TOKEN_DELTA_SCHEMA_ID,
    TOKEN_DELTA_SCHEMA_VERSION,
};
use nnrp_ffi::{
    nnrp_buffer_release, nnrp_client_await_event, nnrp_client_await_events, nnrp_client_connect,
    nnrp_client_open_session, nnrp_client_submit, nnrp_connection_close, nnrp_server_accept,
    nnrp_server_await_events, nnrp_server_bind, nnrp_server_close, nnrp_server_send_result,
    NnrpBufferView, NnrpClientConnectRequest, NnrpEvent, NnrpEventKind, NnrpFfiStatus, NnrpHandle,
    NnrpHandleKind, NnrpPollResult, NnrpRoleEventPollRequest, NnrpServerAcceptRequest,
    NnrpServerBindRequest, NnrpServerSendResultRequest, NnrpSessionOpenRequest, NnrpSubmitRequest,
    NnrpTransportFrameBatch, NnrpTransportOpenRequest, NnrpTransportReadBatchRequest,
};

unsafe extern "C" {
    fn nnrp_transport_connect(
        request: NnrpTransportOpenRequest,
        out_connection: *mut NnrpHandle,
    ) -> NnrpFfiStatus;
    fn nnrp_transport_listen(
        request: NnrpTransportOpenRequest,
        out_listener: *mut NnrpHandle,
    ) -> NnrpFfiStatus;
    fn nnrp_transport_listener_endpoint(
        listener: NnrpHandle,
        out_buffer: *mut NnrpHandle,
        out_endpoint: *mut NnrpBufferView,
    ) -> NnrpFfiStatus;
    fn nnrp_transport_read_batch(
        request: NnrpTransportReadBatchRequest,
        out_batch: *mut NnrpTransportFrameBatch,
    ) -> NnrpFfiStatus;
}

fn view(bytes: &[u8]) -> NnrpBufferView {
    NnrpBufferView {
        ptr: bytes.as_ptr(),
        len: bytes.len(),
    }
}

fn open_request(transport_id: TransportId, endpoint: &str) -> NnrpTransportOpenRequest {
    NnrpTransportOpenRequest {
        transport_id: transport_id as u32,
        flags: 0,
        endpoint: view(endpoint.as_bytes()),
        config: NnrpHandle::invalid(),
        max_packet_bytes: 0,
        timeout_ms: 5_000,
        reserved0: 0,
    }
}

fn token_submit(operation_id: u64) -> FrameSubmitMetadata {
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
        operation_id,
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
        active_profile_id: PROFILE_TOKEN,
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

fn poll_request(scope: NnrpHandle) -> NnrpRoleEventPollRequest {
    NnrpRoleEventPollRequest {
        scope,
        max_events: 1,
        timeout_ms: 5_000,
        flags: 0,
        reserved0: 0,
    }
}

unsafe fn assert_role_handshake(transport_id: TransportId, listen_endpoint: &str, id_base: u64) {
    let mut listener = NnrpHandle::invalid();
    assert_eq!(
        nnrp_transport_listen(open_request(transport_id, listen_endpoint), &mut listener,),
        NnrpFfiStatus::ok()
    );

    let mut endpoint_owner = NnrpHandle::invalid();
    let mut endpoint_view = NnrpBufferView::empty();
    assert_eq!(
        nnrp_transport_listener_endpoint(listener, &mut endpoint_owner, &mut endpoint_view,),
        NnrpFfiStatus::ok()
    );
    let endpoint =
        String::from_utf8(slice::from_raw_parts(endpoint_view.ptr, endpoint_view.len).to_vec())
            .expect("listener endpoint must be UTF-8");
    assert_eq!(nnrp_buffer_release(endpoint_owner), NnrpFfiStatus::ok());

    let mut foreign_listener = listener;
    foreign_listener.flags ^= u32::MAX;
    let mut rejected_server = NnrpHandle::invalid();
    assert_eq!(
        nnrp_server_bind(
            NnrpServerBindRequest {
                server_id: id_base,
                generation: 1,
                reserved0: 0,
                transport_listener: foreign_listener,
            },
            &mut rejected_server,
        ),
        NnrpFfiStatus::invalid_handle(NnrpHandleKind::TransportListener as u32)
    );

    let mut server = NnrpHandle::invalid();
    assert_eq!(
        nnrp_server_bind(
            NnrpServerBindRequest {
                server_id: id_base + 1,
                generation: 1,
                reserved0: 0,
                transport_listener: listener,
            },
            &mut server,
        ),
        NnrpFfiStatus::ok()
    );
    assert_eq!(
        nnrp_transport_listener_endpoint(listener, &mut endpoint_owner, &mut endpoint_view,),
        NnrpFfiStatus::invalid_handle(NnrpHandleKind::TransportListener as u32)
    );

    let accept = thread::spawn(move || {
        let mut session = NnrpHandle::invalid();
        let status = nnrp_server_accept(
            NnrpServerAcceptRequest {
                server,
                session_handle_id: id_base + 4,
                generation: 1,
                timeout_ms: 5_000,
            },
            &mut session,
        );
        (status, session, server)
    });

    let mut transport_connection = NnrpHandle::invalid();
    assert_eq!(
        nnrp_transport_connect(
            open_request(transport_id, &endpoint),
            &mut transport_connection,
        ),
        NnrpFfiStatus::ok()
    );
    let mut foreign_connection = transport_connection;
    foreign_connection.flags ^= u32::MAX;
    let mut rejected_client = NnrpHandle::invalid();
    assert_eq!(
        nnrp_client_connect(
            NnrpClientConnectRequest {
                connection_id: id_base + 5,
                generation: 1,
                reserved0: 0,
                transport_connection: foreign_connection,
            },
            &mut rejected_client,
        ),
        NnrpFfiStatus::invalid_handle(NnrpHandleKind::TransportConnection as u32)
    );
    let mut client = NnrpHandle::invalid();
    assert_eq!(
        nnrp_client_connect(
            NnrpClientConnectRequest {
                connection_id: id_base + 2,
                generation: 1,
                reserved0: 0,
                transport_connection,
            },
            &mut client,
        ),
        NnrpFfiStatus::ok()
    );
    let mut consumed_batch = NnrpTransportFrameBatch::empty();
    assert_eq!(
        nnrp_transport_read_batch(
            NnrpTransportReadBatchRequest {
                connection: transport_connection,
                max_frames: 1,
                timeout_ms: 1,
                max_bytes: 0,
            },
            &mut consumed_batch,
        ),
        NnrpFfiStatus::invalid_handle(NnrpHandleKind::TransportConnection as u32)
    );

    let mut client_session = NnrpHandle::invalid();
    assert_eq!(
        nnrp_client_open_session(
            NnrpSessionOpenRequest {
                connection: client,
                requested_session_id: (id_base + 3) as u32,
                generation: 1,
                profile_id: PROFILE_TOKEN,
                schema_id: TOKEN_DELTA_SCHEMA_ID,
                schema_version: TOKEN_DELTA_SCHEMA_VERSION,
            },
            &mut client_session,
        ),
        NnrpFfiStatus::ok()
    );

    let (server_status, server_session, server) = accept.join().expect("accept thread joins");
    assert_eq!(server_status, NnrpFfiStatus::ok());
    assert_ne!(client_session, NnrpHandle::invalid());
    assert_ne!(server_session, NnrpHandle::invalid());

    let submit_body = b"role-carrier-submit";
    let mut submit_payload = token_submit(id_base + 6)
        .to_bytes()
        .expect("submit metadata")
        .to_vec();
    submit_payload.extend_from_slice(submit_body);
    let mut client_operation = NnrpHandle::invalid();
    let submit_request = NnrpSubmitRequest {
        session: client_session,
        operation_id: id_base + 6,
        frame_id: 42,
        payload: view(&submit_payload),
    };
    assert_eq!(
        nnrp_client_submit(submit_request, ptr::null_mut()),
        NnrpFfiStatus::invalid_argument(12)
    );
    assert_eq!(
        nnrp_client_submit(
            NnrpSubmitRequest {
                payload: NnrpBufferView {
                    ptr: ptr::null(),
                    len: 1,
                },
                ..submit_request
            },
            &mut client_operation,
        ),
        NnrpFfiStatus::invalid_argument(1)
    );
    assert_eq!(
        nnrp_client_submit(
            NnrpSubmitRequest {
                payload: view(&submit_payload[..1]),
                ..submit_request
            },
            &mut client_operation,
        ),
        NnrpFfiStatus::invalid_argument(144)
    );
    let malformed_submit = [u8::MAX; 72];
    assert_ne!(
        nnrp_client_submit(
            NnrpSubmitRequest {
                payload: view(&malformed_submit),
                ..submit_request
            },
            &mut client_operation,
        ),
        NnrpFfiStatus::ok()
    );
    assert_ne!(
        nnrp_client_submit(
            NnrpSubmitRequest {
                session: NnrpHandle::invalid(),
                ..submit_request
            },
            &mut client_operation,
        ),
        NnrpFfiStatus::ok()
    );
    assert_eq!(
        nnrp_client_submit(submit_request, &mut client_operation),
        NnrpFfiStatus::ok()
    );
    assert_ne!(client_operation.id, submit_request.operation_id);
    assert_eq!(
        nnrp_client_submit(submit_request, &mut NnrpHandle::invalid()),
        NnrpFfiStatus::invalid_argument(145)
    );
    assert_ne!(
        nnrp_client_submit(
            NnrpSubmitRequest {
                operation_id: id_base + 7,
                frame_id: 41,
                ..submit_request
            },
            &mut NnrpHandle::invalid(),
        ),
        NnrpFfiStatus::ok()
    );

    let mut event_count = 0usize;
    assert_eq!(
        nnrp_client_await_event(client_session, ptr::null_mut()),
        NnrpFfiStatus::invalid_argument(17)
    );
    assert_eq!(
        nnrp_client_await_events(
            poll_request(client_session),
            ptr::null_mut(),
            0,
            &mut event_count,
        ),
        NnrpFfiStatus::invalid_argument(32)
    );
    assert_eq!(
        nnrp_client_await_events(
            poll_request(client_session),
            &mut NnrpEvent::none(),
            1,
            ptr::null_mut(),
        ),
        NnrpFfiStatus::invalid_argument(31)
    );
    assert_eq!(
        nnrp_client_await_events(
            NnrpRoleEventPollRequest {
                flags: 1,
                ..poll_request(client_session)
            },
            &mut NnrpEvent::none(),
            1,
            &mut event_count,
        ),
        NnrpFfiStatus::invalid_argument(146)
    );
    assert_ne!(
        nnrp_client_await_events(
            poll_request(server_session),
            &mut NnrpEvent::none(),
            1,
            &mut event_count,
        ),
        NnrpFfiStatus::ok()
    );
    assert_ne!(
        nnrp_client_await_events(
            NnrpRoleEventPollRequest {
                scope: NnrpHandle::invalid(),
                max_events: 0,
                ..poll_request(client_session)
            },
            &mut NnrpEvent::none(),
            1,
            &mut event_count,
        ),
        NnrpFfiStatus::ok()
    );
    let mut invalid_poll = NnrpPollResult {
        status: NnrpFfiStatus::ok(),
        has_event: 0,
        event: NnrpEvent::none(),
    };
    let invalid_poll_status = nnrp_client_await_event(NnrpHandle::invalid(), &mut invalid_poll);
    assert_ne!(invalid_poll_status, NnrpFfiStatus::ok());
    assert_eq!(invalid_poll.status, invalid_poll_status);
    assert_eq!(invalid_poll.has_event, 0);
    assert_eq!(
        nnrp_server_await_events(
            poll_request(server_session),
            ptr::null_mut(),
            0,
            &mut event_count,
        ),
        NnrpFfiStatus::invalid_argument(32)
    );

    let mut server_events = [NnrpEvent::none(); 2];
    let mut server_event_count = 0usize;
    assert_eq!(
        nnrp_server_await_events(
            NnrpRoleEventPollRequest {
                max_events: 2,
                ..poll_request(server_session)
            },
            server_events.as_mut_ptr(),
            server_events.len(),
            &mut server_event_count,
        ),
        NnrpFfiStatus::ok()
    );
    let server_event = server_events[0];
    assert_eq!(server_event_count, 1);
    assert_eq!(server_event.kind, NnrpEventKind::SubmitAccepted as u32);
    assert_eq!(server_event.frame_id, 42);
    assert_ne!(server_event.operation.id, submit_request.operation_id);
    assert_ne!(server_event.operation, client_operation);
    assert_eq!(
        slice::from_raw_parts(server_event.payload.ptr, server_event.payload.len),
        submit_payload
    );
    assert_eq!(
        nnrp_buffer_release(server_event.payload_owner),
        NnrpFfiStatus::ok()
    );

    let result_body = b"role-carrier-result";
    let mut result_payload = token_result().to_bytes().expect("result metadata").to_vec();
    result_payload.extend_from_slice(result_body);
    assert_eq!(
        nnrp_server_send_result(NnrpServerSendResultRequest {
            operation: NnrpHandle::invalid(),
            payload: view(&result_payload),
        })
        .status_code,
        nnrp_ffi::NnrpFfiStatusCode::InvalidHandle as u32
    );
    assert_eq!(
        nnrp_server_send_result(NnrpServerSendResultRequest {
            operation: server_event.operation,
            payload: view(&result_payload[..1]),
        }),
        NnrpFfiStatus::invalid_argument(148)
    );
    let malformed_result = [u8::MAX; 64];
    assert_ne!(
        nnrp_server_send_result(NnrpServerSendResultRequest {
            operation: server_event.operation,
            payload: view(&malformed_result),
        }),
        NnrpFfiStatus::ok()
    );
    assert_eq!(
        nnrp_server_send_result(NnrpServerSendResultRequest {
            operation: server_event.operation,
            payload: view(&result_payload),
        }),
        NnrpFfiStatus::ok()
    );

    let mut client_events = [NnrpEvent::none(); 2];
    let mut client_event_count = 0usize;
    assert_eq!(
        nnrp_client_await_events(
            NnrpRoleEventPollRequest {
                max_events: 2,
                ..poll_request(client_session)
            },
            client_events.as_mut_ptr(),
            client_events.len(),
            &mut client_event_count,
        ),
        NnrpFfiStatus::ok()
    );
    let client_event = client_events[0];
    assert_eq!(client_event_count, 1);
    assert_eq!(client_event.kind, NnrpEventKind::ResultPushed as u32);
    assert_eq!(client_event.operation, client_operation);
    assert_eq!(client_event.frame_id, 42);
    assert_eq!(
        slice::from_raw_parts(client_event.payload.ptr, client_event.payload.len),
        result_payload
    );
    assert_eq!(
        nnrp_buffer_release(client_event.payload_owner),
        NnrpFfiStatus::ok()
    );

    assert_eq!(nnrp_server_close(server_session), NnrpFfiStatus::ok());
    assert_eq!(nnrp_connection_close(client), NnrpFfiStatus::ok());
    assert_eq!(nnrp_connection_close(server), NnrpFfiStatus::ok());
}

#[test]
fn tcp_role_runtime_adopts_carriers_and_completes_real_handshake() {
    unsafe {
        assert_role_handshake(TransportId::Tcp, "tcp://127.0.0.1:0", 700_000);
    }
}

#[test]
fn role_runtime_rejects_invalid_arguments_and_cross_role_handles() {
    unsafe {
        let mut output = NnrpHandle::invalid();
        assert_eq!(
            nnrp_client_connect(
                NnrpClientConnectRequest {
                    connection_id: 0,
                    generation: 1,
                    reserved0: 0,
                    transport_connection: NnrpHandle::invalid(),
                },
                &mut output,
            ),
            NnrpFfiStatus::invalid_argument(10)
        );
        assert_eq!(
            nnrp_client_open_session(
                NnrpSessionOpenRequest {
                    connection: NnrpHandle::invalid(),
                    requested_session_id: 0,
                    generation: 1,
                    profile_id: PROFILE_TOKEN,
                    schema_id: TOKEN_DELTA_SCHEMA_ID,
                    schema_version: TOKEN_DELTA_SCHEMA_VERSION,
                },
                &mut output,
            ),
            NnrpFfiStatus::invalid_argument(11)
        );
        assert_eq!(
            nnrp_server_bind(
                NnrpServerBindRequest {
                    server_id: 0,
                    generation: 1,
                    reserved0: 0,
                    transport_listener: NnrpHandle::invalid(),
                },
                &mut output,
            ),
            NnrpFfiStatus::invalid_argument(18)
        );
        assert_eq!(
            nnrp_server_accept(
                NnrpServerAcceptRequest {
                    server: NnrpHandle::invalid(),
                    session_handle_id: 0,
                    generation: 1,
                    timeout_ms: 1,
                },
                &mut output,
            ),
            NnrpFfiStatus::invalid_argument(19)
        );

        let mut listener = NnrpHandle::invalid();
        assert_eq!(
            nnrp_transport_listen(
                open_request(TransportId::Tcp, "tcp://127.0.0.1:0"),
                &mut listener,
            ),
            NnrpFfiStatus::ok()
        );
        let mut endpoint_owner = NnrpHandle::invalid();
        let mut endpoint_view = NnrpBufferView::empty();
        assert_eq!(
            nnrp_transport_listener_endpoint(listener, &mut endpoint_owner, &mut endpoint_view),
            NnrpFfiStatus::ok()
        );
        let endpoint =
            String::from_utf8(slice::from_raw_parts(endpoint_view.ptr, endpoint_view.len).to_vec())
                .expect("listener endpoint must be UTF-8");
        assert_eq!(nnrp_buffer_release(endpoint_owner), NnrpFfiStatus::ok());

        let mut server = NnrpHandle::invalid();
        assert_eq!(
            nnrp_server_bind(
                NnrpServerBindRequest {
                    server_id: 730_000,
                    generation: 1,
                    reserved0: 0,
                    transport_listener: listener,
                },
                &mut server,
            ),
            NnrpFfiStatus::ok()
        );
        let mut transport_connection = NnrpHandle::invalid();
        assert_eq!(
            nnrp_transport_connect(
                open_request(TransportId::Tcp, &endpoint),
                &mut transport_connection,
            ),
            NnrpFfiStatus::ok()
        );
        let mut client = NnrpHandle::invalid();
        assert_eq!(
            nnrp_client_connect(
                NnrpClientConnectRequest {
                    connection_id: 730_001,
                    generation: 1,
                    reserved0: 0,
                    transport_connection,
                },
                &mut client,
            ),
            NnrpFfiStatus::ok()
        );

        let session_request = |connection| NnrpSessionOpenRequest {
            connection,
            requested_session_id: 730_002,
            generation: 1,
            profile_id: PROFILE_TOKEN,
            schema_id: TOKEN_DELTA_SCHEMA_ID,
            schema_version: TOKEN_DELTA_SCHEMA_VERSION,
        };
        assert_eq!(
            nnrp_client_open_session(session_request(server), &mut output),
            NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
        );
        assert_eq!(
            nnrp_client_open_session(session_request(NnrpHandle::invalid()), &mut output),
            NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
        );

        let accept_request = |server| NnrpServerAcceptRequest {
            server,
            session_handle_id: 730_003,
            generation: 1,
            timeout_ms: 1,
        };
        assert_eq!(
            nnrp_server_accept(accept_request(client), &mut output),
            NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
        );
        assert_eq!(
            nnrp_server_accept(accept_request(NnrpHandle::invalid()), &mut output),
            NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
        );

        assert_eq!(nnrp_connection_close(client), NnrpFfiStatus::ok());
        assert_eq!(nnrp_connection_close(server), NnrpFfiStatus::ok());
    }
}

#[cfg(feature = "transport-websocket")]
#[test]
fn websocket_role_runtime_adopts_carriers_and_completes_real_handshake() {
    unsafe {
        assert_role_handshake(TransportId::WebSocket, "ws://127.0.0.1:0/nnrp", 710_000);
    }
}

#[cfg(all(feature = "transport-ipc", windows))]
#[test]
fn named_pipe_role_runtime_adopts_carriers_and_completes_real_handshake() {
    let endpoint = format!("npipe://nnrp-role-{}", std::process::id());
    unsafe {
        assert_role_handshake(TransportId::Ipc, &endpoint, 720_000);
    }
}

#[cfg(all(feature = "transport-ipc", unix))]
#[test]
fn unix_ipc_role_runtime_adopts_carriers_and_completes_real_handshake() {
    let path = std::env::temp_dir().join(format!("nnrp-role-{}.sock", std::process::id()));
    let endpoint = format!("unix://{}", path.display());
    unsafe {
        assert_role_handshake(TransportId::Ipc, &endpoint, 720_000);
    }
    let _ = std::fs::remove_file(path);
}
