#!/usr/bin/env python3
"""Resolve a deterministic crates.io publish order from Cargo metadata."""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]


def load_metadata() -> dict[str, object]:
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or error.stdout or str(error)).strip()
        raise RuntimeError(f"cargo metadata failed: {detail}") from error
    return json.loads(result.stdout)


def is_publishable_to_crates_io(package: dict[str, object]) -> bool:
    registries = package.get("publish")
    if registries is None:
        return True
    if not isinstance(registries, list) or not all(
        isinstance(registry, str) for registry in registries
    ):
        raise TypeError(f"invalid publish registry list for {package.get('name')}")
    return "crates-io" in registries


def resolve_publish_order(metadata: dict[str, object]) -> list[str]:
    packages = {
        package["id"]: package
        for package in metadata["packages"]
        if package["id"] in metadata["workspace_members"]
        and is_publishable_to_crates_io(package)
    }
    package_names = {package["name"] for package in packages.values()}
    dependencies = {
        package["name"]: {
            dependency["name"]
            for dependency in package["dependencies"]
            if dependency["kind"] != "dev" and dependency["name"] in package_names
        }
        for package in packages.values()
    }

    order: list[str] = []
    remaining = set(dependencies)
    while remaining:
        published = set(order)
        ready = sorted(name for name in remaining if dependencies[name] <= published)
        if not ready:
            cycle = ", ".join(sorted(remaining))
            raise RuntimeError(f"workspace publish dependency cycle: {cycle}")
        order.extend(ready)
        remaining.difference_update(ready)
    return order


def main() -> int:
    try:
        order = resolve_publish_order(load_metadata())
    except (KeyError, TypeError, ValueError, subprocess.CalledProcessError, RuntimeError) as error:
        print(f"failed to resolve crate publish order: {error}", file=sys.stderr)
        return 1

    if not order:
        print("failed to resolve crate publish order: no publishable workspace crates", file=sys.stderr)
        return 1

    print("\n".join(order))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
