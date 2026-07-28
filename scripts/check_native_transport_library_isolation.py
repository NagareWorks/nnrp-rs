#!/usr/bin/env python3
import argparse
import ctypes
import queue
import threading
from pathlib import Path

import smoke_test_native_transport_ffi as smoke


NNRP_FFI_STATUS_INVALID_HANDLE = 2


def require_invalid_handle(status: smoke.NnrpFfiStatus, operation: str) -> None:
    if status.status_code != NNRP_FFI_STATUS_INVALID_HANDLE:
        raise RuntimeError(
            f"{operation} must reject a foreign handle: "
            f"status={status.status_code} family={status.error_family} "
            f"protocol={status.protocol_error_code} detail={status.detail_code}"
        )


def connected_pair(
    library: ctypes.CDLL, scope: str
) -> tuple[smoke.NnrpHandle, smoke.NnrpHandle, smoke.NnrpHandle, Path | None]:
    transport_id = smoke.TRANSPORT_IDS[scope]
    endpoint, ipc_path = smoke.endpoint_for(scope)
    endpoint_owner, listen_request = smoke.open_request(
        transport_id, endpoint, smoke.invalid_handle()
    )
    listener = smoke.invalid_handle()
    smoke.require_ok(
        library.nnrp_transport_listen(listen_request, ctypes.byref(listener)),
        f"{scope} isolation listen",
    )

    endpoint_buffer = smoke.invalid_handle()
    endpoint_view = smoke.NnrpBufferView()
    smoke.require_ok(
        library.nnrp_transport_listener_endpoint(
            listener, ctypes.byref(endpoint_buffer), ctypes.byref(endpoint_view)
        ),
        f"{scope} isolation listener endpoint",
    )
    resolved_endpoint = ctypes.string_at(endpoint_view.ptr, endpoint_view.len)
    smoke.require_ok(
        library.nnrp_buffer_release(endpoint_buffer),
        f"{scope} isolation endpoint release",
    )

    accepted: queue.Queue[smoke.NnrpHandle | BaseException] = queue.Queue()

    def accept_connection() -> None:
        try:
            connection = smoke.invalid_handle()
            smoke.require_ok(
                library.nnrp_transport_accept(
                    smoke.NnrpTransportAcceptRequest(listener, 10_000, 0),
                    ctypes.byref(connection),
                ),
                f"{scope} isolation accept",
            )
            accepted.put(connection)
        except BaseException as error:
            accepted.put(error)

    accept_thread = threading.Thread(target=accept_connection, daemon=True)
    accept_thread.start()
    resolved_owner, connect_request = smoke.open_request(
        transport_id, resolved_endpoint, smoke.invalid_handle()
    )
    client = smoke.invalid_handle()
    smoke.require_ok(
        library.nnrp_transport_connect(connect_request, ctypes.byref(client)),
        f"{scope} isolation connect",
    )
    accept_thread.join(10)
    if accept_thread.is_alive():
        raise RuntimeError(f"{scope} isolation accept did not complete")
    server = accepted.get_nowait()
    if isinstance(server, BaseException):
        raise server
    _ = (endpoint_owner, resolved_owner)
    return listener, client, server, ipc_path


def assert_foreign_handle_rejected(
    origin_path: Path,
    origin_scope: str,
    destination_path: Path,
    destination_scope: str,
) -> None:
    origin = ctypes.CDLL(str(origin_path.resolve()))
    destination = ctypes.CDLL(str(destination_path.resolve()))
    smoke.configure_library(origin)
    smoke.configure_library(destination)
    listener, client, server, ipc_path = connected_pair(origin, origin_scope)
    try:
        payload = smoke.packet(901)
        payload_owner, payload_view = smoke.buffer_view(payload)
        request = smoke.NnrpTransportWriteBatchRequest(
            client, ctypes.pointer(payload_view), 1, 0
        )
        smoke.require_ok(
            origin.nnrp_transport_write_batch(request),
            f"{origin_scope} same-library packet write",
        )
        batch = smoke.NnrpTransportFrameBatch()
        smoke.require_ok(
            origin.nnrp_transport_read_batch(
                smoke.NnrpTransportReadBatchRequest(server, 1, 10_000, 0),
                ctypes.byref(batch),
            ),
            f"{origin_scope} same-library packet read",
        )
        smoke.require_ok(
            origin.nnrp_buffer_release(batch.payload_owner),
            f"{origin_scope} same-library packet release",
        )

        require_invalid_handle(
            destination.nnrp_transport_write_batch(request),
            f"{origin_scope}->{destination_scope} packet write",
        )
        role_connection = smoke.invalid_handle()
        require_invalid_handle(
            destination.nnrp_client_connect(
                smoke.NnrpClientConnectRequest(902, 1, 0, client),
                ctypes.byref(role_connection),
            ),
            f"{origin_scope}->{destination_scope} role adoption",
        )
        require_invalid_handle(
            destination.nnrp_transport_close(client),
            f"{origin_scope}->{destination_scope} close",
        )
        _ = payload_owner
    finally:
        for name, handle in (
            ("client", client),
            ("server", server),
            ("listener", listener),
        ):
            smoke.require_ok(
                origin.nnrp_transport_close(handle),
                f"{origin_scope} isolation close {name}",
            )
        if ipc_path is not None:
            ipc_path.unlink(missing_ok=True)


def parse_library(value: str) -> tuple[str, Path]:
    scope, separator, path = value.partition("=")
    if not separator or scope not in smoke.TRANSPORT_IDS or not path:
        raise argparse.ArgumentTypeError("library must use transport=path")
    return scope, Path(path)


def isolation_pairs(libraries: dict[str, Path]) -> list[tuple[str, str]]:
    required = {"tcp", "quic", "ipc", "websocket"}
    if set(libraries) != required:
        missing = ", ".join(sorted(required - set(libraries)))
        raise RuntimeError(f"isolation check requires every transport library; missing: {missing}")
    return [
        ("ipc", "tcp"),
        ("tcp", "quic"),
        ("tcp", "ipc"),
        ("tcp", "websocket"),
    ]


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Reject transport and role calls that cross native library instances."
    )
    parser.add_argument("--library", action="append", type=parse_library, required=True)
    args = parser.parse_args()
    libraries = dict(args.library)
    for origin, destination in isolation_pairs(libraries):
        assert_foreign_handle_rejected(
            libraries[origin], origin, libraries[destination], destination
        )
    print("native transport library isolation passed")


if __name__ == "__main__":
    main()
