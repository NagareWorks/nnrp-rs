import importlib.util
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "package_wasm_primitives.py"


def load_package_script():
    spec = importlib.util.spec_from_file_location("package_wasm_primitives", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class WasmArtifactPathTests(unittest.TestCase):
    def test_cargo_target_root_honors_absolute_and_relative_environment_paths(self):
        package = load_package_script()
        absolute = Path(tempfile.gettempdir()) / "nnrp-wasm-cargo-target"

        with mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": str(absolute)}):
            self.assertEqual(package.cargo_target_root(), absolute)
        with mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": "build/wasm-target"}):
            self.assertEqual(
                package.cargo_target_root(),
                ROOT / "build" / "wasm-target",
            )
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertEqual(package.cargo_target_root(), ROOT / "target")

    def test_declared_class_method_checks_are_class_scoped(self):
        package = load_package_script()
        declarations = """
export class BrowserClientConnection {
  openSession(): Promise<any>;
  close(): Promise<any>;
}
export class BrowserClientRole {
  recoveryTicket(): Uint8Array | undefined;
  close(): Promise<any>;
}
"""

        package.require_class_methods(
            declarations,
            "BrowserClientConnection",
            ("openSession", "close"),
        )
        with self.assertRaisesRegex(SystemExit, "BrowserClientConnection.*recoveryTicket"):
            package.require_class_methods(
                declarations,
                "BrowserClientConnection",
                ("recoveryTicket",),
            )


if __name__ == "__main__":
    unittest.main()
