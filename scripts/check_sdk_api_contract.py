from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any


EXPECTED_CONTRACT_VERSION = 15
EXPECTED_BODY_REGION_VALIDATION = {
    "objectReferenceBlockBytesMultiple": 24,
    "typedPayloadDescriptorBytesMultiple": 24,
    "extensionDescriptorBytesMultiple": 16,
    "typedPayloadDescriptorCountRule": (
        "typed_payload_descriptor_bytes equals payload_frame_count * 24 for "
        "FRAME_SUBMIT and RESULT_PUSH"
    ),
}
EXPECTED_RESULT_PUSH_FLAG_BITS = {"stale": 1, "fallback": 2, "partial": 4}
EXPECTED_RESULT_PUSH_VALIDATION = {
    "staleReuseRule": (
        "(result_class is stale_reuse or result_flags contains stale) if and only if "
        "reused_frame_id is non-zero"
    ),
    "tensorPartialRule": (
        "(result_class is partial or result_flags contains partial) requires dropped_tile_count "
        "greater than zero for tensor payloads"
    ),
    "tensorCoverageRule": (
        "covered_tile_count plus dropped_tile_count equals tile_count for tensor payloads"
    ),
    "nonTensorCoverageRule": (
        "section_count, tile_count, tile_base_id, tile_index_bytes, covered_tile_count, "
        "and dropped_tile_count are zero when payload_kind_bitmap contains no tensor payload"
    ),
}
EXPECTED_CLIENT_SUBMIT_WAIT = {
    "scopeRule": (
        "These rules apply when an SDK exposes a cancellable or time-bounded "
        "submit-and-wait convenience."
    ),
    "preDispatchCancellationRule": (
        "Cancellation before FRAME_SUBMIT dispatch fails the local wait and emits no submit "
        "or cancellation frame."
    ),
    "postDispatchCancellationRule": (
        "Cancellation after FRAME_SUBMIT dispatch fails the local wait with the language-native "
        "cancellation error and sends CANCEL for the submitted operation."
    ),
    "timeoutRule": (
        "A time-bounded submit wait sends DEADLINE before dispatch; expiry fails the local wait "
        "with the language-native timeout error and sends CANCEL for the submitted operation."
    ),
    "lifecycleRule": (
        "The local lifecycle event produced by caller cancellation or wait expiry remains "
        "observable through the client event pump and must not race the same submit wait into a "
        "successful NnrpResult return. A terminal lifecycle initiated independently by the peer "
        "may complete the submit wait as NnrpResult evidence."
    ),
}
EXPECTED_SERVER_EVENT_PUMP = {
    "canonicalOperation": "server_session.next_event",
    "submitConvenience": "server_session.receive_submit",
    "orderingRule": "next_event delivers every server event in per-session wire order without filtering",
    "submitRule": (
        "receive_submit is a selective convenience that may skip non-submit events only by retaining "
        "them in the same session queue; it must never discard, decode-and-forget, or acknowledge them"
    ),
    "ownershipRule": (
        "a FRAME_SUBMIT event becomes one ServerOperation before it is exposed to the application, "
        "so consuming the canonical event pump never loses the reply capability"
    ),
    "concurrencyRule": (
        "one session has one serialized receive source; concurrent receive calls are rejected or "
        "serialized and never race the native event queue"
    ),
}
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
    "roleMethods": {
        "client.open_session": "open_session",
        "client.resume_session": "resume_session",
        "client_session.recovery_ticket": "recovery_ticket",
        "client_session.next_event": "await_event",
        "server.accept": "accept",
        "server_session.next_event": "await_event",
        "server_session.receive_submit": "receive_submit",
        "server_operation.send_result": "send_result",
        "server_operation.send_result_drop": "send_result_drop",
        "server_operation.send_progress": "send_progress",
        "server_operation.send_partial_result": "send_partial_result",
    },
    "serverCapabilityMethods": {
        "negotiate_capabilities": "send_capability",
        "degrade_profile": "send_capability",
    },
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
    "baselineMetadataCodecs": {
        "ClientHelloMetadata": [
            "ClientHelloMetadata::to_bytes",
            "ClientHelloMetadata::parse",
        ],
        "SessionPatchAckMetadata": [
            "SessionPatchAckMetadata::to_bytes",
            "SessionPatchAckMetadata::parse",
        ],
        "FlowUpdateMetadata": [
            "FlowUpdateMetadata::to_bytes",
            "FlowUpdateMetadata::parse",
        ],
        "ResultHintMetadata": [
            "ResultHintMetadata::to_bytes",
            "ResultHintMetadata::parse",
        ],
        "FrameSubmitMetadata": [
            "FrameSubmitMetadata::to_bytes",
            "FrameSubmitMetadata::parse",
        ],
        "ResultPushMetadata": [
            "ResultPushMetadata::to_bytes",
            "ResultPushMetadata::parse",
        ],
        "CachePutMetadata": [
            "CachePutMetadata::to_bytes",
            "CachePutMetadata::parse",
        ],
        "CacheAckMetadata": [
            "CacheAckMetadata::to_bytes",
            "CacheAckMetadata::parse",
        ],
        "CacheInvalidateMetadata": [
            "CacheInvalidateMetadata::to_bytes",
            "CacheInvalidateMetadata::parse",
        ],
        "TransportProbeMetadata": [
            "TransportProbeMetadata::to_bytes",
            "TransportProbeMetadata::parse",
        ],
        "TransportProbeAckMetadata": [
            "TransportProbeAckMetadata::to_bytes",
            "TransportProbeAckMetadata::parse",
        ],
        "ObjectReferenceBlock": [
            "ObjectReferenceBlock::to_bytes",
            "ObjectReferenceBlock::parse",
        ],
    },
}

RUST_BASELINE_METADATA_CODEC_SOURCES = {
    "ClientHelloMetadata": "control",
    "SessionPatchAckMetadata": "control",
    "FlowUpdateMetadata": "flow",
    "ResultHintMetadata": "control",
    "FrameSubmitMetadata": "data",
    "ResultPushMetadata": "data",
    "CachePutMetadata": "cache",
    "CacheAckMetadata": "cache",
    "CacheInvalidateMetadata": "cache",
    "TransportProbeMetadata": "control",
    "TransportProbeAckMetadata": "control",
    "ObjectReferenceBlock": "data",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def require_mapping(value: Any, message: str) -> dict[str, Any]:
    require(isinstance(value, dict), message)
    return value


def rust_block_body(source: str, opening_brace: int) -> str | None:
    depth = 0
    for index in range(opening_brace, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[opening_brace + 1 : index]
    return None


def rust_impl_body(source: str, type_name: str) -> str | None:
    match = re.search(rf"\bimpl\s+{re.escape(type_name)}\s*\{{", source)
    if match is None:
        return None
    return rust_block_body(source, source.find("{", match.start()))


def rust_function_body(source: str, function_name: str) -> str | None:
    match = re.search(
        rf"\b(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+"
        rf"{re.escape(function_name)}\s*\(",
        source,
    )
    if match is None:
        return None
    opening_brace = source.find("{", match.end())
    if opening_brace < 0:
        return None
    return rust_block_body(source, opening_brace)


def require_rust_baseline_metadata_codec(source: str, type_name: str) -> None:
    require(
        re.search(rf"\bpub\s+struct\s+{re.escape(type_name)}\b", source) is not None,
        f"Rust baseline metadata codec {type_name} public type is missing",
    )
    impl_body = rust_impl_body(source, type_name)
    require(
        impl_body is not None,
        f"Rust baseline metadata codec {type_name} implementation is missing",
    )
    for method in ("to_bytes", "parse"):
        require(
            re.search(rf"\bpub\s+fn\s+{method}\s*\(", impl_body) is not None,
            f"Rust baseline metadata codec {type_name}::{method} is missing",
        )


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

    wire_layouts = require_mapping(
        contract.get("wireLayouts"), "SDK wire layouts must be an object"
    )
    body_region_prelude = require_mapping(
        wire_layouts.get("BodyRegionPrelude"),
        "BodyRegionPrelude wire layout must be an object",
    )
    require(
        body_region_prelude.get("validation") == EXPECTED_BODY_REGION_VALIDATION,
        "BodyRegionPrelude validation contract drifted",
    )

    types = require_mapping(contract.get("types"), "SDK contract types must be an object")
    provider_descriptor = require_mapping(
        types.get("TransportProviderDescriptor"),
        "TransportProviderDescriptor SDK type contract must be an object",
    )
    require(
        provider_descriptor.get("nameSemantics")
        == (
            "provider-owned package or display name; protocol transport identity is "
            "transport_id and selection must not derive it from name"
        ),
        "TransportProviderDescriptor name semantics drifted",
    )
    result_push = require_mapping(
        types.get("ResultPushMetadata"),
        "ResultPushMetadata SDK type contract must be an object",
    )
    require(
        result_push.get("resultFlagBits") == EXPECTED_RESULT_PUSH_FLAG_BITS,
        "ResultPushMetadata result flag assignments drifted",
    )
    require(
        result_push.get("validation") == EXPECTED_RESULT_PUSH_VALIDATION,
        "ResultPushMetadata validation contract drifted",
    )
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
    require(
        server_operation.get("terminalMethods") == ["send_result", "send_result_drop"],
        "ServerOperation terminal method contract drifted",
    )
    require(
        server_operation.get("streamingMethods")
        == ["send_progress", "send_partial_result"],
        "ServerOperation streaming method contract drifted",
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
    client_submit_wait = require_mapping(
        role_surfaces.get("clientSubmitWait"),
        "client submit-wait contract must be an object",
    )
    require(
        client_submit_wait == EXPECTED_CLIENT_SUBMIT_WAIT,
        "client submit-wait semantics drifted",
    )
    server_event_pump = require_mapping(
        role_surfaces.get("serverEventPump"),
        "server event pump contract must be an object",
    )
    require(
        server_event_pump == EXPECTED_SERVER_EVENT_PUMP,
        "server event-pump semantics drifted",
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
    expected_operation_methods = {
        "server_operation.send_result": (
            [("metadata", "ResultPushMetadata", True), ("body", "bytes", False)],
            True,
        ),
        "server_operation.send_result_drop": (
            [
                ("metadata", "ResultDropReasonMetadata", True),
                ("diagnostic", "bytes", False),
            ],
            True,
        ),
        "server_operation.send_progress": (
            [("metadata", "ProgressMetadata", True), ("body", "bytes", False)],
            False,
        ),
        "server_operation.send_partial_result": (
            [
                ("metadata", "PartialResultMetadata", True),
                ("body", "bytes", False),
            ],
            False,
        ),
    }
    for operation_name, (parameters, terminal) in expected_operation_methods.items():
        operation = require_mapping(
            role_operations.get(operation_name),
            f"{operation_name} role operation must be an object",
        )
        actual_parameters = []
        for index, parameter in enumerate(
            require_list(
                operation.get("parameters"),
                f"{operation_name} parameters must be an array",
            )
        ):
            parameter = require_mapping(
                parameter,
                f"{operation_name} parameter {index} must be an object",
            )
            actual_parameters.append(
                (
                    parameter.get("name"),
                    parameter.get("type"),
                    parameter.get("required"),
                )
            )
        require(
            actual_parameters == parameters
            and operation.get("returns") == "void"
            and operation.get("async") is True
            and operation.get("terminal") is terminal,
            f"{operation_name} role operation drifted",
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
    data_source = (
        repository_root / "crates" / "nnrp-core" / "src" / "data.rs"
    ).read_text(encoding="utf-8")
    control_source = (
        repository_root / "crates" / "nnrp-core" / "src" / "control.rs"
    ).read_text(encoding="utf-8")
    flow_source = (
        repository_root / "crates" / "nnrp-core" / "src" / "flow.rs"
    ).read_text(encoding="utf-8")
    cache_source = (
        repository_root / "crates" / "nnrp-core" / "src" / "cache.rs"
    ).read_text(encoding="utf-8")
    runtime_client_source = (
        repository_root / "crates" / "nnrp-runtime" / "src" / "client.rs"
    ).read_text(encoding="utf-8")
    runtime_server_source = (
        repository_root / "crates" / "nnrp-runtime" / "src" / "server.rs"
    ).read_text(encoding="utf-8")
    ffi_source = (
        repository_root / "crates" / "nnrp-ffi" / "src" / "lib.rs"
    ).read_text(encoding="utf-8")
    wasm_source = (
        repository_root / "crates" / "nnrp-wasm" / "src" / "browser_role.rs"
    ).read_text(encoding="utf-8")
    core_sources = {
        "control": control_source,
        "flow": flow_source,
        "data": data_source,
        "cache": cache_source,
    }
    for type_name, source_name in RUST_BASELINE_METADATA_CODEC_SOURCES.items():
        require_rust_baseline_metadata_codec(core_sources[source_name], type_name)
    require(
        "pub const EXTENSION_FRAME_DESCRIPTOR_LEN: usize = 16;" in data_source
        and "self.typed_payload_descriptor_bytes as usize % TYPED_PAYLOAD_DESCRIPTOR_LEN"
        in data_source
        and "self.extension_descriptor_bytes as usize % EXTENSION_FRAME_DESCRIPTOR_LEN"
        in data_source,
        "Rust BodyRegionPrelude validation implementation drifted",
    )
    require(
        "self.result_class == ResultClass::StaleReuse" in data_source
        and "is_stale != (self.reused_frame_id != 0)" in data_source
        and "self.result_class == ResultClass::Partial" in data_source
        and "u32::from(self.covered_tile_count) + u32::from(self.dropped_tile_count)"
        in data_source,
        "Rust ResultPushMetadata validation implementation drifted",
    )
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
    operation_body = rust_impl_body(runtime_server_source, "NnrpServerOperation")
    require(operation_body is not None, "Rust server operation implementation is missing")
    operation_signatures = {
        "send_result": "ResultPushMetadata",
        "send_result_drop": "ResultDropReasonMetadata",
        "send_progress": "ProgressMetadata",
        "send_partial_result": "PartialResultMetadata",
    }
    for method, metadata_type in operation_signatures.items():
        require(
            re.search(
                rf"pub\s+async\s+fn\s+{method}\s*\(\s*&self\s*,\s*"
                rf"session\s*:\s*&mut\s+NnrpServerSession\s*,\s*"
                rf"metadata\s*:\s*{metadata_type}\s*,\s*"
                rf"(?:body|diagnostic)\s*:\s*Vec\s*<\s*u8\s*>\s*,?\s*\)\s*"
                rf"->\s*Result\s*<\s*\(\)\s*,\s*RuntimeError\s*>",
                operation_body,
                re.DOTALL,
            )
            is not None,
            f"NnrpServerOperation::{method} signature drifted",
        )
    require(
        re.search(
            r"#\[derive\([^\]]*\bClone\b[^\]]*\)\]\s*"
            r"pub\s+struct\s+NnrpServerOperation",
            runtime_server_source,
        )
        is None,
        "NnrpServerOperation must not be clonable",
    )
    _, session_impl_marker, session_impl = runtime_server_source.partition(
        "impl NnrpServerSession"
    )
    require(
        bool(session_impl_marker),
        "Rust server session implementation is missing",
    )
    capability_method = re.search(
        r"pub\s+async\s+fn\s+send_capability\s*\(\s*&mut\s+self\s*,\s*"
        r"message_type\s*:\s*MessageType\s*,\s*"
        r"metadata\s*:\s*CapabilityMetadata\s*,\s*"
        r"body\s*:\s*Vec\s*<\s*u8\s*>\s*,?\s*\)\s*"
        r"->\s*Result\s*<\s*\(\)\s*,\s*RuntimeError\s*>",
        session_impl,
        re.DOTALL,
    )
    capability_method_body = rust_function_body(session_impl, "send_capability")
    require(
        capability_method is not None
        and capability_method_body is not None
        and all(
            f"MessageType::{message_type}" in capability_method_body
            for message_type in ("CapabilityNegotiation", "DegradeProfile")
        ),
        "NnrpServerSession::send_capability no longer implements the frozen server capability surface",
    )
    for method in ("send_result", "send_result_drop", "send_progress", "send_partial_result"):
        require(
            re.search(rf"pub\s+async\s+fn\s+{method}\s*\(", session_impl)
            is None,
            f"NnrpServerSession::{method} bypasses frozen operation ownership",
        )
    for method in (
        "send_operation_result_for_binding",
        "send_operation_result_drop_for_binding",
        "send_operation_progress_for_binding",
        "send_operation_partial_result_for_binding",
    ):
        require(
            re.search(rf"pub\s+async\s+fn\s+{method}\s*\(", session_impl) is None,
            f"NnrpServerSession::{method} is publicly callable",
        )
        require(
            re.search(rf"pub\(crate\)\s+async\s+fn\s+{method}\s*\(", session_impl)
            is not None,
            f"NnrpServerSession::{method} is no longer crate-internal",
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
