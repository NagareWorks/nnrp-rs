import copy
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
    checker = load_checker()
    return {
        "contractVersion": checker.EXPECTED_CONTRACT_VERSION,
        "apiDomains": {name: {} for name in checker.EXPECTED_API_DOMAINS},
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
            "SessionRecoveryTicket": {
                "fields": [
                    {"name": "session_id", "type": "u32", "required": True},
                    {"name": "resume_token", "type": "bytes", "required": True},
                    {
                        "name": "resume_from_operation_id",
                        "type": "u64?",
                        "required": False,
                    },
                    {
                        "name": "resume_window_ms",
                        "type": "u32",
                        "required": True,
                    },
                ],
                "opaqueEncoding": {
                    "name": "NRTK",
                    "version": 1,
                    "byteOrder": "little-endian",
                    "fixedPrefixBytes": 28,
                    "fields": [
                        {
                            "name": "magic",
                            "type": "bytes[4]",
                            "offset": 0,
                            "constant": "NRTK",
                        },
                        {
                            "name": "version",
                            "type": "u16",
                            "offset": 4,
                            "constant": 1,
                        },
                        {"name": "flags", "type": "u16", "offset": 6},
                        {"name": "session_id", "type": "u32", "offset": 8},
                        {
                            "name": "resume_token_bytes",
                            "type": "u32",
                            "offset": 12,
                        },
                        {
                            "name": "resume_window_ms",
                            "type": "u32",
                            "offset": 16,
                        },
                        {
                            "name": "resume_from_operation_id",
                            "type": "u64",
                            "offset": 20,
                        },
                    ],
                    "flags": {"resume_from_operation_id_present": 1},
                    "reservedFlagsMask": 65_534,
                    "tail": "resume_token[resume_token_bytes]",
                    "validation": [
                        "magic and version match exactly",
                        "reserved flags are zero",
                        "session_id and resume_token_bytes are non-zero",
                        "the input ends exactly after resume_token",
                    ],
                },
            },
            "RuntimeEventMetadata": {
                "variants": checker.EXPECTED_RUNTIME_EVENT_METADATA_VARIANTS.copy(),
            },
        },
        "roleMethodMessages": [
            {"messageType": name}
            for name in sorted(checker.EXPECTED_ROLE_METHOD_MESSAGES)
        ],
        "languageProjections": {
            "rust": copy.deepcopy(checker.EXPECTED_RUST_PROJECTIONS),
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

    def test_rejects_recovery_ticket_encoding_drift(self):
        contract = frozen_contract()
        contract["types"]["SessionRecoveryTicket"]["opaqueEncoding"][
            "fixedPrefixBytes"
        ] = 24
        with self.assertRaisesRegex(
            SystemExit, "SessionRecoveryTicket opaque encoding drifted"
        ):
            self.check(contract)

    def test_rejects_rust_projection_drift(self):
        contract = frozen_contract()
        contract["languageProjections"]["rust"]["terminalEvent"] = "LegacyEvent"
        with self.assertRaisesRegex(SystemExit, "Rust SDK projection map drifted"):
            self.check(contract)

    def test_rejects_missing_api_domain(self):
        contract = frozen_contract()
        del contract["apiDomains"]["roles"]
        with self.assertRaisesRegex(SystemExit, "SDK API domain set drifted"):
            self.check(contract)

    def test_rejects_missing_api_domains_object(self):
        contract = frozen_contract()
        del contract["apiDomains"]
        with self.assertRaisesRegex(SystemExit, "SDK API domains must be an object"):
            self.check(contract)

    def test_rejects_runtime_event_variant_drift(self):
        contract = frozen_contract()
        contract["types"]["RuntimeEventMetadata"]["variants"].append("cache_ack")
        with self.assertRaisesRegex(
            SystemExit, "RuntimeEventMetadata closed variant set drifted"
        ):
            self.check(contract)

    def test_rejects_role_method_message_drift(self):
        contract = frozen_contract()
        contract["roleMethodMessages"] = [
            entry
            for entry in contract["roleMethodMessages"]
            if entry["messageType"] != "cache_ack"
        ]
        with self.assertRaisesRegex(SystemExit, "dedicated role-method message set drifted"):
            self.check(contract)

    def test_rejects_missing_role_method_messages(self):
        contract = frozen_contract()
        del contract["roleMethodMessages"]
        with self.assertRaisesRegex(
            SystemExit, "SDK role-method messages must be an array"
        ):
            self.check(contract)

    def test_rejects_malformed_role_method_message(self):
        contract = frozen_contract()
        contract["roleMethodMessages"][0] = {}
        with self.assertRaisesRegex(SystemExit, "must declare a non-empty messageType"):
            self.check(contract)

    def test_rejects_duplicate_role_method_message(self):
        contract = frozen_contract()
        contract["roleMethodMessages"].append(
            copy.deepcopy(contract["roleMethodMessages"][0])
        )
        with self.assertRaisesRegex(SystemExit, "message types must be unique"):
            self.check(contract)

    def test_frozen_contract_deep_copies_projection_collections(self):
        first = frozen_contract()
        first["languageProjections"]["rust"]["clientRoles"].append("LegacyClient")
        second = frozen_contract()
        self.assertNotIn(
            "LegacyClient", second["languageProjections"]["rust"]["clientRoles"]
        )


if __name__ == "__main__":
    unittest.main()
