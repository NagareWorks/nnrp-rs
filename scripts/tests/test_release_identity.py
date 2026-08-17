from __future__ import annotations

import importlib.util
import json
import pathlib
import subprocess
import tempfile
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "release_identity.py"


def load_script():
    spec = importlib.util.spec_from_file_location("release_identity", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ReleaseIdentityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.identity = load_script()

    def test_tag_identity_rejects_another_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo = pathlib.Path(temp_dir)
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.name", "test"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=repo, check=True)
            tracked = repo / "tracked.txt"
            tracked.write_text("first", encoding="utf-8")
            subprocess.run(["git", "add", "tracked.txt"], cwd=repo, check=True)
            subprocess.run(["git", "commit", "-qm", "first"], cwd=repo, check=True)
            first = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
            subprocess.run(["git", "tag", "v1.0.0"], cwd=repo, check=True)
            tracked.write_text("second", encoding="utf-8")
            subprocess.run(["git", "commit", "-qam", "second"], cwd=repo, check=True)
            second = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()

            previous = pathlib.Path.cwd()
            try:
                import os

                os.chdir(repo)
                self.assertTrue(self.identity.verify_tag_identity("v1.0.0", first, False))
                with self.assertRaisesRegex(RuntimeError, "not source commit"):
                    self.identity.verify_tag_identity("v1.0.0", second, False)
                self.assertFalse(
                    self.identity.verify_tag_identity("v2.0.0", second, True)
                )
            finally:
                os.chdir(previous)

    def test_bom_records_and_verifies_artifacts(self) -> None:
        commit = "a" * 40
        conformance = "b" * 40
        doc = "c" * 40
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            artifact = root / "native" / "artifact.zip"
            artifact.parent.mkdir()
            artifact.write_bytes(b"release artifact")
            bom_path = root / "release-bom.json"

            bom = self.identity.build_bom(
                root,
                bom_path,
                "1.0.0-preview.4.23",
                "v1.0.0-preview.4.23",
                commit,
                conformance,
                doc,
            )
            self.assertEqual(bom["source_commit"], commit)
            self.assertEqual(bom["artifacts"][0]["path"], "native/artifact.zip")
            self.identity.verify_bom(bom_path, root)

            artifact.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "size mismatch|digest mismatch"):
                self.identity.verify_bom(bom_path, root)

    def test_bom_rejects_path_escape(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            bom_path = root / "release-bom.json"
            bom_path.write_text(
                json.dumps(
                    {
                        "source_commit": "a" * 40,
                        "inputs": {
                            "nnrp_conformance_commit": "b" * 40,
                            "nnrp_doc_commit": "c" * 40,
                        },
                        "artifacts": [
                            {"path": "../outside", "sha256": "0" * 64, "size": 0}
                        ],
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "escapes release directory"):
                self.identity.verify_bom(bom_path, root)

    def test_bom_rejects_symlink_escape(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            base = pathlib.Path(temp_dir)
            root = base / "release"
            root.mkdir()
            outside = base / "outside.bin"
            outside.write_bytes(b"outside")
            linked = root / "linked.bin"
            try:
                linked.symlink_to(outside)
            except OSError as error:
                self.skipTest(f"file symlinks are unavailable: {error}")

            bom_path = root / "release-bom.json"
            bom_path.write_text(
                json.dumps(
                    {
                        "source_commit": "a" * 40,
                        "inputs": {
                            "nnrp_conformance_commit": "b" * 40,
                            "nnrp_doc_commit": "c" * 40,
                        },
                        "artifacts": [
                            {
                                "path": "linked.bin",
                                "sha256": self.identity.file_sha256(outside),
                                "size": outside.stat().st_size,
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "escapes release directory"):
                self.identity.verify_bom(bom_path, root)
            with self.assertRaisesRegex(ValueError, "escapes release directory"):
                self.identity.build_bom(
                    root,
                    bom_path,
                    "1.0.0-preview.4.23",
                    "v1.0.0-preview.4.23",
                    "a" * 40,
                    "b" * 40,
                    "c" * 40,
                )

    def test_bom_rejects_malformed_artifact_entries_with_value_errors(self) -> None:
        valid = {
            "source_commit": "a" * 40,
            "inputs": {
                "nnrp_conformance_commit": "b" * 40,
                "nnrp_doc_commit": "c" * 40,
            },
            "artifacts": [
                {
                    "path": "artifact.bin",
                    "sha256": "0" * 64,
                    "size": 1,
                }
            ],
        }
        cases = [
            ({**valid, "artifacts": None}, "artifacts.*array"),
            ({**valid, "artifacts": [None]}, "artifact 0 must be an object"),
            ({**valid, "artifacts": [{"sha256": "0" * 64, "size": 1}]}, "path"),
            (
                {**valid, "artifacts": [{"path": "artifact.bin", "size": 1}]},
                "sha256",
            ),
            (
                {
                    **valid,
                    "artifacts": [
                        {
                            "path": "artifact.bin",
                            "sha256": "not-a-digest",
                            "size": 1,
                        }
                    ],
                },
                "lowercase SHA-256 digest",
            ),
            (
                {
                    **valid,
                    "artifacts": [
                        {
                            "path": "artifact.bin",
                            "sha256": "0" * 64,
                            "size": True,
                        }
                    ],
                },
                "non-negative integer",
            ),
            (
                {
                    **valid,
                    "artifacts": [
                        {
                            "path": "artifact.bin",
                            "sha256": "0" * 64,
                            "size": -1,
                        }
                    ],
                },
                "non-negative integer",
            ),
        ]

        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            bom_path = root / "release-bom.json"
            for bom, error in cases:
                with self.subTest(error=error):
                    bom_path.write_text(json.dumps(bom), encoding="utf-8")
                    with self.assertRaisesRegex(ValueError, error):
                        self.identity.verify_bom(bom_path, root)

    def test_registry_version_requires_matching_immutable_tag(self) -> None:
        with mock.patch.object(
            self.identity, "workspace_crate_names", return_value=["nnrp-core", "nnrp-runtime"]
        ), mock.patch.object(
            self.identity,
            "crate_version_exists",
            side_effect=lambda crate, version: crate == "nnrp-core",
        ), mock.patch.object(
            self.identity,
            "verify_tag_identity",
            side_effect=RuntimeError("tag missing"),
        ):
            with self.assertRaisesRegex(RuntimeError, "immutable release tag"):
                self.identity.verify_registry_identity(
                    ROOT,
                    "1.0.0-preview.4.23",
                    "v1.0.0-preview.4.23",
                    "a" * 40,
                )

    def test_unpublished_registry_version_does_not_require_a_tag(self) -> None:
        with mock.patch.object(
            self.identity, "workspace_crate_names", return_value=["nnrp-core"]
        ), mock.patch.object(
            self.identity, "crate_version_exists", return_value=False
        ), mock.patch.object(self.identity, "verify_tag_identity") as verify_tag:
            self.assertEqual(
                self.identity.verify_registry_identity(
                    ROOT,
                    "1.0.0-preview.4.23",
                    "v1.0.0-preview.4.23",
                    "a" * 40,
                ),
                [],
            )
            verify_tag.assert_not_called()


if __name__ == "__main__":
    unittest.main()
