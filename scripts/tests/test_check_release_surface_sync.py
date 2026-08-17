import importlib.util
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check_release_surface_sync.py"


def load_checker():
    spec = importlib.util.spec_from_file_location("check_release_surface_sync", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ReleaseSurfaceSyncTests(unittest.TestCase):
    def setUp(self):
        self.checker = load_checker()

    def test_reads_complete_typescript_string_union(self):
        declarations = '''
export type TransportRejectionReason =
  | "route-unresolved"
  | "security-unsatisfied";
'''

        self.assertEqual(
            self.checker.declared_typescript_string_union(
                declarations, "TransportRejectionReason"
            ),
            {"route-unresolved", "security-unsatisfied"},
        )

    def test_rejects_missing_typescript_string_union(self):
        with self.assertRaisesRegex(SystemExit, "missing TypeScript string union"):
            self.checker.declared_typescript_string_union("", "Missing")

    def test_reads_matching_rust_and_header_enums(self):
        rust = "pub enum NnrpEventKind { RuntimeFrame = 13, OperationLifecycle = 14 }"
        header = """
typedef enum NnrpEventKind {
  NNRP_EVENT_RUNTIME_FRAME = 13,
  NNRP_EVENT_OPERATION_LIFECYCLE = 14
} NnrpEventKind;
"""

        rust_values = self.checker.declared_rust_enum(rust, "NnrpEventKind")
        header_values = self.checker.declared_header_enum(header, "NnrpEventKind")
        expected_header = {
            f"NNRP_EVENT_{self.checker.screaming_snake(variant)}": value
            for variant, value in rust_values.items()
        }
        self.assertEqual(header_values, expected_header)

    def test_rejects_missing_enum_declarations(self):
        with self.assertRaisesRegex(SystemExit, "missing Rust enum"):
            self.checker.declared_rust_enum("", "Missing")
        with self.assertRaisesRegex(SystemExit, "missing header enum"):
            self.checker.declared_header_enum("", "Missing")


if __name__ == "__main__":
    unittest.main()
