#!/usr/bin/env python3
import argparse
import ctypes
import os
import queue
import shutil
import struct
import subprocess
import tempfile
import threading
import uuid
from pathlib import Path


TRANSPORT_IDS = {"quic": 1, "tcp": 2, "ipc": 3, "websocket": 4}


class NnrpHandle(ctypes.Structure):
    _fields_ = [
        ("kind", ctypes.c_uint32),
        ("id", ctypes.c_uint64),
        ("generation", ctypes.c_uint32),
        ("flags", ctypes.c_uint32),
    ]


class NnrpBufferView(ctypes.Structure):
    _fields_ = [("ptr", ctypes.POINTER(ctypes.c_uint8)), ("len", ctypes.c_size_t)]


class NnrpFfiStatus(ctypes.Structure):
    _fields_ = [
        ("status_code", ctypes.c_uint32),
        ("error_family", ctypes.c_uint32),
        ("protocol_error_code", ctypes.c_uint32),
        ("detail_code", ctypes.c_uint32),
    ]


class NnrpTransportOpenRequest(ctypes.Structure):
    _fields_ = [
        ("transport_id", ctypes.c_uint32),
        ("flags", ctypes.c_uint32),
        ("endpoint", NnrpBufferView),
        ("config", NnrpHandle),
        ("max_packet_bytes", ctypes.c_uint64),
        ("timeout_ms", ctypes.c_uint32),
        ("reserved0", ctypes.c_uint32),
    ]


class NnrpTransportAcceptRequest(ctypes.Structure):
    _fields_ = [
        ("listener", NnrpHandle),
        ("timeout_ms", ctypes.c_uint32),
        ("reserved0", ctypes.c_uint32),
    ]


class NnrpTransportWriteBatchRequest(ctypes.Structure):
    _fields_ = [
        ("connection", NnrpHandle),
        ("frames", ctypes.POINTER(NnrpBufferView)),
        ("frame_count", ctypes.c_uint32),
        ("flags", ctypes.c_uint32),
    ]


class NnrpTransportReadBatchRequest(ctypes.Structure):
    _fields_ = [
        ("connection", NnrpHandle),
        ("max_frames", ctypes.c_uint32),
        ("timeout_ms", ctypes.c_uint32),
        ("max_bytes", ctypes.c_uint64),
    ]


class NnrpTransportFrameBatch(ctypes.Structure):
    _fields_ = [
        ("payload_owner", NnrpHandle),
        ("payload", NnrpBufferView),
        ("frame_count", ctypes.c_uint32),
        ("reserved0", ctypes.c_uint32),
    ]


class NnrpTransportClientSecurityConfigRequest(ctypes.Structure):
    _fields_ = [
        ("transport_id", ctypes.c_uint32),
        ("flags", ctypes.c_uint32),
        ("server_name", NnrpBufferView),
        ("trusted_certificate_der", NnrpBufferView),
    ]


class NnrpTransportServerSecurityConfigRequest(ctypes.Structure):
    _fields_ = [
        ("transport_id", ctypes.c_uint32),
        ("flags", ctypes.c_uint32),
        ("certificate_der", NnrpBufferView),
        ("private_key_pkcs8_der", NnrpBufferView),
    ]


def invalid_handle() -> NnrpHandle:
    return NnrpHandle(0, 0, 0, 0)


def buffer_view(data: bytes) -> tuple[ctypes.Array, NnrpBufferView]:
    owner = (ctypes.c_uint8 * len(data)).from_buffer_copy(data)
    return owner, NnrpBufferView(ctypes.cast(owner, ctypes.POINTER(ctypes.c_uint8)), len(data))


def require_ok(status: NnrpFfiStatus, operation: str) -> None:
    if status.status_code != 0:
        raise RuntimeError(
            f"{operation} failed: status={status.status_code} family={status.error_family} "
            f"protocol={status.protocol_error_code} detail={status.detail_code}"
        )


def configure_library(library: ctypes.CDLL) -> None:
    signatures = {
        "nnrp_transport_client_security_config_create": (
            [NnrpTransportClientSecurityConfigRequest, ctypes.POINTER(NnrpHandle)],
            NnrpFfiStatus,
        ),
        "nnrp_transport_server_security_config_create": (
            [NnrpTransportServerSecurityConfigRequest, ctypes.POINTER(NnrpHandle)],
            NnrpFfiStatus,
        ),
        "nnrp_transport_connect": (
            [NnrpTransportOpenRequest, ctypes.POINTER(NnrpHandle)],
            NnrpFfiStatus,
        ),
        "nnrp_transport_listen": (
            [NnrpTransportOpenRequest, ctypes.POINTER(NnrpHandle)],
            NnrpFfiStatus,
        ),
        "nnrp_transport_accept": (
            [NnrpTransportAcceptRequest, ctypes.POINTER(NnrpHandle)],
            NnrpFfiStatus,
        ),
        "nnrp_transport_listener_endpoint": (
            [NnrpHandle, ctypes.POINTER(NnrpHandle), ctypes.POINTER(NnrpBufferView)],
            NnrpFfiStatus,
        ),
        "nnrp_transport_write_batch": ([NnrpTransportWriteBatchRequest], NnrpFfiStatus),
        "nnrp_transport_read_batch": (
            [NnrpTransportReadBatchRequest, ctypes.POINTER(NnrpTransportFrameBatch)],
            NnrpFfiStatus,
        ),
        "nnrp_transport_close": ([NnrpHandle], NnrpFfiStatus),
        "nnrp_buffer_release": ([NnrpHandle], NnrpFfiStatus),
    }
    for name, (argtypes, restype) in signatures.items():
        function = getattr(library, name)
        function.argtypes = argtypes
        function.restype = restype


def find_openssl() -> str | None:
    openssl = shutil.which("openssl")
    if openssl is not None:
        return openssl
    if os.name == "nt":
        git = shutil.which("git")
        if git is not None:
            bundled_openssl = (
                Path(git).resolve().parent.parent / "usr" / "bin" / "openssl.exe"
            )
            if bundled_openssl.is_file():
                return str(bundled_openssl)
    return None


def generate_security_material() -> tuple[bytes, bytes]:
    openssl = find_openssl()
    if openssl is None:
        raise RuntimeError("OpenSSL is required for the QUIC artifact smoke test")
    with tempfile.TemporaryDirectory(prefix="nnrp-smoke-cert-") as temporary_directory:
        directory = Path(temporary_directory)
        config = directory / "certificate.cnf"
        certificate_pem = directory / "certificate.pem"
        certificate_der = directory / "certificate.der"
        private_key_pem = directory / "private-key.pem"
        private_key_der = directory / "private-key.der"
        config.write_text(
            "[req]\n"
            "distinguished_name=subject\n"
            "x509_extensions=extensions\n"
            "prompt=no\n"
            "[subject]\n"
            "CN=localhost\n"
            "[extensions]\n"
            "subjectAltName=DNS:localhost\n",
            encoding="ascii",
        )
        subprocess.run(
            [
                openssl,
                "req",
                "-x509",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:P-256",
                "-nodes",
                "-days",
                "1",
                "-config",
                str(config),
                "-keyout",
                str(private_key_pem),
                "-out",
                str(certificate_pem),
            ],
            check=True,
            capture_output=True,
        )
        subprocess.run(
            [
                openssl,
                "x509",
                "-in",
                str(certificate_pem),
                "-outform",
                "DER",
                "-out",
                str(certificate_der),
            ],
            check=True,
            capture_output=True,
        )
        subprocess.run(
            [
                openssl,
                "pkcs8",
                "-topk8",
                "-nocrypt",
                "-in",
                str(private_key_pem),
                "-outform",
                "DER",
                "-out",
                str(private_key_der),
            ],
            check=True,
            capture_output=True,
        )
        return certificate_der.read_bytes(), private_key_der.read_bytes()


def security_configs(library: ctypes.CDLL, transport_id: int) -> tuple[NnrpHandle, NnrpHandle]:
    certificate_der, private_key_der = generate_security_material()
    server_name_owner, server_name = buffer_view(b"localhost")
    certificate_owner, certificate = buffer_view(certificate_der)
    key_owner, key = buffer_view(private_key_der)
    client = invalid_handle()
    server = invalid_handle()
    require_ok(
        library.nnrp_transport_client_security_config_create(
            NnrpTransportClientSecurityConfigRequest(
                transport_id, 0, server_name, certificate
            ),
            ctypes.byref(client),
        ),
        "create client security config",
    )
    require_ok(
        library.nnrp_transport_server_security_config_create(
            NnrpTransportServerSecurityConfigRequest(
                transport_id, 0, certificate, key
            ),
            ctypes.byref(server),
        ),
        "create server security config",
    )
    _ = (server_name_owner, certificate_owner, key_owner)
    return client, server


def open_request(transport_id: int, endpoint: bytes, config: NnrpHandle):
    endpoint_owner, endpoint_view = buffer_view(endpoint)
    return endpoint_owner, NnrpTransportOpenRequest(
        transport_id, 0, endpoint_view, config, 0, 10_000, 0
    )


def packet(frame_id: int) -> bytes:
    return struct.pack(
        "<4sBBBBIIIIIHHQ", b"NNRP", 1, 0, 0x20, 40, 0, 0, 0, 0, frame_id, 0, 0, 0
    )


def endpoint_for(scope: str) -> tuple[bytes, Path | None]:
    if scope == "tcp":
        return b"tcp://127.0.0.1:0", None
    if scope == "quic":
        return b"quic://127.0.0.1:0", None
    if scope == "websocket":
        return b"ws://127.0.0.1:0/nnrp", None
    if os.name == "nt":
        return f"npipe://nnrp-artifact-{uuid.uuid4().hex}".encode(), None
    path = Path(tempfile.gettempdir()) / f"nnrp-artifact-{uuid.uuid4().hex}.sock"
    return f"unix://{path}".encode(), path


def run_smoke_test_at_endpoint(library_path: Path, scope: str, endpoint: bytes) -> None:
    library = ctypes.CDLL(str(library_path.resolve()))
    configure_library(library)
    transport_id = TRANSPORT_IDS[scope]
    client_config = invalid_handle()
    server_config = invalid_handle()
    if scope == "quic":
        client_config, server_config = security_configs(library, transport_id)

    endpoint_owner, request = open_request(transport_id, endpoint, server_config)
    listener = invalid_handle()
    require_ok(library.nnrp_transport_listen(request, ctypes.byref(listener)), "listen")

    endpoint_buffer = invalid_handle()
    endpoint_view = NnrpBufferView()
    require_ok(
        library.nnrp_transport_listener_endpoint(
            listener, ctypes.byref(endpoint_buffer), ctypes.byref(endpoint_view)
        ),
        "listener endpoint",
    )
    resolved_endpoint = ctypes.string_at(endpoint_view.ptr, endpoint_view.len)
    require_ok(library.nnrp_buffer_release(endpoint_buffer), "release listener endpoint")

    accepted: queue.Queue = queue.Queue()

    def accept_connection() -> None:
        connection = invalid_handle()
        try:
            require_ok(
                library.nnrp_transport_accept(
                    NnrpTransportAcceptRequest(listener, 10_000, 0),
                    ctypes.byref(connection),
                ),
                "accept",
            )
            accepted.put(connection)
        except BaseException as error:
            accepted.put(error)

    accept_thread = threading.Thread(target=accept_connection, daemon=True)
    accept_thread.start()
    resolved_owner, connect_request = open_request(
        transport_id, resolved_endpoint, client_config
    )
    client = invalid_handle()
    require_ok(library.nnrp_transport_connect(connect_request, ctypes.byref(client)), "connect")
    accept_thread.join(10)
    if accept_thread.is_alive():
        raise RuntimeError("accept did not complete")
    server = accepted.get_nowait()
    if isinstance(server, BaseException):
        raise server

    first = packet(1)
    second = packet(2)
    first_owner, first_view = buffer_view(first)
    second_owner, second_view = buffer_view(second)
    frames = (NnrpBufferView * 2)(first_view, second_view)
    require_ok(
        library.nnrp_transport_write_batch(
            NnrpTransportWriteBatchRequest(client, frames, 2, 0)
        ),
        "write batch",
    )
    expected = struct.pack("<I", len(first)) + first + struct.pack("<I", len(second)) + second
    encoded_parts: list[bytes] = []
    received_frames = 0
    while received_frames < 2:
        batch = NnrpTransportFrameBatch()
        require_ok(
            library.nnrp_transport_read_batch(
                NnrpTransportReadBatchRequest(
                    server, 2 - received_frames, 10_000, 0
                ),
                ctypes.byref(batch),
            ),
            "read batch",
        )
        if batch.frame_count == 0:
            raise RuntimeError("dynamic library returned an empty packet batch")
        encoded_parts.append(ctypes.string_at(batch.payload.ptr, batch.payload.len))
        received_frames += batch.frame_count
        require_ok(library.nnrp_buffer_release(batch.payload_owner), "release packet batch")
    if received_frames != 2 or b"".join(encoded_parts) != expected:
        raise RuntimeError("dynamic library returned an invalid packet batch")

    for handle, name in ((client, "client"), (server, "server"), (listener, "listener")):
        require_ok(library.nnrp_transport_close(handle), f"close {name}")
    if scope == "quic":
        require_ok(library.nnrp_transport_close(client_config), "close client config")
        require_ok(library.nnrp_transport_close(server_config), "close server config")
    _ = (endpoint_owner, resolved_owner, first_owner, second_owner)


def run_smoke_test(library_path: Path, scope: str) -> None:
    endpoint, ipc_path = endpoint_for(scope)
    try:
        run_smoke_test_at_endpoint(library_path, scope, endpoint)
    finally:
        if ipc_path is not None:
            ipc_path.unlink(missing_ok=True)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Load a transport-scoped nnrp-ffi library and run a real packet loopback."
    )
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--transport-scope", choices=sorted(TRANSPORT_IDS), required=True)
    args = parser.parse_args()
    run_smoke_test(args.library, args.transport_scope)
    print(f"{args.transport_scope}: dynamic-library packet loopback passed")


if __name__ == "__main__":
    main()
