import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT_PATH = (
    Path(__file__).resolve().parents[1]
    / "run_preview4_host_route_conformance.py"
)
SPEC = importlib.util.spec_from_file_location("host_route_conformance", SCRIPT_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"cannot load host-route conformance script: {SCRIPT_PATH}")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class HostRouteConformanceScriptTests(unittest.TestCase):
    def test_patch_arguments_cover_every_rust_dependency(self):
        root = Path("repo").resolve()
        arguments = MODULE.patch_arguments(root)

        self.assertEqual(len(arguments), len(MODULE.RUST_PACKAGES) * 2)
        configs = arguments[1::2]
        for package in MODULE.RUST_PACKAGES:
            self.assertTrue(
                any(
                    config.startswith(
                        f'patch."https://github.com/NagareWorks/nnrp-rs.git".{package}.path='
                    )
                    for config in configs
                )
            )

    def test_native_coverage_requires_all_non_browser_public_cases(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            case_path = root / MODULE.HOST_ROUTE_CASES
            case_path.parent.mkdir(parents=True)
            public_ids = {
                "wire.host-route.client.multi-route",
                MODULE.UNINSTALLED_CASE,
                MODULE.BROWSER_CASE,
            }
            scenarios = [
                {"id": "wire.host-route.client.multi-route"},
                {"id": MODULE.UNINSTALLED_CASE},
                {
                    "id": MODULE.BROWSER_CASE,
                    "host_route": {
                        "platform": "browser",
                        "application_endpoint": "nnrps://host-route.test",
                        "routes": [
                            {
                                "transport": "websocket",
                                "provider_id": "nnrp.transport.websocket.browser-wasm",
                                "security": {"mode": "browser_host"},
                            }
                        ],
                    },
                },
            ]
            case_path.write_text(
                json.dumps({"scenarios": scenarios}),
                encoding="utf-8",
            )
            native = root / "native.json"
            native.write_text(
                json.dumps(
                    {
                        "results": [
                            {
                                "id": "wire.host-route.client.multi-route",
                                "outcome": "passed",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            uninstalled = root / "uninstalled.json"
            uninstalled.write_text(
                json.dumps(
                    {
                        "results": [
                            {"id": MODULE.UNINSTALLED_CASE, "outcome": "passed"}
                        ]
                    }
                ),
                encoding="utf-8",
            )

            MODULE.assert_native_coverage(root, native, uninstalled)

            native.write_text(json.dumps({"results": []}), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "result report is empty"):
                MODULE.assert_native_coverage(root, native, uninstalled)

            for invalid_result in (
                "not-an-object",
                {"outcome": "passed"},
                {"id": 7, "outcome": "passed"},
                {"id": "", "outcome": "passed"},
            ):
                native.write_text(
                    json.dumps({"results": [invalid_result]}), encoding="utf-8"
                )
                with self.assertRaisesRegex(RuntimeError, "invalid result entry"):
                    MODULE.result_ids(native)

            native.write_text(
                json.dumps(
                    {
                        "results": [
                            {"id": "valid.case", "outcome": "failed"},
                            {"id": "second.case", "outcome": "passed"},
                        ]
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(RuntimeError, "valid.case"):
                MODULE.result_ids(native)

            manifest = json.loads(case_path.read_text(encoding="utf-8"))
            browser = next(
                scenario
                for scenario in manifest["scenarios"]
                if scenario["id"] == MODULE.BROWSER_CASE
            )
            browser["host_route"]["routes"][0]["security"]["mode"] = "wss"
            case_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "contract drifted"):
                MODULE.assert_browser_contract(root)


if __name__ == "__main__":
    unittest.main()
