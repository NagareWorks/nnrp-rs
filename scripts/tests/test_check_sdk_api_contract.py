import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check_sdk_api_contract.py"


def load_checker():
    spec = importlib.util.spec_from_file_location("check_sdk_api_contract", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def frozen_contract():
    return {
        "contractVersion": 8,
        "types": {
            "OperationLifecycleEvent": {
                "fields": [
                    {"name": "operation_id", "type": "u64", "required": True},
                    {"name": "state", "type": "OperationState", "required": True},
                ],
                "terminalMapping": {
                    "completed": "success",
                    "cancelled": "cancelled",
                    "superseded": "dropped",
                    "failed": "error",
                },
            },
            "TerminalEvent": {
                "representation": "tagged-union",
                "variants": ["runtime", "lifecycle"],
                "variantTypes": {
                    "runtime": "RuntimeEvent",
                    "lifecycle": "OperationLifecycleEvent",
                },
            },
            "NnrpResult": {
                "fields": [
                    {"name": "operation_id", "type": "u64", "required": True},
                    {
                        "name": "terminal_state",
                        "type": "ResultTerminalState",
                        "required": True,
                    },
                    {"name": "event", "type": "TerminalEvent", "required": True},
                ]
            },
        },
        "languageProjections": {
            "rust": {
                "operationLifecycleEvent": "nnrp_runtime::OperationLifecycleEvent",
                "terminalEvent": "nnrp_runtime::NnrpTerminalEvent",
                "result": "nnrp_runtime::NnrpResult",
            }
        },
    }


class SdkApiContractTests(unittest.TestCase):
    def setUp(self):
        self.checker = load_checker()

    def check(self, contract):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "contract.json"
            path.write_text(json.dumps(contract), encoding="utf-8")
            self.checker.check_contract(path)

    def test_accepts_the_frozen_terminal_result_contract(self):
        self.check(frozen_contract())

    def test_rejects_terminal_event_variant_drift(self):
        contract = frozen_contract()
        contract["types"]["TerminalEvent"]["variants"] = ["runtime"]
        with self.assertRaisesRegex(SystemExit, "TerminalEvent variants drifted"):
            self.check(contract)

    def test_rejects_result_field_drift(self):
        contract = frozen_contract()
        contract["types"]["NnrpResult"]["fields"][2]["type"] = "RuntimeEvent"
        with self.assertRaisesRegex(SystemExit, "NnrpResult field contract drifted"):
            self.check(contract)

    def test_rejects_rust_projection_drift(self):
        contract = frozen_contract()
        contract["languageProjections"]["rust"]["terminalEvent"] = "LegacyEvent"
        with self.assertRaisesRegex(SystemExit, "Rust NnrpTerminalEvent projection drifted"):
            self.check(contract)


if __name__ == "__main__":
    unittest.main()
