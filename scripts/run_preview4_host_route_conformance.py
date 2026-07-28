#!/usr/bin/env python3
"""Run the public Preview4 host-route suite against the current Rust tree."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from typing import Iterable


RUST_PACKAGES = (
    "nnrp-conformance",
    "nnrp-core",
    "nnrp-runtime",
    "nnrp-transport-ipc",
    "nnrp-transport-provider",
    "nnrp-transport-quic",
    "nnrp-transport-tcp",
    "nnrp-transport-websocket",
)
HOST_ROUTE_CASES = "wire-conformance/nnrp-1-preview4/cases/host-route-e2e.json"
BROWSER_CASE = "wire.host-route.browser.wss"
UNINSTALLED_CASE = "wire.host-route.client.known-uninstalled"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--suite-root",
        type=Path,
        help="nnrp-conformance checkout (defaults to the environment or sibling repo)",
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        default=Path("target/wire-conformance/preview4-host-route"),
    )
    return parser.parse_args()


def locate_suite_root(repo_root: Path, explicit: Path | None) -> Path:
    candidates = [
        explicit,
        Path(os.environ["NNRP_CONFORMANCE_SUITE_REPO"])
        if os.environ.get("NNRP_CONFORMANCE_SUITE_REPO")
        else None,
        repo_root / "nnrp-conformance-action",
        repo_root.parent / "nnrp-conformance",
    ]
    for candidate in candidates:
        if candidate is not None and (candidate / HOST_ROUTE_CASES).is_file():
            return candidate.resolve()
    raise RuntimeError(
        "nnrp-conformance checkout is required; pass --suite-root, set "
        "NNRP_CONFORMANCE_SUITE_REPO, or checkout it beside nnrp-rs"
    )


def patch_arguments(repo_root: Path) -> list[str]:
    source = "https://github.com/NagareWorks/nnrp-rs.git"
    arguments: list[str] = []
    for package in RUST_PACKAGES:
        path = (repo_root / "crates" / package).resolve().as_posix()
        arguments.extend(
            ["--config", f'patch."{source}".{package}.path="{path}"']
        )
    return arguments


def run(command: Iterable[str], cwd: Path) -> None:
    rendered = " ".join(str(part) for part in command)
    print(f"+ {rendered}", flush=True)
    subprocess.run([str(part) for part in command], cwd=cwd, check=True)


def read_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as source:
        return json.load(source)


def public_host_route_ids(suite_root: Path) -> set[str]:
    manifest = read_json(suite_root / HOST_ROUTE_CASES)
    scenarios = manifest.get("scenarios")
    if not isinstance(scenarios, list) or not scenarios:
        raise RuntimeError("public host-route scenario manifest is empty")
    identifiers = {scenario.get("id") for scenario in scenarios}
    if None in identifiers or len(identifiers) != len(scenarios):
        raise RuntimeError("public host-route scenario ids must be non-empty and unique")
    return identifiers


def assert_browser_contract(suite_root: Path) -> None:
    manifest = read_json(suite_root / HOST_ROUTE_CASES)
    scenarios = manifest["scenarios"]
    browser = next(
        (scenario for scenario in scenarios if scenario.get("id") == BROWSER_CASE), None
    )
    if browser is None:
        raise RuntimeError(f"public host-route suite omitted {BROWSER_CASE}")
    fixture = browser.get("host_route", {})
    routes = fixture.get("routes", [])
    expected = {
        "platform": "browser",
        "application_endpoint": "nnrps://host-route.test",
        "transport": "websocket",
        "provider_id": "nnrp.transport.websocket.browser-wasm",
        "security_mode": "browser_host",
    }
    actual = {
        "platform": fixture.get("platform"),
        "application_endpoint": fixture.get("application_endpoint"),
        "transport": routes[0].get("transport") if len(routes) == 1 else None,
        "provider_id": routes[0].get("provider_id") if len(routes) == 1 else None,
        "security_mode": (
            routes[0].get("security", {}).get("mode") if len(routes) == 1 else None
        ),
    }
    if actual != expected:
        raise RuntimeError(
            f"public browser WSS host-route contract drifted: expected={expected}, actual={actual}"
        )


def result_ids(path: Path) -> set[str]:
    report = read_json(path)
    results = report.get("results")
    if not isinstance(results, list) or not results:
        raise RuntimeError(f"host-route result report is empty: {path}")
    failed = [
        result.get("id")
        for result in results
        if result.get("outcome") != "passed"
    ]
    if failed:
        raise RuntimeError(f"host-route scenarios did not pass: {', '.join(failed)}")
    return {result["id"] for result in results}


def assert_native_coverage(
    suite_root: Path, native_results: Path, uninstalled_results: Path
) -> None:
    assert_browser_contract(suite_root)
    public_ids = public_host_route_ids(suite_root)
    executed = result_ids(native_results) | result_ids(uninstalled_results)
    expected_native = public_ids - {BROWSER_CASE}
    if executed != expected_native:
        missing = sorted(expected_native - executed)
        unexpected = sorted(executed - expected_native)
        raise RuntimeError(
            "native host-route coverage drifted: "
            f"missing={missing or 'none'}, unexpected={unexpected or 'none'}"
        )
    if result_ids(uninstalled_results) != {UNINSTALLED_CASE}:
        raise RuntimeError("uninstalled-provider target must execute exactly its public case")
    print(
        f"host-route E2E passed {len(executed)} native scenarios; "
        f"{BROWSER_CASE} contract is pinned here and executed by the WASM browser-role job",
        flush=True,
    )


def copy_suite(source: Path, destination: Path) -> None:
    if destination.exists():
        shutil.rmtree(destination)
    shutil.copytree(
        source,
        destination,
        ignore=shutil.ignore_patterns(".git", "target", "__pycache__"),
    )
    lockfile = destination / "Cargo.lock"
    if lockfile.exists():
        lockfile.unlink()


def verify_local_rust_dependencies(
    cargo: str, suite_copy: Path, repo_root: Path, cargo_args: list[str]
) -> None:
    command = [
        cargo,
        "metadata",
        "--manifest-path",
        str(suite_copy / "Cargo.toml"),
        "--format-version",
        "1",
        *cargo_args,
    ]
    metadata = subprocess.run(
        command,
        cwd=suite_copy,
        check=True,
        capture_output=True,
        text=True,
    )
    packages = {package["name"]: package for package in json.loads(metadata.stdout)["packages"]}
    for name in RUST_PACKAGES:
        package = packages.get(name)
        expected = (repo_root / "crates" / name / "Cargo.toml").resolve()
        actual = Path(package["manifest_path"]).resolve() if package else None
        if actual != expected:
            raise RuntimeError(
                f"conformance target did not resolve {name} from the current Rust tree: "
                f"expected {expected}, got {actual}"
            )


def execute_profile(
    runner: Path,
    manifest_writer: Path,
    host_target: Path,
    suite_copy: Path,
    output_root: Path,
    profile: str,
) -> Path:
    profile_root = output_root / profile
    profile_root.mkdir(parents=True, exist_ok=True)
    target_manifest = profile_root / "target.json"
    plan = profile_root / "plan.json"
    results = profile_root / "results.json"
    evidence = profile_root / "evidence"

    run(
        [manifest_writer, "--manifest", target_manifest, "--profile", profile],
        suite_copy,
    )
    run(
        [
            runner,
            "wire-plan",
            "--suite",
            suite_copy / "wire-conformance/nnrp-1-preview4/manifest.json",
            "--target",
            target_manifest,
            "--scenarios",
            suite_copy / HOST_ROUTE_CASES,
            "--output",
            plan,
            "--results-path",
            results,
            "--evidence-dir",
            evidence,
        ],
        suite_copy,
    )
    run(
        [
            runner,
            "wire-run",
            "--plan",
            plan,
            "--target",
            target_manifest,
            "--output",
            results,
            "--host-route-target",
            host_target,
        ],
        suite_copy,
    )
    run(
        [runner, "validate-wire-results", "--plan", plan, "--results", results],
        suite_copy,
    )
    return results


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parents[1]
    suite_root = locate_suite_root(repo_root, args.suite_root)
    output_root = (repo_root / args.output_root).resolve()
    suite_copy = output_root / "suite-src"
    cargo_target = output_root / "cargo-target"
    copy_suite(suite_root, suite_copy)

    cargo = shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("cargo is required to run host-route conformance")
    cargo_args = patch_arguments(repo_root)
    verify_local_rust_dependencies(cargo, suite_copy, repo_root, cargo_args)
    run(
        [
            cargo,
            "build",
            "--manifest-path",
            suite_copy / "Cargo.toml",
            "--target-dir",
            cargo_target,
            "-p",
            "nnrp-conformance-runner",
            "--bin",
            "nnrp-conformance-runner",
            "--bin",
            "nnrp-wire-reference-target",
            "--bin",
            "nnrp-wire-host-route-reference-target",
            *cargo_args,
        ],
        suite_copy,
    )

    suffix = ".exe" if os.name == "nt" else ""
    runner = cargo_target / "debug" / f"nnrp-conformance-runner{suffix}"
    manifest_writer = cargo_target / "debug" / f"nnrp-wire-reference-target{suffix}"
    host_target = (
        cargo_target / "debug" / f"nnrp-wire-host-route-reference-target{suffix}"
    )
    native_results = execute_profile(
        runner,
        manifest_writer,
        host_target,
        suite_copy,
        output_root,
        "host-route-only",
    )
    uninstalled_results = execute_profile(
        runner,
        manifest_writer,
        host_target,
        suite_copy,
        output_root,
        "uninstalled-quic",
    )
    assert_native_coverage(suite_copy, native_results, uninstalled_results)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"host-route conformance failed: {error}", file=sys.stderr)
        sys.exit(1)
