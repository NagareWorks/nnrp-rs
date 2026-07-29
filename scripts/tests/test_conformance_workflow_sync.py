import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]


class ConformanceWorkflowSyncTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
