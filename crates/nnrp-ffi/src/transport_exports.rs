use crate::{
    transport::{
        transport_accept, transport_client_security_config_create, transport_close,
        transport_connect, transport_listen, transport_listener_endpoint, transport_probe,
        transport_read_batch, transport_server_security_config_create, transport_write_batch,
    },
    NnrpBufferView, NnrpFfiStatus, NnrpHandle, NnrpTransportAcceptRequest,
    NnrpTransportClientSecurityConfigRequest, NnrpTransportFrameBatch, NnrpTransportOpenRequest,
    NnrpTransportProbeRequest, NnrpTransportProbeResult, NnrpTransportReadBatchRequest,
    NnrpTransportServerSecurityConfigRequest, NnrpTransportWriteBatchRequest,
};

#[no_mangle]
pub unsafe extern "C" fn nnrp_transport_client_security_config_create(
    request: NnrpTransportClientSecurityConfigRequest,
    out_config: *mut NnrpHandle,
) -> NnrpFfiStatus {
    transport_client_security_config_create(request, out_config)
}

#[no_mangle]
pub unsafe extern "C" fn nnrp_transport_server_security_config_create(
    request: NnrpTransportServerSecurityConfigRequest,
    out_config: *mut NnrpHandle,
) -> NnrpFfiStatus {
    transport_server_security_config_create(request, out_config)
}

#[no_mangle]
pub unsafe extern "C" fn nnrp_transport_connect(
    request: NnrpTransportOpenRequest,
    out_connection: *mut NnrpHandle,
) -> NnrpFfiStatus {
    transport_connect(request, out_connection)
}

#[no_mangle]
pub unsafe extern "C" fn nnrp_transport_listen(
    request: NnrpTransportOpenRequest,
    out_listener: *mut NnrpHandle,
) -> NnrpFfiStatus {
    transport_listen(request, out_listener)
}

#[no_mangle]
pub unsafe extern "C" fn nnrp_transport_listener_endpoint(
    listener: NnrpHandle,
    out_buffer: *mut NnrpHandle,
    out_endpoint: *mut NnrpBufferView,
) -> NnrpFfiStatus {
    transport_listener_endpoint(listener, out_buffer, out_endpoint)
}

#[no_mangle]
pub unsafe extern "C" fn nnrp_transport_accept(
    request: NnrpTransportAcceptRequest,
    out_connection: *mut NnrpHandle,
) -> NnrpFfiStatus {
    transport_accept(request, out_connection)
}

#[no_mangle]
pub unsafe extern "C" fn nnrp_transport_write_batch(
    request: NnrpTransportWriteBatchRequest,
) -> NnrpFfiStatus {
    transport_write_batch(request)
}

#[no_mangle]
pub unsafe extern "C" fn nnrp_transport_read_batch(
    request: NnrpTransportReadBatchRequest,
    out_batch: *mut NnrpTransportFrameBatch,
) -> NnrpFfiStatus {
    transport_read_batch(request, out_batch)
}

#[no_mangle]
pub unsafe extern "C" fn nnrp_transport_probe(
    request: NnrpTransportProbeRequest,
    out_result: *mut NnrpTransportProbeResult,
) -> NnrpFfiStatus {
    transport_probe(request, out_result)
}

#[no_mangle]
pub unsafe extern "C" fn nnrp_transport_close(handle: NnrpHandle) -> NnrpFfiStatus {
    transport_close(handle)
}
