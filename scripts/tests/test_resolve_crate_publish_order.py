from __future__ import annotations

import subprocess
import unittest
from unittest.mock import patch

from scripts.resolve_crate_publish_order import (
    is_publishable_to_crates_io,
    load_metadata,
    resolve_publish_order,
)


def package(
    package_id: str,
    name: str,
    dependencies: list[tuple[str, str | None]],
    *,
    publish: list[str] | None = None,
) -> dict[str, object]:
    return {
        "id": package_id,
        "name": name,
        "publish": publish,
        "dependencies": [
            {"name": dependency, "kind": kind}
            for dependency, kind in dependencies
        ],
    }


class ResolveCratePublishOrderTests(unittest.TestCase):
    @patch("scripts.resolve_crate_publish_order.subprocess.run")
    def test_metadata_failure_reports_cargo_stderr(self, run) -> None:
        run.side_effect = subprocess.CalledProcessError(
            101,
            ["cargo", "metadata"],
            stderr="manifest dependency is invalid",
        )

        with self.assertRaisesRegex(RuntimeError, "manifest dependency is invalid"):
            load_metadata()

    def test_orders_publishable_normal_and_build_dependencies(self) -> None:
        metadata = {
            "workspace_members": ["ffi", "runtime", "core", "private"],
            "packages": [
                package("ffi", "nnrp-ffi", [("nnrp-runtime", None)]),
                package("runtime", "nnrp-runtime", [("nnrp-core", "build")]),
                package("core", "nnrp-core", [("test-helper", "dev")]),
                package("private", "test-helper", [], publish=[]),
            ],
        }

        self.assertEqual(
            resolve_publish_order(metadata),
            ["nnrp-core", "nnrp-runtime", "nnrp-ffi"],
        )

    def test_only_includes_packages_publishable_to_crates_io(self) -> None:
        metadata = {
            "workspace_members": ["default", "crates-io", "private", "disabled"],
            "packages": [
                package("default", "default", []),
                package("crates-io", "crates-io", [], publish=["crates-io"]),
                package("private", "private", [], publish=["private-registry"]),
                package("disabled", "disabled", [], publish=[]),
            ],
        }

        self.assertEqual(resolve_publish_order(metadata), ["crates-io", "default"])

    def test_rejects_invalid_publish_registry_metadata(self) -> None:
        with self.assertRaisesRegex(TypeError, "invalid publish registry list"):
            is_publishable_to_crates_io({"name": "invalid", "publish": "crates-io"})

    def test_rejects_workspace_dependency_cycles(self) -> None:
        metadata = {
            "workspace_members": ["first", "second"],
            "packages": [
                package("first", "first", [("second", None)]),
                package("second", "second", [("first", None)]),
            ],
        }

        with self.assertRaisesRegex(RuntimeError, "workspace publish dependency cycle"):
            resolve_publish_order(metadata)

    def test_orders_same_layer_packages_lexicographically(self) -> None:
        metadata = {
            "workspace_members": ["zeta", "core", "alpha"],
            "packages": [
                package("zeta", "zeta", [("core", None)]),
                package("core", "core", []),
                package("alpha", "alpha", [("core", None)]),
            ],
        }

        self.assertEqual(resolve_publish_order(metadata), ["core", "alpha", "zeta"])

    def test_ignores_dev_dependencies_between_publishable_packages(self) -> None:
        metadata = {
            "workspace_members": ["alpha", "zeta"],
            "packages": [
                package("alpha", "alpha", [("zeta", "dev")]),
                package("zeta", "zeta", []),
            ],
        }

        self.assertEqual(resolve_publish_order(metadata), ["alpha", "zeta"])


if __name__ == "__main__":
    unittest.main()
