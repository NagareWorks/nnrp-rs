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


if __name__ == "__main__":
    unittest.main()
