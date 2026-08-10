from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any


EXPECTED_CONTRACT_VERSION = 13
EXPECTED_API_DOMAINS = {
    "submission",
    "runtimeEvents",
    "lifecycle",
    "capability",
    "cache",
    "schema",
    "transport",
    "roles",
}
EXPECTED_RUNTIME_EVENT_METADATA_VARIANTS = [
    "none",
    "frame_submit",
    "result_push",
    "result_hint",
    "control_request",
    "scheduling",
    "supersede",
    "budget",
    "progress",
    "partial_result",
    "pressure",
    "capability",
    "route_hint",
    "trace_context",
    "result_drop_reason",
    "recoverable_error",
    "retry_after",
    "flow_update",
    "object_descriptor",
    "object_reference",
    "object_release",
    "object_delta",
    "cache_reference",
    "cache_miss",
    "cache_invalidate",
    "session_close",
]
EXPECTED_ROLE_METHOD_MESSAGES = {
    "client_hello",
    "server_hello_ack",
    "session_patch",
    "session_patch_ack",
    "session_open",
    "session_open_ack",
    "session_close_ack",
    "cache_put",
    "cache_ack",
    "transport_probe",
    "transport_probe_ack",
    "session_migrate",
    "session_migrate_ack",
    "ping",
    "pong",
}
EXPECTED_NATIVE_LIFECYCLE_PROJECTION = {
    "eventKind": "operation_lifecycle",
    "eventKindCode": 14,
    "headerPresent": 0,
    "payloadBytes": 1,
    "payloadLayout": [
        {
            "name": "state",
            "type": "OperationState",
            "wireType": "u8",
            "offset": 0,
        }
    ],
    "operationIdentity": (
        "diagnostic.related_operation_id and the operation handle, "
        "when the handle remains live"
    ),
    "ownership": (
        "the one-byte state payload follows the same payload_owner lifetime as "
        "wire-event payloads"
    ),
}
EXPECTED_RUST_PROJECTIONS = {
    "submitRequest": "nnrp_runtime::NnrpSubmitRequest",
    "submitHeaderContext": "nnrp_runtime::NnrpSubmitHeaderContext",
    "submitBuilders": [
        "NnrpSubmitRequest::tensor",
        "NnrpSubmitRequest::token",
        "NnrpSubmitRequest::typed_payload",
    ],
    "runtimeFrameHeader": "nnrp_runtime::RuntimeFrameHeader",
    "runtimeEvent": "nnrp_runtime::NnrpRuntimeEvent",
    "clientEvent": "nnrp_runtime::NnrpClientRoleEvent",
    "serverEvent": "nnrp_runtime::NnrpServerEvent",
    "serverOperation": "nnrp_runtime::NnrpServerOperation",
    "operationLifecycleEvent": "nnrp_runtime::OperationLifecycleEvent",
    "terminalEvent": "nnrp_runtime::NnrpTerminalEvent",
    "result": "nnrp_runtime::NnrpResult",
    "clientRoles": ["nnrp_runtime::NnrpClient", "nnrp_runtime::NnrpClientSession"],
    "serverRoles": ["nnrp_runtime::NnrpServer", "nnrp_runtime::NnrpServerSession"],
    "runtimeMetadataNamespace": "nnrp_core",
    "capabilityMetadata": "nnrp_core::CapabilityMetadata",
    "connectionLifecycle": "nnrp_core::ConnectionLifecycle",
    "sessionLifecycle": "nnrp_core::SessionLifecycle",
    "typedPayloadDescriptor": "nnrp_core::TypedPayloadDescriptor",
    "typedPayloadFrame": "nnrp_core::TypedPayloadFrameView",
    "cacheObjectId": "nnrp_core::CacheObjectId",
    "cacheLease": "nnrp_core::CacheLease",
    "cacheLeaseResult": "nnrp_runtime::CacheLeaseResult",
    "cachePolicyOptions": "nnrp_runtime::CachePolicyOptions",
    "transportProviderMetadata": "nnrp_transport_provider::TransportProviderMetadata",
    "transportProviderDescriptor": "nnrp_transport_provider::TransportProviderDescriptor",
    "transportSelectionOptions": "nnrp_transport_provider::TransportSelectionOptions",
    "transportSelection": "nnrp_transport_provider::TransportSelection",
    "transportSelectionFailure": "nnrp_transport_provider::TransportSelectionError",
    "applicationEndpoint": "nnrp_runtime::NnrpEndpoint",
    "providerEndpoint": "nnrp_runtime::ProviderEndpoint",
    "clientTransportSecurity": "nnrp_runtime::ClientTransportSecurity",
    "serverTransportSecurity": "nnrp_runtime::ServerTransportSecurity",
    "clientProviderRoute": "nnrp_runtime::ClientProviderRoute",
    "serverProviderRoute": "nnrp_runtime::ServerProviderRoute",
    "schemaDescriptor": "nnrp_core::SchemaDescriptorHeader",
    "schemaRegistry": "nnrp_core::SchemaRegistry",
    "clientBootstrapOptions": "nnrp_runtime::NnrpClientOptions",
    "clientSessionOptions": "nnrp_runtime::NnrpClientConfig",
    "sessionRecoveryTicket": "nnrp_runtime::NnrpSessionRecoveryTicket",
    "sessionRecoveryTicketEncode": "NnrpSessionRecoveryTicket::to_bytes",
    "sessionRecoveryTicketDecode": "NnrpSessionRecoveryTicket::from_bytes",
    "serverBootstrapOptions": "nnrp_runtime::NnrpServerOptions",
    "serverSessionOptions": "nnrp_runtime::NnrpServerConfig",
    "serverAcceptOptions": "nnrp_runtime::NnrpServerAcceptOptions",
    "serverSessionPolicy": "nnrp_runtime::NnrpServerPolicy",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def require_mapping(value: Any, message: str) -> dict[str, Any]:
    require(isinstance(value, dict), message)
    return value


def require_list(value: Any, message: str) -> list[Any]:
    require(isinstance(value, list), message)
    return value


def field_shape(type_contract: Any) -> list[tuple[str, str, bool]]:
    contract = require_mapping(type_contract, "SDK type contract must be an object")
    fields = require_list(contract.get("fields"), "SDK type fields must be an array")
    shape = []
    for index, value in enumerate(fields):
        field = require_mapping(value, f"SDK type field {index} must be an object")
        name = field.get("name")
        field_type = field.get("type")
        required = field.get("required", False)
        require(
            isinstance(name, str) and name,
            f"SDK type field {index} must declare a non-empty name",
        )
        require(
            isinstance(field_type, str) and field_type,
            f"SDK type field {index} must declare a non-empty type",
        )
        require(
            isinstance(required, bool),
            f"SDK type field {index} required must be a boolean",
        )
        shape.append((name, field_type, required))
    return shape


def check_contract(contract_path: Path) -> None:
    contract = json.loads(contract_path.read_text(encoding="utf-8"))
    require(
        contract.get("contractVersion") == EXPECTED_CONTRACT_VERSION,
        f"expected SDK contract version {EXPECTED_CONTRACT_VERSION}",
    )

    types = require_mapping(contract.get("types"), "SDK contract types must be an object")
    required_type_names = (
        "OperationLifecycleEvent",
        "ClientEvent",
        "TerminalEvent",
        "NnrpResult",
        "RuntimeEventMetadata",
        "ServerEvent",
        "ServerOperation",
        "SessionRecoveryTicket",
    )
    require(
        set(required_type_names).issubset(types),
        "SDK contract is missing required type contracts",
    )
    type_contracts = {
        name: require_mapping(
            types[name], f"{name} SDK type contract must be an object"
        )
        for name in required_type_names
    }
    lifecycle = type_contracts["OperationLifecycleEvent"]
    require(
        field_shape(lifecycle)
        == [("operation_id", "u64", True), ("state", "OperationState", True)],
        "OperationLifecycleEvent field contract drifted",
    )
    require(
        lifecycle.get("terminalMapping")
        == {
            "completed": "success",
            "cancelled": "cancelled",
            "superseded": "dropped",
            "failed": "error",
        },
        "OperationLifecycleEvent terminal mapping drifted",
    )
    require(
        lifecycle.get("nativeEventProjection")
        == EXPECTED_NATIVE_LIFECYCLE_PROJECTION,
        "OperationLifecycleEvent native projection drifted",
    )

    client_event = type_contracts["ClientEvent"]
    require(
        client_event.get("representation") == "tagged-union",
        "ClientEvent is no longer a tagged union",
    )
    require(
        client_event.get("variants") == ["runtime", "lifecycle"],
        "ClientEvent variants drifted",
    )
    require(
        client_event.get("variantTypes")
        == {"runtime": "RuntimeEvent", "lifecycle": "OperationLifecycleEvent"},
        "ClientEvent variant types drifted",
    )

    terminal = type_contracts["TerminalEvent"]
    require(
        terminal.get("representation") == "tagged-union",
        "TerminalEvent is no longer a tagged union",
    )
    require(
        terminal.get("variants") == ["runtime", "lifecycle"],
        "TerminalEvent variants drifted",
    )
    require(
        terminal.get("variantTypes")
        == {"runtime": "RuntimeEvent", "lifecycle": "OperationLifecycleEvent"},
        "TerminalEvent variant types drifted",
    )

    server_operation = type_contracts["ServerOperation"]
    require(
        field_shape(server_operation)
        == [
            ("operation_id", "u64", True),
            ("frame_id", "u32", True),
            ("submit", "RuntimeEvent", True),
        ],
        "ServerOperation field contract drifted",
    )

    server_event = type_contracts["ServerEvent"]
    require(
        server_event.get("representation") == "tagged-union",
        "ServerEvent is no longer a tagged union",
    )
    require(
        server_event.get("variants") == ["submit", "runtime", "lifecycle"],
        "ServerEvent variants drifted",
    )
    require(
        server_event.get("variantTypes")
        == {
            "submit": "ServerOperation",
            "runtime": "RuntimeEvent",
            "lifecycle": "OperationLifecycleEvent",
        },
        "ServerEvent variant types drifted",
    )

    result = type_contracts["NnrpResult"]
    require(
        field_shape(result)
        == [
            ("operation_id", "u64", True),
            ("terminal_state", "ResultTerminalState", True),
            ("event", "TerminalEvent", True),
        ],
        "NnrpResult field contract drifted",
    )

    recovery_ticket = type_contracts["SessionRecoveryTicket"]
    require(
        field_shape(recovery_ticket)
        == [
            ("session_id", "u32", True),
            ("resume_token", "bytes", True),
            ("resume_from_operation_id", "u64?", False),
            ("resume_window_ms", "u32", True),
        ],
        "SessionRecoveryTicket field contract drifted",
    )
    require(
        recovery_ticket.get("opaqueEncoding")
        == {
            "name": "NRTK",
            "version": 1,
            "byteOrder": "little-endian",
            "fixedPrefixBytes": 28,
            "fields": [
                {"name": "magic", "type": "bytes[4]", "offset": 0, "constant": "NRTK"},
                {"name": "version", "type": "u16", "offset": 4, "constant": 1},
                {"name": "flags", "type": "u16", "offset": 6},
                {"name": "session_id", "type": "u32", "offset": 8},
                {"name": "resume_token_bytes", "type": "u32", "offset": 12},
                {"name": "resume_window_ms", "type": "u32", "offset": 16},
                {"name": "resume_from_operation_id", "type": "u64", "offset": 20},
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
        "SessionRecoveryTicket opaque encoding drifted",
    )

    api_domains = require_mapping(
        contract.get("apiDomains"), "SDK API domains must be an object"
    )
    require(
        set(api_domains) == EXPECTED_API_DOMAINS,
        "SDK API domain set drifted",
    )
    language_projections = require_mapping(
        contract.get("languageProjections"),
        "SDK language projections must be an object",
    )
    rust_projection = require_mapping(
        language_projections.get("rust"), "Rust SDK projection must be an object"
    )
    require(
        rust_projection == EXPECTED_RUST_PROJECTIONS,
        "Rust SDK projection map drifted; update the implementation contract test with the frozen API",
    )
    require(
        type_contracts["RuntimeEventMetadata"].get("variants")
        == EXPECTED_RUNTIME_EVENT_METADATA_VARIANTS,
        "RuntimeEventMetadata closed variant set drifted",
    )
    role_surfaces = require_mapping(
        contract.get("roleSurfaces"), "SDK role surfaces must be an object"
    )
    server_event_pump = require_mapping(
        role_surfaces.get("serverEventPump"),
        "server event pump contract must be an object",
    )
    require(
        server_event_pump.get("canonicalOperation") == "server_session.next_event"
        and server_event_pump.get("submitConvenience")
        == "server_session.receive_submit",
        "server event-pump operation names drifted",
    )
    role_operations = require_mapping(
        contract.get("roleOperations"), "SDK role operations must be an object"
    )
    require(
        require_mapping(
            role_operations.get("client_session.next_event"),
            "client next-event operation must be an object",
        ).get("returns")
        == "ClientEvent",
        "client next-event return type drifted",
    )
    require(
        require_mapping(
            role_operations.get("server_session.next_event"),
            "server next-event operation must be an object",
        ).get("returns")
        == "ServerEvent",
        "server next-event return type drifted",
    )
    receive_submit = require_mapping(
        role_operations.get("server_session.receive_submit"),
        "server receive-submit operation must be an object",
    )
    require(
        receive_submit.get("returns") == "ServerOperation"
        and receive_submit.get("selective") is True
        and receive_submit.get("retainsSkippedEvents") is True,
        "server selective submit contract drifted",
    )
    role_method_messages = require_list(
        contract.get("roleMethodMessages"),
        "SDK role-method messages must be an array",
    )
    message_types: list[str] = []
    for index, entry in enumerate(role_method_messages):
        entry = require_mapping(
            entry, f"SDK role-method message {index} must be an object"
        )
        message_type = entry.get("messageType")
        require(
            isinstance(message_type, str) and bool(message_type),
            f"SDK role-method message {index} must declare a non-empty messageType",
        )
        message_types.append(message_type)
    require(
        len(message_types) == len(set(message_types)),
        "SDK role-method message types must be unique",
    )
    require(
        set(message_types) == EXPECTED_ROLE_METHOD_MESSAGES,
        "dedicated role-method message set drifted",
    )

    repository_root = Path(__file__).resolve().parent.parent
    runtime_event_source = (
        repository_root / "crates" / "nnrp-runtime" / "src" / "event.rs"
    ).read_text(encoding="utf-8")
    runtime_client_source = (
        repository_root / "crates" / "nnrp-runtime" / "src" / "client.rs"
    ).read_text(encoding="utf-8")
    ffi_source = (
        repository_root / "crates" / "nnrp-ffi" / "src" / "lib.rs"
    ).read_text(encoding="utf-8")
    wasm_source = (
        repository_root / "crates" / "nnrp-wasm" / "src" / "browser_role.rs"
    ).read_text(encoding="utf-8")
    require(
        "pub enum NnrpClientRoleEvent" in runtime_event_source
        and "Runtime(NnrpRuntimeEvent)" in runtime_event_source
        and "Lifecycle(OperationLifecycleEvent)" in runtime_event_source,
        "Rust client role event union implementation drifted",
    )
    require(
        re.search(
            r"pub\s+async\s+fn\s+await_event\s*\(\s*&mut\s+self\s*\)\s*"
            r"->\s*Result\s*<\s*NnrpClientRoleEvent\s*,\s*RuntimeError\s*>",
            runtime_client_source,
        )
        is not None,
        "NnrpClientSession::await_event no longer returns the frozen client event union",
    )
    require(
        "NnrpClientRoleEvent::Lifecycle(event)" in ffi_source
        and "role_lifecycle_event(scope, connection, event)" in ffi_source,
        "native client lifecycle projection implementation drifted",
    )
    require(
        "event_kind: 14" in wasm_source
        and "header_present: 0" in wasm_source
        and "related_operation_id: event.operation_id" in wasm_source
        and "operation_state: Some(event.state as u8)" in wasm_source,
        "browser WASM client lifecycle projection implementation drifted",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", type=Path, required=True)
    args = parser.parse_args()
    check_contract(args.contract)


if __name__ == "__main__":
    main()
