#!/usr/bin/env python3
import argparse
import json
import re
from pathlib import Path

NATIVE_TRANSPORTS = {
    "tcp": {
        "package": "nnrp-ffi-transport-tcp",
        "features": ["transport-tcp"],
        "provider_id": "nnrp.transport.tcp.native",
        "preference_rank": 2,
        "limitations": ["requires-tcp", "native-host-only"],
    },
    "quic": {
        "package": "nnrp-ffi-transport-quic",
        "features": ["transport-quic"],
        "provider_id": "nnrp.transport.quic.native",
        "preference_rank": 1,
        "limitations": ["requires-udp", "native-host-only"],
    },
    "ipc": {
        "package": "nnrp-ffi-transport-ipc",
        "features": ["transport-ipc"],
        "provider_id": "nnrp.transport.ipc.native",
        "preference_rank": 0,
        "limitations": ["local-host-only", "native-host-only"],
    },
    "websocket": {
        "package": "nnrp-ffi-transport-websocket",
        "features": ["transport-websocket"],
        "provider_id": "nnrp.transport.websocket.native",
        "preference_rank": 3,
        "limitations": ["requires-tcp", "native-host-only"],
    },
}

BROWSER_WASM_SCOPE = {
    "package": "nnrp-wasm",
    "artifact": "nnrp-wasm-browser",
    "scope": "browser",
    "slots": ["websocket"],
    "features": ["transport-websocket", "wasm-provider"],
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
        "openBrowserClientRole",
    ],
    "provider": {
        "id": "nnrp.transport.websocket.browser-wasm",
        "cost": {"model_id": 0, "units": "0"},
        "preference_rank": 3,
        "limits": {"max_frame_bytes": "67108864"},
        "limitations": ["requires-tcp", "browser-host-only"],
    },
}


def read_manifest(path: Path) -> dict:
    try:
        return json.loads(path.read_text())
    except FileNotFoundError as error:
        raise SystemExit(f"missing manifest: {path}") from error
    except json.JSONDecodeError as error:
        raise SystemExit(f"invalid manifest JSON: {path}: {error}") from error


def require_equal(actual, expected, label: str, manifest_path: Path) -> None:
    if actual != expected:
        raise SystemExit(
            f"{manifest_path}: expected {label} {expected!r}, found {actual!r}"
        )


def expected_native_provider(scope: str, os_name: str) -> dict:
    expected = NATIVE_TRANSPORTS[scope]
    limitations = list(expected["limitations"])
    if scope == "ipc":
        limitations.append(
            "windows-named-pipe" if os_name == "windows" else "unix-domain-socket"
        )
    return {
        "id": expected["provider_id"],
        "cost": {"model_id": 0, "units": "0"},
        "preference_rank": expected["preference_rank"],
        "limits": {"max_frame_bytes": "67108864"},
        "limitations": limitations,
    }


def inspect_native(native_dir: Path) -> None:
    manifest_paths = sorted(native_dir.glob("*/manifest.json"))
    if not manifest_paths:
        raise SystemExit(f"no native artifact manifests found under {native_dir}")

    platforms: dict[str, set[str]] = {}
    for manifest_path in manifest_paths:
        manifest = read_manifest(manifest_path)
        scope = manifest.get("transport_scope")
        if scope not in NATIVE_TRANSPORTS:
            raise SystemExit(
                f"{manifest_path}: release native artifacts must be transport-scoped; "
                f"found {scope!r}"
            )

        expected = NATIVE_TRANSPORTS[scope]
        require_equal(manifest.get("transport_name"), scope, "transport_name", manifest_path)
        require_equal(manifest.get("package"), expected["package"], "package", manifest_path)
        require_equal(manifest.get("transport_slots"), [scope], "transport_slots", manifest_path)
        require_equal(
            manifest.get("enabled_features"),
            expected["features"],
            "enabled_features",
            manifest_path,
        )
        require_equal(
            manifest.get("provider"),
            expected_native_provider(scope, manifest.get("os")),
            "provider",
            manifest_path,
        )

        platform_name = manifest_path.parent.name
        if platform_name.startswith(f"{scope}-"):
            platform_name = platform_name[len(scope) + 1 :]
        platforms.setdefault(platform_name, set()).add(scope)

    expected_scopes = set(NATIVE_TRANSPORTS)
    incomplete = {
        platform: sorted(expected_scopes - scopes)
        for platform, scopes in sorted(platforms.items())
        if scopes != expected_scopes
    }
    if incomplete:
        details = ", ".join(
            f"{platform} missing {missing}" for platform, missing in incomplete.items()
        )
        raise SystemExit(f"native artifact transport matrix is incomplete: {details}")


def inspect_wasm(wasm_dir: Path) -> None:
    manifest_paths = sorted(wasm_dir.glob("*/manifest.json"))
    if not manifest_paths:
        raise SystemExit(f"no WASM artifact manifests found under {wasm_dir}")

    for manifest_path in manifest_paths:
        manifest = read_manifest(manifest_path)
        require_equal(
            manifest.get("transport_scope"),
            BROWSER_WASM_SCOPE["scope"],
            "transport_scope",
            manifest_path,
        )
        require_equal(
            manifest.get("transport_name"),
            BROWSER_WASM_SCOPE["scope"],
            "transport_name",
            manifest_path,
        )
        require_equal(
            manifest.get("package"),
            BROWSER_WASM_SCOPE["package"],
            "package",
            manifest_path,
        )
        require_equal(
            manifest.get("artifact"),
            BROWSER_WASM_SCOPE["artifact"],
            "artifact",
            manifest_path,
        )
        require_equal(
            manifest.get("transport_slots"),
            BROWSER_WASM_SCOPE["slots"],
            "transport_slots",
            manifest_path,
        )
        require_equal(
            manifest.get("enabled_features"),
            BROWSER_WASM_SCOPE["features"],
            "enabled_features",
            manifest_path,
        )
        require_equal(
            manifest.get("exports"),
            BROWSER_WASM_SCOPE["exports"],
            "exports",
            manifest_path,
        )
        require_equal(
            manifest.get("provider"),
            BROWSER_WASM_SCOPE["provider"],
            "provider",
            manifest_path,
        )
        wasm_path = manifest_path.parent / manifest.get("wasm", "")
        glue_path = manifest_path.parent / manifest.get("glue", "")
        types_path = manifest_path.parent / manifest.get("types", "")
        for artifact_path, label in (
            (wasm_path, "wasm"),
            (glue_path, "glue"),
            (types_path, "types"),
        ):
            if not artifact_path.is_file():
                raise SystemExit(f"{manifest_path}: missing {label} artifact {artifact_path.name!r}")

        wasm = wasm_path.read_bytes()
        if not wasm.startswith(b"\0asm\x01\0\0\0"):
            raise SystemExit(f"{wasm_path}: not a WebAssembly 1 binary")
        glue = glue_path.read_text()
        declarations = types_path.read_text()
        if wasm_path.name not in glue:
            raise SystemExit(f"{glue_path}: does not load {wasm_path.name}")
        for export_name in BROWSER_WASM_SCOPE["exports"]:
            pattern = rf"export\s+function\s+{re.escape(export_name)}\s*\("
            if re.search(pattern, glue) is None:
                raise SystemExit(f"{glue_path}: missing callable export {export_name}")
            if re.search(pattern, declarations) is None:
                raise SystemExit(f"{types_path}: missing declaration for {export_name}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Reject release artifacts that collapse transport ownership boundaries."
    )
    parser.add_argument("--native-dir", type=Path)
    parser.add_argument("--wasm-dir", type=Path)
    args = parser.parse_args()

    if args.native_dir is None and args.wasm_dir is None:
        raise SystemExit("provide --native-dir, --wasm-dir, or both")

    if args.native_dir is not None:
        inspect_native(args.native_dir)
    if args.wasm_dir is not None:
        inspect_wasm(args.wasm_dir)


if __name__ == "__main__":
    main()
