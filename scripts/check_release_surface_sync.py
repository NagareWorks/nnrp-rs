#!/usr/bin/env python3
import importlib.util
import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NATIVE_TRANSPORTS = {
    "tcp": {
        "package": "nnrp-ffi-transport-tcp",
        "features": ["transport-tcp"],
        "slots": ["tcp"],
        "provider_id": "nnrp.transport.tcp.native",
        "preference_rank": 2,
        "limitations": ["requires-tcp", "native-host-only"],
    },
    "quic": {
        "package": "nnrp-ffi-transport-quic",
        "features": ["transport-quic"],
        "slots": ["quic"],
        "provider_id": "nnrp.transport.quic.native",
        "preference_rank": 1,
        "limitations": ["requires-udp", "native-host-only"],
    },
    "ipc": {
        "package": "nnrp-ffi-transport-ipc",
        "features": ["transport-ipc"],
        "slots": ["ipc"],
        "provider_id": "nnrp.transport.ipc.native",
        "preference_rank": 0,
        "limitations": ["local-host-only", "native-host-only"],
    },
    "websocket": {
        "package": "nnrp-ffi-transport-websocket",
        "features": ["transport-websocket"],
        "slots": ["websocket"],
        "provider_id": "nnrp.transport.websocket.native",
        "preference_rank": 3,
        "limitations": ["requires-tcp", "native-host-only"],
    },
}
BROWSER_WASM_SCOPE = {
    "package": "nnrp-wasm",
    "artifact": "nnrp-wasm-browser",
    "scope": "browser",
    "features": ["transport-websocket", "wasm-provider"],
    "slots": ["websocket"],
    "exports": [
        "nnrp_wasm_protocol_major",
        "nnrp_wasm_wire_format",
        "selectTransportWithProbeJson",
        "summarizeProviderProbeJson",
        "encodeWebSocketBinaryFrameJson",
        "decodeWebSocketBinaryFrameJson",
        "decodeWebSocketBinaryFrameBatchJson",
        "encodeRuntimeControlMetadataJson",
        "decodeRuntimeControlMetadataJson",
        "encodeRuntimeObjectMetadataJson",
        "decodeRuntimeObjectMetadataJson",
    ],
    "provider": {
        "id": "nnrp.transport.websocket.browser-wasm",
        "cost": {"model_id": 0, "units": "0"},
        "preference_rank": 3,
        "limits": {"max_frame_bytes": "67108864"},
        "limitations": ["requires-tcp", "browser-host-only"],
    },
}


def load_script(path: Path):
    spec = importlib.util.spec_from_file_location(path.stem, path)
    if spec is None or spec.loader is None:
        raise SystemExit(f"failed to load script: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def require_equal(actual, expected, label: str) -> None:
    if actual != expected:
        raise SystemExit(f"{label}: expected {expected!r}, found {actual!r}")


def read_text(relative: str) -> str:
    return (ROOT / relative).read_text()


def rust_const_u16(source: str, name: str) -> int:
    pattern = rf"pub const {re.escape(name)}: u16 = (\d+);"
    match = re.search(pattern, source)
    if match is None:
        raise SystemExit(f"missing Rust u16 constant {name}")
    return int(match.group(1))


def rust_const_u32(source: str, name: str) -> int:
    pattern = rf"pub const {re.escape(name)}: u32 = (0x[0-9a-fA-F_]+|\d+);"
    match = re.search(pattern, source)
    if match is None:
        raise SystemExit(f"missing Rust u32 constant {name}")
    return int(match.group(1).replace("_", ""), 0)


def header_define_int(source: str, name: str) -> int:
    pattern = rf"#define {re.escape(name)} (0x[0-9a-fA-F]+|\d+)u?"
    match = re.search(pattern, source)
    if match is None:
        raise SystemExit(f"missing header define {name}")
    return int(match.group(1), 0)


def header_define_string(source: str, name: str) -> str:
    pattern = rf'#define {re.escape(name)} "([^"]+)"'
    match = re.search(pattern, source)
    if match is None:
        raise SystemExit(f"missing header string define {name}")
    return match.group(1)


def declared_ffi_functions(header: str) -> set[str]:
    return set(re.findall(r"\bNnrpFfiStatus\s+(nnrp_[a-zA-Z0-9_]+)\s*\(", header)) | set(
        re.findall(r"\bNnrpProtocolVersion\s+(nnrp_[a-zA-Z0-9_]+)\s*\(", header)
    ) | set(re.findall(r"\bNnrpRuntimeCapabilities\s+(nnrp_[a-zA-Z0-9_]+)\s*\(", header))


def declared_wasm_functions(typescript: str) -> set[str]:
    return set(re.findall(r"\bexport function\s+([a-zA-Z0-9_]+)\s*\(", typescript))


def check_abi_version() -> None:
    rust = read_text("crates/nnrp-ffi/src/lib.rs")
    header = read_text("include/nnrp/nnrp_ffi.h")
    native = load_script(ROOT / "scripts" / "package_native_artifacts.py")

    rust_version = (
        rust_const_u16(rust, "NNRP_FFI_ABI_MAJOR"),
        rust_const_u16(rust, "NNRP_FFI_ABI_MINOR"),
        rust_const_u16(rust, "NNRP_FFI_ABI_PATCH"),
    )
    header_version = (
        header_define_int(header, "NNRP_FFI_ABI_MAJOR"),
        header_define_int(header, "NNRP_FFI_ABI_MINOR"),
        header_define_int(header, "NNRP_FFI_ABI_PATCH"),
    )
    script_version = tuple(int(part) for part in native.FFI_ABI_VERSION.split("."))

    require_equal(header_version, rust_version, "include/nnrp/nnrp_ffi.h ABI version")
    require_equal(script_version, rust_version, "scripts/package_native_artifacts.py ABI version")


def check_sdk_version_header() -> None:
    workspace = tomllib.loads(read_text("Cargo.toml"))
    header = read_text("include/nnrp/nnrp_version.h")
    require_equal(
        header_define_string(header, "NNRP_SDK_VERSION"),
        workspace["workspace"]["package"]["version"],
        "include/nnrp/nnrp_version.h SDK version",
    )


def check_transport_slots() -> None:
    rust = read_text("crates/nnrp-ffi/src/lib.rs")
    header = read_text("include/nnrp/nnrp_ffi.h")
    for name in ("QUIC", "TCP", "IPC", "WEBSOCKET"):
        const_name = f"NNRP_TRANSPORT_SLOT_{name}"
        require_equal(
            header_define_int(header, const_name),
            rust_const_u32(rust, const_name),
            f"{const_name} header/Rust value",
        )


def check_native_manifests() -> None:
    native = load_script(ROOT / "scripts" / "package_native_artifacts.py")
    inspector = load_script(ROOT / "scripts" / "inspect_release_artifacts.py")

    for scope, expected in NATIVE_TRANSPORTS.items():
        packaged = native.TRANSPORT_SCOPES.get(scope)
        inspected = inspector.NATIVE_TRANSPORTS.get(scope)
        if packaged is None:
            raise SystemExit(f"missing native packaging scope {scope}")
        if inspected is None:
            raise SystemExit(f"missing native inspection scope {scope}")
        require_equal(packaged["package"], expected["package"], f"{scope} package name")
        require_equal(packaged["features"], expected["features"], f"{scope} package features")
        require_equal(packaged["slots"], expected["slots"], f"{scope} package slots")
        require_equal(packaged["provider_id"], expected["provider_id"], f"{scope} provider id")
        require_equal(
            packaged["preference_rank"],
            expected["preference_rank"],
            f"{scope} preference rank",
        )
        require_equal(
            packaged["limitations"], expected["limitations"], f"{scope} limitations"
        )
        require_equal(inspected["package"], expected["package"], f"{scope} inspector package name")
        require_equal(inspected["features"], expected["features"], f"{scope} inspector features")
        require_equal(
            inspected["limitations"],
            expected["limitations"],
            f"{scope} inspector limitations",
        )
        for os_name in ("linux", "windows"):
            require_equal(
                native.provider_manifest(scope, os_name),
                inspector.expected_native_provider(scope, os_name),
                f"{scope} {os_name} provider metadata",
            )

    require_equal(set(native.TRANSPORT_SCOPES), set(NATIVE_TRANSPORTS), "native scopes")


def check_wasm_manifest() -> None:
    wasm = load_script(ROOT / "scripts" / "package_wasm_primitives.py")
    inspector = load_script(ROOT / "scripts" / "inspect_release_artifacts.py")
    declarations = declared_wasm_functions(read_text("crates/nnrp-wasm/pkg/nnrp_wasm.d.ts"))

    packaged = wasm.TRANSPORT_SCOPES.get(BROWSER_WASM_SCOPE["scope"])
    if packaged is None:
        raise SystemExit("missing browser WASM packaging scope")
    require_equal(packaged["package"], BROWSER_WASM_SCOPE["package"], "browser WASM package")
    require_equal(packaged["artifact"], BROWSER_WASM_SCOPE["artifact"], "browser WASM artifact")
    require_equal(packaged["features"], BROWSER_WASM_SCOPE["features"], "browser WASM features")
    require_equal(packaged["slots"], BROWSER_WASM_SCOPE["slots"], "browser WASM slots")
    require_equal(
        packaged["provider"], BROWSER_WASM_SCOPE["provider"], "browser WASM provider"
    )
    require_equal(wasm.WASM_EXPORTS, BROWSER_WASM_SCOPE["exports"], "browser WASM exports")
    require_equal(
        inspector.BROWSER_WASM_SCOPE["package"],
        BROWSER_WASM_SCOPE["package"],
        "browser WASM inspector package",
    )
    require_equal(
        inspector.BROWSER_WASM_SCOPE["artifact"],
        BROWSER_WASM_SCOPE["artifact"],
        "browser WASM inspector artifact",
    )
    require_equal(
        inspector.BROWSER_WASM_SCOPE["features"],
        BROWSER_WASM_SCOPE["features"],
        "browser WASM inspector features",
    )
    require_equal(
        inspector.BROWSER_WASM_SCOPE["slots"],
        BROWSER_WASM_SCOPE["slots"],
        "browser WASM inspector slots",
    )
    require_equal(
        inspector.BROWSER_WASM_SCOPE["exports"],
        BROWSER_WASM_SCOPE["exports"],
        "browser WASM inspector exports",
    )
    require_equal(
        inspector.BROWSER_WASM_SCOPE["provider"],
        BROWSER_WASM_SCOPE["provider"],
        "browser WASM inspector provider",
    )
    missing = sorted(set(BROWSER_WASM_SCOPE["exports"]) - declarations)
    if missing:
        raise SystemExit(
            "browser WASM manifest expects functions missing from TypeScript declarations: "
            + ", ".join(missing)
        )


def check_expected_exports_are_declared() -> None:
    native = load_script(ROOT / "scripts" / "package_native_artifacts.py")
    header = read_text("include/nnrp/nnrp_ffi.h")
    declarations = declared_ffi_functions(header)
    missing = sorted(set(native.EXPECTED_EXPORTS) - declarations)
    if missing:
        raise SystemExit(
            "native export verification expects functions missing from header: "
            + ", ".join(missing)
        )


def check_benchmark_exports_are_isolated() -> None:
    native = load_script(ROOT / "scripts" / "package_native_artifacts.py")
    production_declarations = declared_ffi_functions(read_text("include/nnrp/nnrp_ffi.h"))
    benchmark_declarations = declared_ffi_functions(
        read_text("benchmarks/include/nnrp/nnrp_ffi_benchmark.h")
    )
    rust = read_text("crates/nnrp-ffi/src/lib.rs")

    require_equal(
        benchmark_declarations,
        set(native.BENCHMARK_ONLY_EXPORTS),
        "benchmark-only FFI declarations",
    )
    leaked = sorted(set(native.FORBIDDEN_EXPORTS) & production_declarations)
    if leaked:
        raise SystemExit(
            "production FFI header declares non-production functions: " + ", ".join(leaked)
        )
    missing_rust = sorted(
        symbol
        for symbol in native.BENCHMARK_ONLY_EXPORTS
        if re.search(rf"\bfn\s+{re.escape(symbol)}\s*\(", rust) is None
    )
    if missing_rust:
        raise SystemExit(
            "benchmark FFI header declares functions missing from Rust: "
            + ", ".join(missing_rust)
        )


def main() -> None:
    check_abi_version()
    check_sdk_version_header()
    check_transport_slots()
    check_native_manifests()
    check_wasm_manifest()
    check_expected_exports_are_declared()
    check_benchmark_exports_are_isolated()


if __name__ == "__main__":
    main()
