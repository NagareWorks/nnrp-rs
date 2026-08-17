import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
CONFORMANCE_REVISION = "685505dc0624f68ff4d660c78d24ea7e9b1b0290"
DOC_REVISION = "3439ded0d318bd736f6485b17f2563fae77627bf"


class ConformanceWorkflowSyncTests(unittest.TestCase):
    def test_ci_and_release_pin_frozen_contract_inputs(self) -> None:
        ci = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        release = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")

        for workflow in (ci, release):
            with self.subTest(workflow=workflow.splitlines()[0]):
                self.assertIn(
                    f"NNRP_CONFORMANCE_SOURCE_COMMIT: {CONFORMANCE_REVISION}", workflow
                )
                self.assertIn(f"NNRP_DOC_SOURCE_COMMIT: {DOC_REVISION}", workflow)
                self.assertIn("ref: ${{ env.NNRP_CONFORMANCE_SOURCE_COMMIT }}", workflow)
                self.assertIn("ref: ${{ env.NNRP_DOC_SOURCE_COMMIT }}", workflow)
                self.assertIn("python scripts/check_sdk_api_contract.py", workflow)
                self.assertIn(
                    'require-complete-capability-coverage: "true"', workflow
                )

        self.assertEqual(
            ci.count("ref: ${{ env.NNRP_CONFORMANCE_SOURCE_COMMIT }}"), 2
        )
        self.assertEqual(
            release.count("ref: ${{ env.NNRP_CONFORMANCE_SOURCE_COMMIT }}"), 1
        )
        self.assertEqual(ci.count("ref: ${{ env.NNRP_DOC_SOURCE_COMMIT }}"), 1)
        self.assertEqual(release.count("ref: ${{ env.NNRP_DOC_SOURCE_COMMIT }}"), 1)

    def test_ci_publishes_and_requires_windows_x86_native_artifacts(self) -> None:
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")

        self.assertIn("native-artifact-windows-x86:", workflow)
        self.assertIn("--target i686-pc-windows-msvc", workflow)
        self.assertIn("name: nnrp-ffi-native-Windows-X86", workflow)
        self.assertIn("needs.native-artifact-windows-x86.result", workflow)

    def test_ci_and_release_run_only_the_current_suite_adapter(self) -> None:
        for relative_path in (
            pathlib.Path(".github/workflows/ci.yml"),
            pathlib.Path(".github/workflows/release.yml"),
        ):
            workflow = (ROOT / relative_path).read_text(encoding="utf-8")
            with self.subTest(workflow=str(relative_path)):
                self.assertEqual(workflow.count("protocol-version: nnrp-1-preview4"), 1)
                self.assertEqual(
                    workflow.count(
                        "capabilities-path: conformance/nnrp-1-preview4.capabilities.json"
                    ),
                    1,
                )
                self.assertEqual(
                    workflow.count(
                        "cargo run -p nnrp-conformance --bin nnrp-conformance-adapter --"
                    ),
                    1,
                )
                self.assertNotIn("protocol-version: nnrp-1-preview2", workflow)
                self.assertNotIn("protocol-version: nnrp-1-preview3", workflow)
                self.assertNotIn("nnrp-1-preview2.capabilities.json", workflow)
                self.assertNotIn("nnrp-1-preview3.capabilities.json", workflow)

    def test_release_builds_every_artifact_from_one_immutable_source(self) -> None:
        workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")

        self.assertIn("prepare:", workflow)
        self.assertIn("python scripts/release_identity.py resolve --github-output", workflow)
        self.assertGreaterEqual(
            workflow.count("ref: ${{ needs.prepare.outputs.source_commit }}"), 2
        )
        self.assertIn("NNRP_SOURCE_COMMIT: ${{ needs.prepare.outputs.source_commit }}", workflow)
        self.assertIn("python scripts/release_identity.py verify-tag", workflow)
        self.assertIn("python scripts/release_identity.py verify-registry", workflow)
        self.assertIn("python scripts/release_identity.py create-bom", workflow)
        self.assertIn("python scripts/release_identity.py verify-bom", workflow)
        self.assertIn('--sdk-version "${{ needs.prepare.outputs.package_version }}"', workflow)
        self.assertIn('--source-commit "${{ needs.prepare.outputs.source_commit }}"', workflow)
        self.assertNotIn("steps.version.outputs", workflow.split("package:", 1)[1])


if __name__ == "__main__":
    unittest.main()
