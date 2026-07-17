from __future__ import annotations

import unittest

from scripts.resolve_version import build_package_version, parse_sdk_version_components


class ResolveVersionTests(unittest.TestCase):
    def test_parses_preview_release_components(self) -> None:
        self.assertEqual(
            parse_sdk_version_components("1.0.0-preview.4.5"),
            (1, 0, 0, 4, 5),
        )

    def test_parses_ci_package_suffix_without_changing_revision(self) -> None:
        self.assertEqual(
            parse_sdk_version_components("1.0.0-preview.4.5.20260717.123"),
            (1, 0, 0, 4, 5),
        )

    def test_parses_stable_release_and_ci_components(self) -> None:
        for version in ("1.2.3", "1.2.3-dev.20260717.123"):
            with self.subTest(version=version):
                self.assertEqual(
                    parse_sdk_version_components(version),
                    (1, 2, 3, 0, 0),
                )

    def test_rejects_noncanonical_ci_suffix(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsupported SDK version format"):
            parse_sdk_version_components("1.0.0-preview.4.5.1.2")

    def test_builds_canonical_ci_package_suffix(self) -> None:
        self.assertEqual(
            build_package_version("1.0.0-preview.4.5", "20260717", "00123"),
            "1.0.0-preview.4.5.20260717.123",
        )
        self.assertEqual(
            build_package_version("1.2.3", "20260717", "00123"),
            "1.2.3-dev.20260717.123",
        )

    def test_rejects_invalid_ci_version_inputs(self) -> None:
        for version_date, run_number in (("2026-07-17", "1"), ("20260717", "run-1")):
            with self.subTest(version_date=version_date, run_number=run_number):
                with self.assertRaises(ValueError):
                    build_package_version(
                        "1.0.0-preview.4.5",
                        version_date,
                        run_number,
                    )

    def test_rejects_unsupported_version_shapes(self) -> None:
        for version in ("1.0", "1.0.0-rc.1", "1.0.0-dev.1.2"):
            with self.subTest(version=version):
                with self.assertRaisesRegex(ValueError, "unsupported SDK version format"):
                    parse_sdk_version_components(version)


if __name__ == "__main__":
    unittest.main()
