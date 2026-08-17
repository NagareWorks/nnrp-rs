#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import re
import subprocess
import tomllib
import urllib.error
import urllib.request


SHA_PATTERN = re.compile(r"[0-9a-f]{40}")


def normalize_commit(value: str, label: str) -> str:
    commit = value.strip().lower()
    if SHA_PATTERN.fullmatch(commit) is None:
        raise ValueError(f"{label} must be a full 40-character Git commit SHA")
    return commit


def git_commit(ref: str) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"{ref}^{{commit}}"],
        check=True,
        capture_output=True,
        text=True,
    )
    return normalize_commit(result.stdout, f"Git ref {ref!r}")


def verify_tag_identity(tag: str, source_commit: str, allow_missing: bool) -> bool:
    source = normalize_commit(source_commit, "source commit")
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        if allow_missing:
            return False
        raise RuntimeError(f"release tag {tag!r} does not exist")

    tagged = normalize_commit(result.stdout, f"release tag {tag!r}")
    if tagged != source:
        raise RuntimeError(
            f"release tag {tag!r} points to {tagged}, not source commit {source}"
        )
    return True


def workspace_crate_names(root: pathlib.Path) -> list[str]:
    workspace = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    names = []
    for member in workspace["workspace"]["members"]:
        manifest = tomllib.loads((root / member / "Cargo.toml").read_text(encoding="utf-8"))
        names.append(manifest["package"]["name"])
    return sorted(names)


def crate_version_exists(crate: str, version: str) -> bool:
    request = urllib.request.Request(
        f"https://crates.io/api/v1/crates/{crate}/{version}",
        headers={"User-Agent": "nnrp-rs-release-identity"},
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return response.status == 200
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return False
        raise


def verify_registry_identity(
    root: pathlib.Path,
    version: str,
    tag: str,
    source_commit: str,
) -> list[str]:
    published = [
        crate for crate in workspace_crate_names(root) if crate_version_exists(crate, version)
    ]
    if published:
        try:
            verify_tag_identity(tag, source_commit, allow_missing=False)
        except RuntimeError as error:
            raise RuntimeError(
                f"crates.io already contains {version} for {', '.join(published)}, "
                "but its immutable release tag does not identify this source"
            ) from error
    return published


def file_sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def build_bom(
    artifacts_dir: pathlib.Path,
    output: pathlib.Path,
    version: str,
    tag: str,
    source_commit: str,
    conformance_source_commit: str,
    doc_source_commit: str,
) -> dict:
    source = normalize_commit(source_commit, "source commit")
    conformance = normalize_commit(conformance_source_commit, "Conformance source commit")
    doc = normalize_commit(doc_source_commit, "documentation source commit")
    output_resolved = output.resolve()
    artifacts = []
    for path in sorted(artifacts_dir.rglob("*")):
        if not path.is_file() or path.resolve() == output_resolved or path.name == "SHA256SUMS":
            continue
        artifacts.append(
            {
                "path": path.relative_to(artifacts_dir).as_posix(),
                "sha256": file_sha256(path),
                "size": path.stat().st_size,
            }
        )
    if not artifacts:
        raise ValueError(f"no release artifacts found under {artifacts_dir}")

    bom = {
        "schema_version": 1,
        "repository": "NagareWorks/nnrp-rs",
        "version": version,
        "tag": tag,
        "source_commit": source,
        "inputs": {
            "nnrp_conformance_commit": conformance,
            "nnrp_doc_commit": doc,
        },
        "artifacts": artifacts,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(bom, indent=2) + "\n", encoding="utf-8")
    return bom


def verify_bom(path: pathlib.Path, artifacts_dir: pathlib.Path) -> dict:
    bom = json.loads(path.read_text(encoding="utf-8"))
    normalize_commit(bom["source_commit"], "BOM source commit")
    normalize_commit(
        bom["inputs"]["nnrp_conformance_commit"], "BOM Conformance source commit"
    )
    normalize_commit(bom["inputs"]["nnrp_doc_commit"], "BOM documentation source commit")
    declared_paths = set()
    for artifact in bom["artifacts"]:
        relative = pathlib.PurePosixPath(artifact["path"])
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError(f"BOM artifact path escapes release directory: {relative}")
        if artifact["path"] in declared_paths:
            raise ValueError(f"duplicate BOM artifact path: {artifact['path']}")
        declared_paths.add(artifact["path"])
        artifact_path = artifacts_dir.joinpath(*relative.parts)
        if not artifact_path.is_file():
            raise ValueError(f"BOM artifact is missing: {artifact['path']}")
        if artifact_path.stat().st_size != artifact["size"]:
            raise ValueError(f"BOM artifact size mismatch: {artifact['path']}")
        if file_sha256(artifact_path) != artifact["sha256"]:
            raise ValueError(f"BOM artifact digest mismatch: {artifact['path']}")
    if not declared_paths:
        raise ValueError("BOM does not declare any artifacts")
    return bom


def write_github_output(values: dict[str, str]) -> None:
    path = pathlib.Path(os.environ["GITHUB_OUTPUT"])
    with path.open("a", encoding="utf-8") as handle:
        for key, value in values.items():
            handle.write(f"{key}={value}\n")


def main() -> None:
    parser = argparse.ArgumentParser(description="Verify and record release source identity.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    resolve = subparsers.add_parser("resolve")
    resolve.add_argument("--ref", default="HEAD")
    resolve.add_argument("--github-output", action="store_true")

    verify_tag = subparsers.add_parser("verify-tag")
    verify_tag.add_argument("--tag", required=True)
    verify_tag.add_argument("--source-commit", required=True)
    verify_tag.add_argument("--allow-missing", action="store_true")

    verify_registry = subparsers.add_parser("verify-registry")
    verify_registry.add_argument("--root", type=pathlib.Path, default=pathlib.Path.cwd())
    verify_registry.add_argument("--version", required=True)
    verify_registry.add_argument("--tag", required=True)
    verify_registry.add_argument("--source-commit", required=True)

    create_bom = subparsers.add_parser("create-bom")
    create_bom.add_argument("--artifacts-dir", type=pathlib.Path, required=True)
    create_bom.add_argument("--output", type=pathlib.Path, required=True)
    create_bom.add_argument("--version", required=True)
    create_bom.add_argument("--tag", required=True)
    create_bom.add_argument("--source-commit", required=True)
    create_bom.add_argument("--conformance-source-commit", required=True)
    create_bom.add_argument("--doc-source-commit", required=True)

    verify = subparsers.add_parser("verify-bom")
    verify.add_argument("--bom", type=pathlib.Path, required=True)
    verify.add_argument("--artifacts-dir", type=pathlib.Path, required=True)

    args = parser.parse_args()
    if args.command == "resolve":
        commit = git_commit(args.ref)
        if args.github_output:
            write_github_output({"source_commit": commit})
        else:
            print(commit)
    elif args.command == "verify-tag":
        exists = verify_tag_identity(args.tag, args.source_commit, args.allow_missing)
        print("present" if exists else "missing")
    elif args.command == "verify-registry":
        published = verify_registry_identity(
            args.root, args.version, args.tag, args.source_commit
        )
        print("published=" + ",".join(published))
    elif args.command == "create-bom":
        build_bom(
            args.artifacts_dir,
            args.output,
            args.version,
            args.tag,
            args.source_commit,
            args.conformance_source_commit,
            args.doc_source_commit,
        )
    elif args.command == "verify-bom":
        verify_bom(args.bom, args.artifacts_dir)


if __name__ == "__main__":
    main()
