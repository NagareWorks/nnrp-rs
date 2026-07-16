use std::slice;
use std::thread;

use nnrp_core::{TransportId, PROFILE_TOKEN, TOKEN_DELTA_SCHEMA_ID, TOKEN_DELTA_SCHEMA_VERSION};
use nnrp_ffi::{
    nnrp_buffer_release, nnrp_client_connect, nnrp_client_open_session, nnrp_connection_close,
    nnrp_server_accept, nnrp_server_bind, nnrp_server_close, NnrpBufferView,
    NnrpClientConnectRequest, NnrpFfiStatus, NnrpHandle, NnrpHandleKind, NnrpServerAcceptRequest,
    NnrpServerBindRequest, NnrpSessionOpenRequest, NnrpTransportFrameBatch,
    NnrpTransportOpenRequest, NnrpTransportReadBatchRequest,
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
