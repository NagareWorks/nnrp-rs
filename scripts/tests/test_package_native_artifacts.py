import importlib.util
import os
import tempfile
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
    def test_cargo_target_root_honors_absolute_and_relative_environment_paths(self):
        package = load_package_script()
        absolute = Path(tempfile.gettempdir()) / "nnrp-cargo-target"

        with mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": str(absolute)}):
            self.assertEqual(package.cargo_target_root(), absolute)
        with mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": "build/cargo-target"}):
            self.assertEqual(
                package.cargo_target_root(),
                ROOT / "build" / "cargo-target",
            )
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertEqual(package.cargo_target_root(), ROOT / "target")

    def test_static_exports_use_rust_llvm_nm(self):
        package = load_package_script()
        library = Path("libnnrp_ffi.a")
        llvm_nm = Path("llvm-nm")
        output = """
libnnrp_ffi.a[empty.rcgu.o]:

libnnrp_ffi.a[ffi.rcgu.o]:
_nnrp_ffi_abi_version T 00000000 00000004
not_an_nnrp_export T 00000004 00000004
"""

        with mock.patch.object(package, "find_rust_llvm_tool", return_value=llvm_nm):
            with mock.patch.object(
                package.subprocess,
                "check_output",
                return_value=output,
            ) as check_output:
                self.assertEqual(
                    package.list_exports(library, "ios", "static"),
                    {"nnrp_ffi_abi_version"},
                )

        check_output.assert_called_once_with(
            [
                str(llvm_nm),
                "--format=posix",
                "--extern-only",
                "--defined-only",
                str(library),
            ],
            text=True,
        )

    def test_static_exports_require_rust_llvm_nm(self):
        package = load_package_script()
        with mock.patch.object(package, "find_rust_llvm_tool", return_value=None):
            with self.assertRaisesRegex(SystemExit, "llvm-tools-preview"):
                package.list_exports(Path("libnnrp_ffi.a"), "ios", "static")

    def test_find_rust_llvm_tool_uses_active_toolchain_sysroot(self):
        package = load_package_script()
        with tempfile.TemporaryDirectory() as temp_dir:
            sysroot = Path(temp_dir)
            executable = "llvm-nm.exe" if os.name == "nt" else "llvm-nm"
            bundled = sysroot / "lib" / "rustlib" / "test-host" / "bin" / executable
            bundled.parent.mkdir(parents=True)
            bundled.touch()
            with mock.patch.object(
                package.shutil,
                "which",
                return_value=str(Path("system-tools") / executable),
            ):
                with mock.patch.object(
                    package.subprocess,
                    "check_output",
                    side_effect=[str(sysroot), "rustc 1.0\nhost: test-host\n"],
                ):
                    self.assertEqual(package.find_rust_llvm_tool("llvm-nm"), bundled)

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

    def test_skip_build_rejects_multiple_transport_scopes(self):
        package = load_package_script()

        with self.assertRaisesRegex(SystemExit, "requires exactly one"):
            package.validate_transport_build_selection(["tcp", "quic"], True)

    def test_skip_build_accepts_one_transport_scope(self):
        package = load_package_script()

        package.validate_transport_build_selection(["tcp"], True)


if __name__ == "__main__":
    unittest.main()
