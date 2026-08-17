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
        "wireLayouts": {
            "BodyRegionPrelude": {
                "validation": copy.deepcopy(
                    checker.EXPECTED_BODY_REGION_VALIDATION
                )
            }
        },
        "apiDomains": {name: {} for name in checker.EXPECTED_API_DOMAINS},
        "types": {
            "TransportProviderDescriptor": {
                "nameSemantics": (
                    "provider-owned package or display name; protocol transport identity is "
                    "transport_id and selection must not derive it from name"
                ),
            },
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
                "nativeEventProjection": copy.deepcopy(
                    checker.EXPECTED_NATIVE_LIFECYCLE_PROJECTION
                ),
            },
            "ClientEvent": {
                "fields": [],
                "representation": "tagged-union",
                "variants": ["runtime", "lifecycle"],
                "variantTypes": {
                    "runtime": "RuntimeEvent",
                    "lifecycle": "OperationLifecycleEvent",
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
            "ServerOperation": {
                "fields": [
                    {"name": "operation_id", "type": "u64", "required": True},
                    {"name": "frame_id", "type": "u32", "required": True},
                    {"name": "submit", "type": "RuntimeEvent", "required": True},
                ],
                "terminalMethods": ["send_result", "send_result_drop"],
                "streamingMethods": ["send_progress", "send_partial_result"],
            },
            "ServerEvent": {
                "fields": [],
                "representation": "tagged-union",
                "variants": ["submit", "runtime", "lifecycle"],
                "variantTypes": {
                    "submit": "ServerOperation",
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
            "ResultPushMetadata": {
                "resultFlagBits": copy.deepcopy(
                    checker.EXPECTED_RESULT_PUSH_FLAG_BITS
                ),
                "validation": copy.deepcopy(
                    checker.EXPECTED_RESULT_PUSH_VALIDATION
                ),
            },
        },
        "roleMethodMessages": [
            {"messageType": name}
            for name in sorted(checker.EXPECTED_ROLE_METHOD_MESSAGES)
        ],
        "languageProjections": {
            "rust": copy.deepcopy(checker.EXPECTED_RUST_PROJECTIONS),
        },
        "roleSurfaces": {
            "clientSubmitWait": copy.deepcopy(checker.EXPECTED_CLIENT_SUBMIT_WAIT),
            "serverEventPump": copy.deepcopy(checker.EXPECTED_SERVER_EVENT_PUMP),
        },
        "roleOperations": {
            "client_session.next_event": {"returns": "ClientEvent"},
            "server_session.next_event": {"returns": "ServerEvent"},
            "server_session.receive_submit": {
                "returns": "ServerOperation",
                "selective": True,
                "retainsSkippedEvents": True,
            },
            "server_operation.send_result": {
                "parameters": [
                    {"name": "metadata", "type": "ResultPushMetadata", "required": True},
                    {"name": "body", "type": "bytes", "required": False},
                ],
                "returns": "void",
                "async": True,
                "terminal": True,
            },
            "server_operation.send_result_drop": {
                "parameters": [
                    {
                        "name": "metadata",
                        "type": "ResultDropReasonMetadata",
                        "required": True,
                    },
                    {"name": "diagnostic", "type": "bytes", "required": False},
                ],
                "returns": "void",
                "async": True,
                "terminal": True,
            },
            "server_operation.send_progress": {
                "parameters": [
                    {"name": "metadata", "type": "ProgressMetadata", "required": True},
                    {"name": "body", "type": "bytes", "required": False},
                ],
                "returns": "void",
                "async": True,
                "terminal": False,
            },
            "server_operation.send_partial_result": {
                "parameters": [
                    {
                        "name": "metadata",
                        "type": "PartialResultMetadata",
                        "required": True,
                    },
                    {"name": "body", "type": "bytes", "required": False},
                ],
                "returns": "void",
                "async": True,
                "terminal": False,
            },
        },
    }


class SdkApiContractTests(unittest.TestCase):
    def setUp(self):
        self.checker = load_checker()

    def test_rust_impl_body_preserves_methods_after_nested_blocks(self):
        source = """
impl NnrpServerOperation {
    pub fn first(&self) {
        if true {
            let _nested = 1;
        }
    }

    pub async fn send_result(&self) {}
}
"""
        body = self.checker.rust_impl_body(source, "NnrpServerOperation")
        self.assertIsNotNone(body)
        self.assertIn("pub async fn send_result", body)

    def check(self, contract):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "contract.json"
            path.write_text(json.dumps(contract), encoding="utf-8")
            self.checker.check_contract(path)

    def test_accepts_the_frozen_terminal_result_contract(self):
        self.check(frozen_contract())

    def test_rejects_transport_provider_name_semantics_drift(self):
        contract = frozen_contract()
        contract["types"]["TransportProviderDescriptor"]["nameSemantics"] = (
            "provider name is transport identity"
        )
        with self.assertRaisesRegex(
            SystemExit, "TransportProviderDescriptor name semantics drifted"
        ):
            self.check(contract)

    def test_rejects_data_plane_validation_drift(self):
        contract = frozen_contract()
        contract["wireLayouts"]["BodyRegionPrelude"]["validation"][
            "typedPayloadDescriptorBytesMultiple"
        ] = 16
        with self.assertRaisesRegex(
            SystemExit, "BodyRegionPrelude validation contract drifted"
        ):
            self.check(contract)

        contract = frozen_contract()
        contract["types"]["ResultPushMetadata"]["resultFlagBits"]["partial"] = 2
        with self.assertRaisesRegex(
            SystemExit, "ResultPushMetadata result flag assignments drifted"
        ):
            self.check(contract)

        contract = frozen_contract()
        contract["types"]["ResultPushMetadata"]["validation"][
            "tensorCoverageRule"
        ] = "coverage may be incomplete"
        with self.assertRaisesRegex(
            SystemExit, "ResultPushMetadata validation contract drifted"
        ):
            self.check(contract)

    def test_rejects_contract_version_and_role_surface_drift(self):
        contract = frozen_contract()
        contract["contractVersion"] = 14
        with self.assertRaisesRegex(SystemExit, "expected SDK contract version 15"):
            self.check(contract)

        contract = frozen_contract()
        contract["roleSurfaces"]["clientSubmitWait"]["timeoutRule"] = (
            "timeout remains local"
        )
        with self.assertRaisesRegex(SystemExit, "client submit-wait semantics drifted"):
            self.check(contract)

        contract = frozen_contract()
        contract["roleSurfaces"]["clientSubmitWait"] = None
        with self.assertRaisesRegex(SystemExit, "client submit-wait contract must be an object"):
            self.check(contract)

        contract = frozen_contract()
        contract["roleSurfaces"]["serverEventPump"]["ownershipRule"] = (
            "submit ownership may be discarded"
        )
        with self.assertRaisesRegex(SystemExit, "server event-pump semantics drifted"):
            self.check(contract)

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

    def test_rejects_client_event_or_operation_drift(self):
        contract = frozen_contract()
        contract["types"]["ClientEvent"]["representation"] = "untagged_union"
        with self.assertRaisesRegex(SystemExit, "ClientEvent is no longer a tagged union"):
            self.check(contract)

        contract = frozen_contract()
        contract["types"]["ClientEvent"]["variants"] = ["runtime"]
        with self.assertRaisesRegex(SystemExit, "ClientEvent variants drifted"):
            self.check(contract)

        contract = frozen_contract()
        contract["types"]["ClientEvent"]["variantTypes"]["lifecycle"] = (
            "RuntimeEvent"
        )
        with self.assertRaisesRegex(SystemExit, "ClientEvent variant types drifted"):
            self.check(contract)

        contract = frozen_contract()
        contract["roleOperations"]["client_session.next_event"]["returns"] = (
            "RuntimeEvent"
        )
        with self.assertRaisesRegex(SystemExit, "client next-event return type drifted"):
            self.check(contract)

    def test_rejects_server_event_or_selective_receive_drift(self):
        contract = frozen_contract()
        contract["types"]["ServerEvent"]["variants"] = ["runtime"]
        with self.assertRaisesRegex(SystemExit, "ServerEvent variants drifted"):
            self.check(contract)

        contract = frozen_contract()
        contract["roleOperations"]["server_session.receive_submit"][
            "retainsSkippedEvents"
        ] = False
        with self.assertRaisesRegex(SystemExit, "selective submit contract drifted"):
            self.check(contract)

    def test_rejects_server_operation_method_drift(self):
        contract = frozen_contract()
        contract["types"]["ServerOperation"]["terminalMethods"] = ["send_result"]
        with self.assertRaisesRegex(
            SystemExit, "ServerOperation terminal method contract drifted"
        ):
            self.check(contract)

        contract = frozen_contract()
        contract["roleOperations"]["server_operation.send_progress"]["terminal"] = True
        with self.assertRaisesRegex(
            SystemExit, "server_operation.send_progress role operation drifted"
        ):
            self.check(contract)

        contract = frozen_contract()
        contract["languageProjections"]["rust"]["roleMethods"][
            "server_operation.send_result"
        ] = "reply"
        with self.assertRaisesRegex(SystemExit, "Rust SDK projection map drifted"):
            self.check(contract)

        contract = frozen_contract()
        contract["types"]["ServerOperation"]["streamingMethods"] = [
            "send_partial_result"
        ]
        with self.assertRaisesRegex(
            SystemExit, "ServerOperation streaming method contract drifted"
        ):
            self.check(contract)

    def test_rejects_malformed_type_fields_without_a_traceback(self):
        contract = frozen_contract()
        contract["types"]["NnrpResult"] = ["operation_id"]
        with self.assertRaisesRegex(
            SystemExit, "NnrpResult SDK type contract must be an object"
        ):
            self.check(contract)

        contract = frozen_contract()
        contract["types"]["NnrpResult"]["fields"] = {"name": "operation_id"}
        with self.assertRaisesRegex(SystemExit, "SDK type fields must be an array"):
            self.check(contract)

        contract = frozen_contract()
        del contract["types"]["NnrpResult"]["fields"][0]["name"]
        with self.assertRaisesRegex(SystemExit, "must declare a non-empty name"):
            self.check(contract)

    def test_rejects_non_object_required_type_contracts_without_a_traceback(self):
        for type_name in ("TerminalEvent", "RuntimeEventMetadata"):
            with self.subTest(type_name=type_name):
                contract = frozen_contract()
                contract["types"][type_name] = ["invalid"]
                with self.assertRaisesRegex(
                    SystemExit,
                    f"{type_name} SDK type contract must be an object",
                ):
                    self.check(contract)

    def test_required_type_contract_diagnostics_use_frozen_order(self):
        contract = frozen_contract()
        for type_name in (
            "OperationLifecycleEvent",
            "TerminalEvent",
            "NnrpResult",
            "RuntimeEventMetadata",
            "ServerEvent",
            "ServerOperation",
            "SessionRecoveryTicket",
        ):
            contract["types"][type_name] = ["invalid"]
        with self.assertRaisesRegex(
            SystemExit,
            "OperationLifecycleEvent SDK type contract must be an object",
        ):
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

    def test_rejects_native_lifecycle_projection_drift(self):
        contract = frozen_contract()
        contract["types"]["OperationLifecycleEvent"]["nativeEventProjection"][
            "eventKindCode"
        ] = 13
        with self.assertRaisesRegex(
            SystemExit, "OperationLifecycleEvent native projection drifted"
        ):
            self.check(contract)

    def test_rejects_rust_projection_drift(self):
        contract = frozen_contract()
        contract["languageProjections"]["rust"]["terminalEvent"] = "LegacyEvent"
        with self.assertRaisesRegex(SystemExit, "Rust SDK projection map drifted"):
            self.check(contract)

        contract = frozen_contract()
        del contract["languageProjections"]["rust"]["baselineMetadataCodecs"][
            "ObjectReferenceBlock"
        ]
        with self.assertRaisesRegex(SystemExit, "Rust SDK projection map drifted"):
            self.check(contract)

    def test_rejects_missing_rust_baseline_metadata_codec_surface(self):
        source = """
pub struct ExampleMetadata;
impl ExampleMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, Error> { todo!() }
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> { todo!() }
}
"""
        self.checker.require_rust_baseline_metadata_codec(source, "ExampleMetadata")

        without_parse = source.replace("pub fn parse", "fn parse")
        with self.assertRaisesRegex(
            SystemExit, "Rust baseline metadata codec ExampleMetadata::parse is missing"
        ):
            self.checker.require_rust_baseline_metadata_codec(
                without_parse, "ExampleMetadata"
            )

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
