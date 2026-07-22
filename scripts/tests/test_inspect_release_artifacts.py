import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "inspect_release_artifacts.py"


def load_inspector():
    spec = importlib.util.spec_from_file_location("inspect_release_artifacts", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class WasmArtifactInspectionTests(unittest.TestCase):
    def setUp(self):
        self.inspector = load_inspector()
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.wasm_dir = Path(self.temporary_directory.name)
        self.package_dir = self.wasm_dir / "nnrp-wasm-browser"
        self.package_dir.mkdir()

        scope = self.inspector.BROWSER_WASM_SCOPE
        manifest = {
            "transport_scope": scope["scope"],
            "transport_name": scope["scope"],
            "package": scope["package"],
            "artifact": scope["artifact"],
            "transport_slots": scope["slots"],
            "enabled_features": scope["features"],
            "exports": scope["exports"],
            "provider": scope["provider"],
            "wasm": "nnrp_wasm_bg.wasm",
            "glue": "nnrp_wasm.js",
            "types": "nnrp_wasm.d.ts",
        }
        (self.package_dir / "manifest.json").write_text(json.dumps(manifest))
        (self.package_dir / "nnrp_wasm_bg.wasm").write_bytes(b"\0asm\x01\0\0\0")
        declarations = "\n".join(
            f"export function {name}(): void;" for name in scope["exports"]
        )
        glue = f"// nnrp_wasm_bg.wasm\n{declarations}\n"
        (self.package_dir / "nnrp_wasm.js").write_text(glue)
        (self.package_dir / "nnrp_wasm.d.ts").write_text(declarations)

    def tearDown(self):
        self.temporary_directory.cleanup()

    def test_accepts_browser_loadable_wasm_package(self):
        self.inspector.inspect_wasm(self.wasm_dir)

    def test_rejects_manifest_export_missing_from_glue(self):
        missing = self.inspector.BROWSER_WASM_SCOPE["exports"][0]
        glue_path = self.package_dir / "nnrp_wasm.js"
        glue_path.write_text(
            glue_path.read_text().replace(
                f"export function {missing}", "function missing"
            )
        )

        with self.assertRaisesRegex(SystemExit, f"missing callable export {missing}"):
            self.inspector.inspect_wasm(self.wasm_dir)

    def test_rejects_raw_or_corrupt_wasm_binary(self):
        (self.package_dir / "nnrp_wasm_bg.wasm").write_bytes(b"not-wasm")

        with self.assertRaisesRegex(SystemExit, "not a WebAssembly 1 binary"):
            self.inspector.inspect_wasm(self.wasm_dir)


if __name__ == "__main__":
    unittest.main()
