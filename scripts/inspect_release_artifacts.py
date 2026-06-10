#!/usr/bin/env python3
import argparse
import json
from pathlib import Path

NATIVE_TRANSPORTS = {
    "tcp": {
        "package": "nnrp-ffi-transport-tcp",
        "features": ["transport-tcp"],
    },
    "quic": {
        "package": "nnrp-ffi-transport-quic",
        "features": ["transport-quic"],
    },
    "ipc": {
        "package": "nnrp-ffi-transport-ipc",
        "features": ["transport-ipc"],
    },
    "websocket": {
        "package": "nnrp-ffi-transport-websocket",
        "features": ["transport-websocket"],
    },
}

BROWSER_WASM_SCOPE = {
    "package": "nnrp-wasm",
    "artifact": "nnrp-wasm-browser",
    "scope": "browser",
    "slots": ["websocket"],
    "features": ["transport-websocket", "wasm-provider"],
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
