#!/usr/bin/env python3
import argparse
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROTOCOL_VERSION = "NNRP/1"
WASM_ABI_VERSION = "1.0.0"
WASM_BINDGEN_VERSION = "0.2.122"
WASM_BINDGEN_OUT = ROOT / "target" / "wasm-bindgen" / "browser"
WASM_EXPORTS = [
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
]

TRANSPORT_SCOPES = {
    "browser": {
        "package": "nnrp-wasm",
        "artifact": "nnrp-wasm-browser",
        "directory": "nnrp-wasm-browser",
        "features": ["transport-websocket", "wasm-provider"],
        "slots": ["websocket"],
        "provider": {
            "id": "nnrp.transport.websocket.browser-wasm",
            "cost": {"model_id": 0, "units": "0"},
            "preference_rank": 3,
            "limits": {"max_frame_bytes": "67108864"},
            "limitations": ["requires-tcp", "browser-host-only"],
        },
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
    if WASM_BINDGEN_OUT.exists():
        shutil.rmtree(WASM_BINDGEN_OUT)
    WASM_BINDGEN_OUT.mkdir(parents=True)
    subprocess.run(
        [
            "wasm-bindgen",
            "--target",
            "web",
            "--out-dir",
            str(WASM_BINDGEN_OUT),
            "--out-name",
            "nnrp_wasm",
            str(
                ROOT
                / "target"
                / "wasm32-unknown-unknown"
                / "release"
                / "nnrp_wasm.wasm"
            ),
        ],
        cwd=ROOT,
        check=True,
    )


def package_wasm(out_dir: Path, transport_scope: str) -> Path:
    source_wasm = WASM_BINDGEN_OUT / "nnrp_wasm_bg.wasm"
    source_glue = WASM_BINDGEN_OUT / "nnrp_wasm.js"
    generated_dts = WASM_BINDGEN_OUT / "nnrp_wasm.d.ts"
    source_dts = ROOT / "crates" / "nnrp-wasm" / "pkg" / "nnrp_wasm.d.ts"
    if not source_wasm.is_file():
        raise SystemExit(
            f"missing wasm-bindgen artifact: {source_wasm}; "
            f"run wasm-bindgen-cli {WASM_BINDGEN_VERSION}"
        )
    if not source_glue.is_file():
        raise SystemExit(f"missing wasm-bindgen ESM glue: {source_glue}")
    if not generated_dts.is_file():
        raise SystemExit(f"missing wasm-bindgen declarations: {generated_dts}")
    if not source_dts.is_file():
        raise SystemExit(f"missing TypeScript declarations: {source_dts}")

    glue = source_glue.read_text()
    generated_declarations = generated_dts.read_text()
    for export_name in WASM_EXPORTS:
        declaration = f"export function {export_name}"
        if declaration not in glue or declaration not in generated_declarations:
            raise SystemExit(
                f"wasm-bindgen output is missing callable export {export_name}"
            )

    scope = TRANSPORT_SCOPES[transport_scope]
    package_dir = out_dir / scope["directory"]
    if package_dir.exists():
        shutil.rmtree(package_dir)
    package_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source_wasm, package_dir / "nnrp_wasm_bg.wasm")
    shutil.copy2(source_glue, package_dir / "nnrp_wasm.js")
    shutil.copy2(source_dts, package_dir / "nnrp_wasm.d.ts")
    manifest = {
        "package": scope["package"],
        "artifact": scope["artifact"],
        "transport_name": transport_scope,
        "transport_scope": transport_scope,
        "transport_slots": scope["slots"],
        "protocol_version": PROTOCOL_VERSION,
        "abi_version": WASM_ABI_VERSION,
        "enabled_features": scope["features"],
        "provider": scope["provider"],
        "wasm": "nnrp_wasm_bg.wasm",
        "glue": "nnrp_wasm.js",
        "types": "nnrp_wasm.d.ts",
        "owner": "nnrp-rs",
        "downstream_wrapper": "nnrp-js",
        "exports": WASM_EXPORTS,
    }
    (package_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    smoke_browser_package(package_dir)
    return package_dir


def smoke_browser_package(package_dir: Path) -> None:
    glue_url = (package_dir / "nnrp_wasm.js").resolve().as_uri()
    wasm_path = str((package_dir / "nnrp_wasm_bg.wasm").resolve())
    script = f"""
import fs from 'node:fs';
const module = await import({json.dumps(glue_url)});
const bytes = fs.readFileSync({json.dumps(wasm_path)});
await module.default({{ module_or_path: bytes }});
if (module.nnrp_wasm_protocol_major() !== 1 || module.nnrp_wasm_wire_format() !== 0) {{
  throw new Error('browser WASM protocol version mismatch');
}}
const packet = module.encodeWebSocketBinaryFrameJson(
  JSON.stringify({{ message_type: 1, session_id: 7 }}),
  new Uint8Array(),
  new Uint8Array([9]),
);
const decoded = JSON.parse(module.decodeWebSocketBinaryFrameJson(packet));
if (decoded.header.session_id !== 7 || decoded.body_len !== 1) {{
  throw new Error('browser WASM frame round-trip mismatch');
}}
"""
    subprocess.run(
        ["node", "--input-type=module", "--eval", script],
        cwd=ROOT,
        check=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Build and package nnrp-wasm primitives.")
    parser.add_argument("--out", type=Path, default=ROOT / "artifacts" / "wasm")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument(
        "--transport-scope",
        action="append",
        choices=sorted(TRANSPORT_SCOPES.keys()),
        help="Transport scope to package. Repeat to package multiple scopes. Defaults to browser.",
    )
    args = parser.parse_args()

    for transport_scope in args.transport_scope or ["browser"]:
        if not args.skip_build:
            build_wasm(transport_scope)
        print(package_wasm(args.out, transport_scope))


if __name__ == "__main__":
    main()
