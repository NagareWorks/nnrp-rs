use nnrp_core::TransportId;
use nnrp_ffi::{
    nnrp_connection_close, nnrp_server_accept_begin, nnrp_server_accept_release, nnrp_server_bind,
    NnrpBufferView, NnrpFfiStatus, NnrpHandle, NnrpHandleKind, NnrpServerAcceptBeginRequest,
    NnrpServerBindRequest, NnrpTransportOpenRequest,
};

unsafe extern "C" {
    fn nnrp_transport_listen(
        request: NnrpTransportOpenRequest,
        out_listener: *mut NnrpHandle,
    ) -> NnrpFfiStatus;
    fn nnrp_transport_close(handle: NnrpHandle) -> NnrpFfiStatus;
    fn nnrp_transport_runtime_shutdown() -> NnrpFfiStatus;
}

fn open_request(endpoint: &str) -> NnrpTransportOpenRequest {
    NnrpTransportOpenRequest {
        transport_id: TransportId::Tcp as u32,
        flags: 0,
        endpoint: NnrpBufferView {
            ptr: endpoint.as_ptr(),
            len: endpoint.len(),
        },
        config: NnrpHandle::invalid(),
        max_packet_bytes: 0,
        timeout_ms: 5_000,
        reserved0: 0,
    }
}

#[test]
fn shutdown_invalidates_live_role_handles_and_restarts_the_runtime() {
    unsafe {
        let mut listener = NnrpHandle::invalid();
        assert_eq!(
            nnrp_transport_listen(open_request("tcp://127.0.0.1:0"), &mut listener,),
            NnrpFfiStatus::ok()
        );
        let mut server = NnrpHandle::invalid();
        assert_eq!(
            nnrp_server_bind(
                NnrpServerBindRequest {
                    server_id: 701_000,
                    generation: 1,
                    reserved0: 0,
                    transport_listener: listener,
                },
                &mut server,
            ),
            NnrpFfiStatus::ok()
        );
        let mut accept = NnrpHandle::invalid();
        assert_eq!(
            nnrp_server_accept_begin(
                NnrpServerAcceptBeginRequest {
                    server,
                    accept_handle_id: 701_001,
                    generation: 1,
                    reserved0: 0,
                },
                &mut accept,
            ),
            NnrpFfiStatus::ok()
        );

        assert_eq!(nnrp_transport_runtime_shutdown(), NnrpFfiStatus::ok());
        assert_eq!(
            nnrp_server_accept_release(accept),
            NnrpFfiStatus::invalid_handle(NnrpHandleKind::ServerAccept as u32)
        );
        assert_eq!(
            nnrp_connection_close(server),
            NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
        );

        let mut restarted_listener = NnrpHandle::invalid();
        assert_eq!(
            nnrp_transport_listen(open_request("tcp://127.0.0.1:0"), &mut restarted_listener,),
            NnrpFfiStatus::ok()
        );
        assert_eq!(
            nnrp_transport_close(restarted_listener),
            NnrpFfiStatus::ok()
        );
        assert_eq!(nnrp_transport_runtime_shutdown(), NnrpFfiStatus::ok());
    }
}
