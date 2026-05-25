#!/usr/bin/env python3
import argparse
import json
import os
import platform
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_EXPORTS = [
    "nnrp_current_protocol_version",
    "nnrp_runtime_capabilities",
    "nnrp_connection_bootstrap",
    "nnrp_client_connect",
    "nnrp_session_open",
    "nnrp_client_open_session",
    "nnrp_submit",
    "nnrp_client_submit",
    "nnrp_session_close",
    "nnrp_client_close",
    "nnrp_client_cancel",
    "nnrp_client_await_event",
    "nnrp_server_bind",
    "nnrp_server_accept",
    "nnrp_server_receive_submit",
    "nnrp_server_send_result",
    "nnrp_server_send_flow_update",
    "nnrp_server_close",
    "nnrp_control",
    "nnrp_poll_empty",
    "nnrp_dispatch_event",
]

TARGETS = {
    "x86_64-unknown-linux-gnu": ("linux", "x86_64", "dynamic"),
    "i686-unknown-linux-gnu": ("linux", "x86", "dynamic"),
    "aarch64-unknown-linux-gnu": ("linux", "aarch64", "dynamic"),
    "armv7-unknown-linux-gnueabihf": ("linux", "armv7", "dynamic"),
    "x86_64-pc-windows-msvc": ("windows", "x86_64", "dynamic"),
    "i686-pc-windows-msvc": ("windows", "x86", "dynamic"),
    "aarch64-pc-windows-msvc": ("windows", "aarch64", "dynamic"),
    "x86_64-pc-windows-gnu": ("windows", "x86_64", "dynamic"),
    "i686-pc-windows-gnu": ("windows", "x86", "dynamic"),
    "aarch64-pc-windows-gnullvm": ("windows", "aarch64", "dynamic"),
    "x86_64-apple-darwin": ("macos", "x86_64", "dynamic"),
    "aarch64-apple-darwin": ("macos", "aarch64", "dynamic"),
    "aarch64-apple-ios": ("ios", "aarch64", "static"),
    "aarch64-apple-ios-sim": ("ios", "aarch64-sim", "static"),
    "x86_64-apple-ios": ("ios", "x86_64-sim", "static"),
    "aarch64-linux-android": ("android", "aarch64", "dynamic"),
    "armv7-linux-androideabi": ("android", "armv7", "dynamic"),
    "i686-linux-android": ("android", "x86", "dynamic"),
    "x86_64-linux-android": ("android", "x86_64", "dynamic"),
}


def host_os_name() -> str:
    value = platform.system().lower()
    if value == "darwin":
        return "macos"
    if value == "windows":
        return "windows"
    if value == "linux":
        return "linux"
    raise SystemExit(f"unsupported host OS: {platform.system()}")


def host_arch_name() -> str:
    value = platform.machine().lower()
    if value in {"amd64", "x86_64"}:
        return "x86_64"
    if value in {"arm64", "aarch64"}:
        return "aarch64"
    return value.replace(" ", "_")


def expected_library_name(os_name: str, library_kind: str) -> str:
    if library_kind == "static":
        return "libnnrp_ffi.a"
    if os_name == "windows":
        return "nnrp_ffi.dll"
    if os_name in {"macos", "ios"}:
        return "libnnrp_ffi.dylib"
    if os_name in {"linux", "android"}:
        return "libnnrp_ffi.so"
    raise SystemExit(f"unsupported artifact OS: {os_name}")


def build_library(release: bool, target: str | None) -> None:
    command = ["cargo", "build", "-p", "nnrp-ffi"]
    if target:
        command.extend(["--target", target])
    if release:
        command.append("--release")
    subprocess.run(command, cwd=ROOT, check=True)


def locate_library(os_name: str, library_kind: str, release: bool, target: str | None) -> Path:
    if target:
        profile_dir = ROOT / "target" / target / ("release" if release else "debug")
    else:
        profile_dir = ROOT / "target" / ("release" if release else "debug")
    library = profile_dir / expected_library_name(os_name, library_kind)
    if not library.is_file():
        raise SystemExit(f"expected native library was not found: {library}")
    return library


def list_exports(library: Path, os_name: str, library_kind: str) -> set[str]:
    if library_kind == "static":
        output = subprocess.check_output(["nm", "-g", str(library)], text=True)
        return parse_nm_exports(output)

    if os_name == "windows":
        dumpbin = find_dumpbin()
        if dumpbin is None:
            raise SystemExit("dumpbin is required to verify Windows DLL exports")
        output = subprocess.check_output([str(dumpbin), "/nologo", "/exports", str(library)], text=True)
        return {line.split()[-1] for line in output.splitlines() if "nnrp_" in line}

    if os_name == "macos":
        output = subprocess.check_output(["nm", "-gU", str(library)], text=True)
    else:
        output = subprocess.check_output(["nm", "-D", "--defined-only", str(library)], text=True)
    return parse_nm_exports(output)


def parse_nm_exports(output: str) -> set[str]:
    exports = set()
    for line in output.splitlines():
        symbol = line.split()[-1]
        if symbol.startswith("_nnrp_"):
            symbol = symbol[1:]
        if symbol.startswith("nnrp_"):
            exports.add(symbol)
    return exports


def find_dumpbin() -> Path | None:
    candidate = shutil.which("dumpbin")
    if candidate:
        return Path(candidate)

    vswhere = Path(os.environ.get("ProgramFiles(x86)", "")) / "Microsoft Visual Studio" / "Installer" / "vswhere.exe"
    if not vswhere.is_file():
        return None
    install_path = subprocess.check_output(
        [
            str(vswhere),
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ],
        text=True,
    ).strip()
    if not install_path:
        return None

    tools_root = Path(install_path) / "VC" / "Tools" / "MSVC"
    versions = sorted(tools_root.glob("*"), reverse=True)
    for version in versions:
        candidate = version / "bin" / "Hostx64" / "x64" / "dumpbin.exe"
        if candidate.is_file():
            return candidate
    return None


def verify_exports(library: Path, os_name: str, library_kind: str) -> None:
    exports = list_exports(library, os_name, library_kind)
    missing = [symbol for symbol in EXPECTED_EXPORTS if symbol not in exports]
    if missing:
        raise SystemExit(
            "native library is missing expected exports: " + ", ".join(missing)
        )


def copy_headers(package_dir: Path) -> list[str]:
    include_root = ROOT / "include" / "nnrp"
    package_include = package_dir / "include" / "nnrp"
    package_include.mkdir(parents=True, exist_ok=True)

    headers = sorted(include_root.glob("*.h"))
    for header in headers:
        shutil.copy2(header, package_include / header.name)

    # Keep the legacy root-level FFI header for early Preview3 consumers.
    shutil.copy2(include_root / "nnrp_ffi.h", package_dir / "nnrp_ffi.h")
    return [f"include/nnrp/{header.name}" for header in headers]


def copy_library_artifacts(library: Path, package_dir: Path, os_name: str) -> list[str]:
    copied = []
    shutil.copy2(library, package_dir / library.name)
    copied.append(library.name)
    if os_name == "windows":
        for sidecar_name in (
            f"{library.name}.lib",
            f"{library.name}.exp",
            f"{library.stem}.pdb",
        ):
            sidecar = library.with_name(sidecar_name)
            if sidecar.is_file():
                shutil.copy2(sidecar, package_dir / sidecar.name)
                copied.append(sidecar.name)
    return copied


def package_artifact(
    library: Path,
    os_name: str,
    arch_name: str,
    library_kind: str,
    target: str | None,
    package_name: str | None,
    out_dir: Path,
    release: bool,
) -> Path:
    resolved_package_name = package_name or f"{os_name}-{arch_name}"
    if target and package_name is None:
        resolved_package_name = f"{resolved_package_name}-{target}"
    package_dir = out_dir / resolved_package_name
    if package_dir.exists():
        shutil.rmtree(package_dir)
    package_dir.mkdir(parents=True, exist_ok=True)
    libraries = copy_library_artifacts(library, package_dir, os_name)
    headers = copy_headers(package_dir)
    manifest = {
        "package": "nnrp-ffi",
        "profile": "release" if release else "debug",
        "os": os_name,
        "arch": arch_name,
        "target": target,
        "library_kind": library_kind,
        "library": library.name,
        "libraries": libraries,
        "header": "include/nnrp/nnrp.h",
        "headers": headers,
        "legacy_header": "nnrp_ffi.h",
        "exports": EXPECTED_EXPORTS,
    }
    (package_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return package_dir


def main() -> None:
    parser = argparse.ArgumentParser(description="Build and package nnrp-ffi native artifacts.")
    parser.add_argument("--target", choices=sorted(TARGETS.keys()))
    parser.add_argument("--os", choices=["windows", "linux", "macos", "android", "ios"])
    parser.add_argument("--arch")
    parser.add_argument("--library-kind", choices=["dynamic", "static"])
    parser.add_argument("--package-name")
    parser.add_argument("--out", type=Path, default=ROOT / "artifacts" / "native")
    parser.add_argument("--debug", action="store_true", help="Use the debug target profile.")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--skip-symbol-check", action="store_true")
    args = parser.parse_args()

    target_os, target_arch, target_library_kind = TARGETS.get(
        args.target,
        (host_os_name(), host_arch_name(), "dynamic"),
    )
    os_name = args.os or target_os
    arch_name = args.arch or target_arch
    library_kind = args.library_kind or target_library_kind

    release = not args.debug
    if not args.skip_build:
        build_library(release, args.target)
    library = locate_library(os_name, library_kind, release, args.target)
    if not args.skip_symbol_check:
        verify_exports(library, os_name, library_kind)
    package_dir = package_artifact(
        library,
        os_name,
        arch_name,
        library_kind,
        args.target,
        args.package_name,
        args.out,
        release,
    )
    print(package_dir)


if __name__ == "__main__":
    main()
