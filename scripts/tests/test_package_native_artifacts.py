import importlib.util
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "package_native_artifacts.py"


def load_package_script():
    spec = importlib.util.spec_from_file_location("package_native_artifacts", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class NativeExportVerificationTests(unittest.TestCase):
    def test_every_retired_abi_export_is_rejected(self):
        package = load_package_script()
        library = Path("nnrp_ffi.test")

        for retired in package.RETIRED_ABI_EXPORTS:
            with self.subTest(retired=retired):
                exports = set(package.EXPECTED_EXPORTS)
                exports.add(retired)
                with mock.patch.object(package, "list_exports", return_value=exports):
                    with self.assertRaisesRegex(SystemExit, retired):
                        package.verify_exports(library, "linux", "dynamic")


if __name__ == "__main__":
    unittest.main()
