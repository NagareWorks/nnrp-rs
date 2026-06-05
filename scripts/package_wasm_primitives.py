#!/usr/bin/env python3
import argparse
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

TRANSPORT_SCOPES = {
    "all": {
        "package": "nnrp-wasm",
        "artifact": "nnrp-wasm-primitives",
        "directory": "nnrp-wasm-primitives",
        "features": ["transport-tcp", "transport-quic", "wasm-provider"],
        "slots": ["tcp", "quic"],
    },
    "tcp": {
        "package": "nnrp-wasm-transport-tcp",
        "artifact": "nnrp-wasm-transport-tcp",
        "directory": "nnrp-wasm-transport-tcp",
        "features": ["transport-tcp", "wasm-provider"],
        "slots": ["tcp"],
    },
    "quic": {
        "package": "nnrp-wasm-transport-quic",
        "artifact": "nnrp-wasm-transport-quic",
        "directory": "nnrp-wasm-transport-quic",
        "features": ["transport-quic", "wasm-provider"],
        "slots": ["quic"],
    },
}

def build_wasm(transport_scope: str) -> None:
    subprocess.run(
        [
            "cargo",
            "build",
            "-p",
            "nnrp-wasm",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--no-default-features",
            "--features",
            ",".join(TRANSPORT_SCOPES[transport_scope]["features"]),
        ],
        cwd=ROOT,
        check=True,
    )


def package_wasm(out_dir: Path, transport_scope: str) -> Path:
    source_wasm = ROOT / "target" / "wasm32-unknown-unknown" / "release" / "nnrp_wasm.wasm"
    source_dts = ROOT / "crates" / "nnrp-wasm" / "pkg" / "nnrp_wasm.d.ts"
    if not source_wasm.is_file():
        raise SystemExit(f"missing wasm artifact: {source_wasm}")
    if not source_dts.is_file():
        raise SystemExit(f"missing TypeScript declarations: {source_dts}")

    scope = TRANSPORT_SCOPES[transport_scope]
    package_dir = out_dir / scope["directory"]
    if package_dir.exists():
        shutil.rmtree(package_dir)
    package_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source_wasm, package_dir / "nnrp_wasm.wasm")
    shutil.copy2(source_dts, package_dir / "nnrp_wasm.d.ts")
    manifest = {
        "package": scope["package"],
        "artifact": scope["artifact"],
        "transport_scope": transport_scope,
        "transport_slots": scope["slots"],
        "wasm": "nnrp_wasm.wasm",
        "types": "nnrp_wasm.d.ts",
        "owner": "nnrp-rs",
        "downstream_wrapper": "nnrp-js",
        "exports": [
            "nnrp_wasm_protocol_major",
            "nnrp_wasm_wire_format",
            "selectTransportWithProbeJson",
            "scoreProviderProbeJson",
        ],
    }
    (package_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return package_dir


def main() -> None:
    parser = argparse.ArgumentParser(description="Build and package nnrp-wasm primitives.")
    parser.add_argument("--out", type=Path, default=ROOT / "artifacts" / "wasm")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument(
        "--transport-scope",
        action="append",
        choices=sorted(TRANSPORT_SCOPES.keys()),
        help="Transport scope to package. Repeat to package multiple scopes. Defaults to all.",
    )
    args = parser.parse_args()

    for transport_scope in args.transport_scope or ["all"]:
        if not args.skip_build:
            build_wasm(transport_scope)
        print(package_wasm(args.out, transport_scope))


if __name__ == "__main__":
    main()
