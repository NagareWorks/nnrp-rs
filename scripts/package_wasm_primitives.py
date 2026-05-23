#!/usr/bin/env python3
import argparse
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def build_wasm() -> None:
    subprocess.run(
        [
            "cargo",
            "build",
            "-p",
            "nnrp-wasm",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ],
        cwd=ROOT,
        check=True,
    )


def package_wasm(out_dir: Path) -> Path:
    source_wasm = ROOT / "target" / "wasm32-unknown-unknown" / "release" / "nnrp_wasm.wasm"
    source_dts = ROOT / "crates" / "nnrp-wasm" / "pkg" / "nnrp_wasm.d.ts"
    if not source_wasm.is_file():
        raise SystemExit(f"missing wasm artifact: {source_wasm}")
    if not source_dts.is_file():
        raise SystemExit(f"missing TypeScript declarations: {source_dts}")

    package_dir = out_dir / "nnrp-wasm-primitives"
    package_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source_wasm, package_dir / "nnrp_wasm.wasm")
    shutil.copy2(source_dts, package_dir / "nnrp_wasm.d.ts")
    manifest = {
        "package": "nnrp-wasm",
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
    args = parser.parse_args()

    if not args.skip_build:
        build_wasm()
    print(package_wasm(args.out))


if __name__ == "__main__":
    main()
