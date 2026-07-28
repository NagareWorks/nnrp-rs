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
    def test_dumpbin_exports_accept_x86_alias_format(self):
        package = load_package_script()
        output = """
              22   15 00093390 nnrp_current_protocol_version = _nnrp_current_protocol_version
              66   41 000A1020 nnrp_transport_runtime_shutdown = _nnrp_transport_runtime_shutdown
        """

        self.assertEqual(
            package.parse_dumpbin_exports(output),
            {"nnrp_current_protocol_version", "nnrp_transport_runtime_shutdown"},
        )

    def test_role_connection_lifecycle_exports_are_required(self):
        package = load_package_script()

        self.assertIn("nnrp_connection_close", package.EXPECTED_EXPORTS)
        self.assertIn("nnrp_client_close_connection", package.EXPECTED_EXPORTS)

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

    def test_library_isolation_check_receives_every_packaged_transport(self):
        package = load_package_script()
        libraries = {
            scope: Path(f"artifacts/{scope}/nnrp_ffi.test")
            for scope in package.TRANSPORT_SCOPES
        }

        with mock.patch.object(package.subprocess, "run") as run:
            package.verify_library_isolation(libraries)

        command = run.call_args.args[0]
        self.assertEqual(
            command[1], str(ROOT / "scripts/check_native_transport_library_isolation.py")
        )
        for scope, library in libraries.items():
            self.assertIn(f"{scope}={library}", command)
        self.assertTrue(run.call_args.kwargs["check"])


if __name__ == "__main__":
    unittest.main()
