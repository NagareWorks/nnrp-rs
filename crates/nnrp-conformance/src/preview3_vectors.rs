use nnrp_core::{
    token_delta_schema_descriptor, validate_cache_dependencies, validate_profile_assignment,
    validate_session_recovery_ack, validate_session_recovery_request, BackpressureLevel,
    CacheDependency, CacheDependencyState, CacheLease, CacheLeaseOwnerScope, CacheObjectId,
    CacheObjectKind, CacheValidationFailure, CancelScope, ClientHelloMetadata, CommonHeader,
    ConnectionLifecycle, FlowScopeKind, FlowUpdateMetadata, FlowUpdateReason, FrameSubmitMetadata,
    InFlightPolicy, MessageType, OperationCancelRequest, OperationDescriptor, OperationRegistry,
    OperationState, PayloadFamily, PayloadKindBitmap, ResultPushMetadata, SchemaRegistry,
    SchemaRegistryFailure, ServerHelloAckMetadata, SessionCloseAckMetadata, SessionCloseMetadata,
    SessionCloseReason, SessionCloseStatus, SessionOpenAckMetadata, SessionOpenMetadata,
    SessionPriorityClass, SessionRecoveryOutcome, SessionStatus, TransportId,
    TypedPayloadDescriptor, TypedPayloadRegion, FLOW_UPDATE_FLAG_CREDIT_VALID,
    FLOW_UPDATE_FLAG_RETRY_AFTER_VALID, FLOW_UPDATE_METADATA_LEN, FRAME_SUBMIT_METADATA_LEN,
    PROFILE_TENSOR, PROFILE_TOKEN, SESSION_ACK_FLAG_RESUME_ENABLED, SESSION_ERROR_NONE,
    SESSION_ERROR_RESUME_REJECTED, SESSION_FLAG_ALLOW_RESUME, SESSION_OPEN_ACK_METADATA_LEN,
    SESSION_OPEN_METADATA_LEN, STREAM_SEMANTICS_TOKEN_DELTA, TOKEN_DELTA_SCHEMA_ID,
    TOKEN_DELTA_SCHEMA_VERSION,
};
use nnrp_transport_provider::{
    select_transport_with_probe, ProbeSample, RemoteTransportSupport, TransportPolicy,
    TransportProviderKind, TransportProviderRegistry,
};
use nnrp_transport_quic::QuicProvider;
use nnrp_transport_tcp::TcpProvider;
use serde_json::{json, Value};

pub const PREVIEW3_PROTOCOL_VERSION: &str = "nnrp-1-preview3";
const PUBLIC_PREVIEW3_CASE_IDS: &[&str] = &[
    "l0.cache.error_code.family.golden",
    "l0.flow_update.connection.packet.golden",
    "l0.flow_update.operation.packet.golden",
    "l0.flow_update.packet.golden",
    "l0.flow_update.reserved_flags.reject",
    "l0.header.invalid_length.reject",
    "l0.header.length_mismatch.reject",
    "l0.header.roundtrip.basic",
    "l0.schema.descriptor.header.golden",
    "l0.schema.error_code.family.golden",
    "l0.session_close_ack.metadata.golden",
    "l0.session_close.metadata.golden",
    "l0.session_open_ack.metadata.golden",
    "l0.session_open_ack.reserved_fields.reject",
    "l0.session_open.metadata.golden",
    "l0.session_open.reserved_fields.reject",
    "l0.session_resume_ack.rejected.golden",
    "l0.session_resume_ack.resumed.golden",
    "l0.session_resume.metadata.golden",
    "l0.typed_payload.descriptor.golden",
    "l1.cache.dependency_invalidation.validation",
    "l1.cache.error_code.cache_miss.validation",
    "l1.cache.error_code.dependency_invalid.validation",
    "l1.cache.error_code.lease_expired.validation",
    "l1.cache.error_code.schema_mismatch.validation",
    "l1.cache.error_code.version_mismatch.validation",
    "l1.cache.lease_owner_scope.validation",
    "l1.cache.object_version.monotonicity.validation",
    "l1.connection.close.session_cascade.validation",
    "l1.connection.session_container.parallel_open.validation",
    "l1.flow_update.connection.scope.validation",
    "l1.flow_update.credit_epoch.monotonicity.validation",
    "l1.flow_update.operation.scope.validation",
    "l1.flow_update.preview3",
    "l1.flow_update.session.scope.validation",
    "l1.frame_submit.tensor.inline",
    "l1.frame_submit.tensor.inline.routing.validation",
    "l1.handshake.basic",
    "l1.handshake.capability_window.validation",
    "l1.operation.cancel_scope.validation",
    "l1.operation.lifecycle.progression.validation",
    "l1.operation.lifecycle.terminal_resolution.validation",
    "l1.operation.lifecycle.waiting_tool.validation",
    "l1.result_push.basic.terminal.validation",
    "l1.schema.descriptor.default_stream.validation",
    "l1.schema.descriptor.flags.validation",
    "l1.schema.descriptor.layout.validation",
    "l1.schema.registry.install_update.validation",
    "l1.schema.registry.invalidate_conflict.validation",
    "l1.session.close.sibling_survival.validation",
    "l1.session.close.state_machine.validation",
    "l1.session.open_ack.fixed_metadata.validation",
    "l1.session.open_close",
    "l1.session.open.fixed_metadata.validation",
    "l1.session.resume.from_operation.validation",
    "l1.session.resume.rejected.validation",
    "l1.token_profile.partial.validation",
    "l1.typed_payload.descriptor.validation",
    "l2.payload.typed.buffer_ownership.relative_region.validation",
    "l2.payload.typed.callback_polling.descriptor_consistency.validation",
    "l2.profile.token.partial.callback_polling.validation",
    "l2.result_push.basic.event_pump.single_terminal.validation",
    "l2.schema.registry.conflict_error_mapping.validation",
    "l2.schema.registry.error_mapping.critical_unknown.validation",
    "l3.transport.quic.minimum",
    "l3.transport.tcp.minimum",
];

pub fn preview3_golden_vectors() -> Value {
    json!({
        "protocol_version": PREVIEW3_PROTOCOL_VERSION,
        "vectors": [
            {
                "id": "l0.preview3.schema.token_delta.golden",
                "encoding": "hex",
                "bytes": to_hex(&token_delta_schema_descriptor().to_bytes().expect("token schema bytes")),
            },
            {
                "id": "l0.preview3.typed_payload.token_descriptor.golden",
                "encoding": "hex",
                "bytes": to_hex(&token_delta_descriptor().to_bytes().expect("typed descriptor bytes")),
            },
            {
                "id": "l0.preview3.session_open.resume_request.golden",
                "encoding": "hex",
                "bytes": to_hex(&resume_open().to_bytes().expect("session open bytes")),
            },
            {
                "id": "l0.preview3.session_open_ack.resumed.golden",
                "encoding": "hex",
                "bytes": to_hex(&resume_ack().to_bytes().expect("session open ack bytes")),
            }
        ],
    })
}

pub fn preview3_fixture_manifest() -> Value {
    let cases: Vec<Value> = preview3_case_ids()
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "protocol_version": PREVIEW3_PROTOCOL_VERSION,
                "implementation_role": "canonical-rust",
            })
        })
        .collect();

    json!({
        "protocol_version": PREVIEW3_PROTOCOL_VERSION,
        "implementation_name": "nnrp-rs",
        "cases": cases,
    })
}

pub fn preview3_case_ids() -> &'static [&'static str] {
    &[
        "l0.preview3.payload_family.boundary",
        "l0.preview3.schema.token_delta.golden",
        "l0.preview3.typed_payload.token_descriptor.golden",
        "l0.preview3.session_open.resume_request.golden",
        "l0.preview3.session_open_ack.resumed.golden",
        "l1.preview3.schema.binding.validation",
        "l1.preview3.cache.lease.validation",
        "l1.preview3.recovery.session_resume.validation",
        "l2.preview3.runtime.fixture.flow_update",
        "l2.preview3.runtime.fixture.cancellation",
        "l2.preview3.runtime.fixture.cache_miss",
        "l2.preview3.runtime.fixture.schema_mismatch",
        "l2.preview3.runtime.fixture.resume",
        "l2.preview3.runtime.ffi.client_event_flow",
        "l2.preview3.runtime.ffi.server_event_flow",
    ]
}

pub fn public_preview3_case_ids() -> &'static [&'static str] {
    PUBLIC_PREVIEW3_CASE_IDS
}

pub fn execute_preview3_case(case_id: &str) -> Option<Result<(), String>> {
    let result = match case_id {
        "l0.preview3.payload_family.boundary" => l0_payload_family_boundary(),
        "l0.preview3.schema.token_delta.golden" => l0_schema_token_delta_golden(),
        "l0.preview3.typed_payload.token_descriptor.golden" => {
            l0_typed_payload_token_descriptor_golden()
        }
        "l0.preview3.session_open.resume_request.golden" => l0_session_open_resume_request(),
        "l0.preview3.session_open_ack.resumed.golden" => l0_session_open_ack_resumed(),
        "l1.preview3.schema.binding.validation" => l1_schema_binding_validation(),
        "l1.preview3.cache.lease.validation" => l1_cache_lease_validation(),
        "l1.preview3.recovery.session_resume.validation" => l1_session_resume_validation(),
        "l2.preview3.runtime.fixture.flow_update" => l2_fixture_flow_update(),
        "l2.preview3.runtime.fixture.cancellation" => l2_fixture_cancellation(),
        "l2.preview3.runtime.fixture.cache_miss" => l2_fixture_cache_miss(),
        "l2.preview3.runtime.fixture.schema_mismatch" => l2_fixture_schema_mismatch(),
        "l2.preview3.runtime.fixture.resume" => l2_fixture_resume(),
        "l2.preview3.runtime.ffi.client_event_flow" => l2_runtime_ffi_client_event_flow(),
        "l2.preview3.runtime.ffi.server_event_flow" => l2_runtime_ffi_server_event_flow(),
        "l0.header.roundtrip.basic" => public_header_roundtrip(),
        "l0.header.invalid_length.reject" | "l0.header.length_mismatch.reject" => {
            public_header_length_reject()
        }
        "l1.handshake.basic" | "l1.handshake.capability_window.validation" => {
            public_handshake_basic()
        }
        "l0.session_open.metadata.golden"
        | "l0.session_open_ack.metadata.golden"
        | "l0.session_close.metadata.golden"
        | "l0.session_close_ack.metadata.golden"
        | "l1.session.open.fixed_metadata.validation"
        | "l1.session.open_ack.fixed_metadata.validation"
        | "l1.session.open_close"
        | "l1.session.close.state_machine.validation" => public_session_open_close(),
        "l0.session_open.reserved_fields.reject" | "l0.session_open_ack.reserved_fields.reject" => {
            public_session_reserved_reject()
        }
        "l1.connection.session_container.parallel_open.validation"
        | "l1.connection.close.session_cascade.validation"
        | "l1.session.close.sibling_survival.validation" => public_multi_session_lifecycle(),
        "l1.frame_submit.tensor.inline"
        | "l1.frame_submit.tensor.inline.routing.validation"
        | "l1.result_push.basic.terminal.validation"
        | "l2.result_push.basic.event_pump.single_terminal.validation" => {
            public_submit_result_contract()
        }
        "l0.flow_update.packet.golden"
        | "l0.flow_update.connection.packet.golden"
        | "l0.flow_update.operation.packet.golden"
        | "l1.flow_update.preview3"
        | "l1.flow_update.connection.scope.validation"
        | "l1.flow_update.session.scope.validation"
        | "l1.flow_update.operation.scope.validation"
        | "l1.flow_update.credit_epoch.monotonicity.validation" => public_flow_update_contract(),
        "l0.flow_update.reserved_flags.reject" => public_flow_update_reserved_flags(),
        "l1.operation.lifecycle.progression.validation"
        | "l1.operation.lifecycle.terminal_resolution.validation"
        | "l1.operation.lifecycle.waiting_tool.validation" => public_operation_lifecycle(),
        "l1.operation.cancel_scope.validation" => public_operation_cancel_scope(),
        "l0.typed_payload.descriptor.golden"
        | "l1.typed_payload.descriptor.validation"
        | "l2.payload.typed.buffer_ownership.relative_region.validation"
        | "l2.payload.typed.callback_polling.descriptor_consistency.validation" => {
            public_typed_payload_contract()
        }
        "l1.token_profile.partial.validation"
        | "l2.profile.token.partial.callback_polling.validation" => public_token_profile_contract(),
        "l0.schema.descriptor.header.golden"
        | "l0.schema.error_code.family.golden"
        | "l1.schema.descriptor.default_stream.validation"
        | "l1.schema.descriptor.flags.validation"
        | "l1.schema.descriptor.layout.validation"
        | "l1.schema.registry.install_update.validation"
        | "l1.schema.registry.invalidate_conflict.validation"
        | "l2.schema.registry.conflict_error_mapping.validation"
        | "l2.schema.registry.error_mapping.critical_unknown.validation" => {
            public_schema_contract()
        }
        "l0.cache.error_code.family.golden"
        | "l1.cache.dependency_invalidation.validation"
        | "l1.cache.error_code.cache_miss.validation"
        | "l1.cache.error_code.dependency_invalid.validation"
        | "l1.cache.error_code.lease_expired.validation"
        | "l1.cache.error_code.schema_mismatch.validation"
        | "l1.cache.error_code.version_mismatch.validation"
        | "l1.cache.lease_owner_scope.validation"
        | "l1.cache.object_version.monotonicity.validation" => public_cache_contract(),
        "l0.session_resume.metadata.golden"
        | "l0.session_resume_ack.resumed.golden"
        | "l0.session_resume_ack.rejected.golden"
        | "l1.session.resume.from_operation.validation"
        | "l1.session.resume.rejected.validation" => public_session_resume_contract(),
        "l3.transport.tcp.minimum" => public_tcp_transport_contract(),
        "l3.transport.quic.minimum" => public_quic_transport_contract(),
        _ => return None,
    };
    Some(result)
}

fn l0_payload_family_boundary() -> Result<(), String> {
    if !PayloadFamily::StructuredEvent.is_registry_bound_family()
        || !PayloadFamily::ToolDelta.is_registry_bound_family()
        || PayloadFamily::StructuredEvent.is_standard_profile()
        || PayloadFamily::ToolDelta.is_standard_profile()
    {
        return Err("structured_event/tool_delta escaped the payload-family boundary".to_string());
    }

    let bitmap =
        PayloadKindBitmap(PayloadKindBitmap::STRUCTURED_EVENT | PayloadKindBitmap::TOOL_DELTA);
    if !bitmap.contains(PayloadFamily::StructuredEvent)
        || !bitmap.contains(PayloadFamily::ToolDelta)
    {
        return Err("payload family bitmap no longer reports event/tool families".to_string());
    }

    Ok(())
}

fn l0_schema_token_delta_golden() -> Result<(), String> {
    let bytes = token_delta_schema_descriptor()
        .to_bytes()
        .map_err(to_string)?;
    let expected = "011000000300000002000000010100000000000000000200336b6f7470726e6e";
    if to_hex(&bytes) != expected {
        return Err("token delta schema descriptor golden vector changed".to_string());
    }
    Ok(())
}

fn l0_typed_payload_token_descriptor_golden() -> Result<(), String> {
    let bytes = token_delta_descriptor().to_bytes().map_err(to_string)?;
    let expected = "020002000110000003000000020000000000000003000000";
    if to_hex(&bytes) != expected {
        return Err("token typed-payload descriptor golden vector changed".to_string());
    }
    Ok(())
}

fn l0_session_open_resume_request() -> Result<(), String> {
    validate_session_recovery_request(&resume_open()).map_err(to_string)?;
    let bytes = resume_open().to_bytes().map_err(to_string)?;
    let expected = "2a000000020001010110000003000000f40100000400000030750000100000000000000000000000efcdab8967452301";
    if to_hex(&bytes) != expected {
        return Err("SESSION_OPEN resume request golden vector changed".to_string());
    }
    Ok(())
}

fn l0_session_open_ack_resumed() -> Result<(), String> {
    let open = resume_open();
    let ack = resume_ack();
    if validate_session_recovery_ack(&open, &ack).map_err(to_string)?
        != (SessionRecoveryOutcome::Resumed {
            resume_window_ms: 120_000,
        })
    {
        return Err("SESSION_OPEN_ACK did not preserve resumed recovery outcome".to_string());
    }
    let bytes = ack.to_bytes().map_err(to_string)?;
    let expected = "2a0000000200010301100000030000000200040030750000c0d40100100000000000000021436587a9cbed0f070000000000000001000000";
    if to_hex(&bytes) != expected {
        return Err("SESSION_OPEN_ACK resumed golden vector changed".to_string());
    }
    Ok(())
}

fn l1_schema_binding_validation() -> Result<(), String> {
    let registry = SchemaRegistry::with_standard_preview3_profiles();
    registry
        .validate_descriptor_binding(&token_delta_descriptor())
        .map_err(schema_error)?;

    let unspecified_with_schema = TypedPayloadDescriptor {
        profile_id: 0,
        ..token_delta_descriptor()
    };
    if registry.validate_descriptor_binding(&unspecified_with_schema)
        != Err(SchemaRegistryFailure::Incompatible)
    {
        return Err("profile_id=0 must stay unspecified, not implicit tensor/token".to_string());
    }

    Ok(())
}

fn l1_cache_lease_validation() -> Result<(), String> {
    let object_id = CacheObjectId {
        cache_namespace: 7,
        cache_key_hi: 0x1122_3344,
        cache_key_lo: 0x5566_7788,
        object_kind: CacheObjectKind::PromptSegment,
    };
    let lease = CacheLease {
        object_id,
        object_version: 3,
        lease_id: 99,
        owner_scope: CacheLeaseOwnerScope::Session,
        owner_id: 42,
        granted_at_ms: 10_000,
        ttl_ms: 500,
    };

    lease.validate_live_at(10_499).map_err(cache_error)?;
    if lease.validate_live_at(10_500) != Err(CacheValidationFailure::LeaseExpired) {
        return Err("cache lease expiry boundary changed".to_string());
    }
    lease.validate_version(3).map_err(cache_error)?;
    if lease.validate_version(4) != Err(CacheValidationFailure::VersionMismatch) {
        return Err("cache object version mismatch was not preserved".to_string());
    }

    let dependencies = [CacheDependency {
        object_id,
        required_version: 3,
    }];
    let states = [CacheDependencyState {
        object_id,
        current_version: 3,
        invalidated: false,
    }];
    validate_cache_dependencies(&dependencies, &states).map_err(cache_error)
}

fn l1_session_resume_validation() -> Result<(), String> {
    validate_session_recovery_request(&resume_open()).map_err(to_string)?;
    let outcome =
        validate_session_recovery_ack(&resume_open(), &resume_ack()).map_err(to_string)?;
    if outcome
        != (SessionRecoveryOutcome::Resumed {
            resume_window_ms: 120_000,
        })
    {
        return Err("resume ack outcome changed".to_string());
    }

    let rejected = SessionOpenAckMetadata {
        session_id: 0,
        session_status: SessionStatus::Rejected,
        session_error_code: SESSION_ERROR_RESUME_REJECTED,
        resume_window_ms: 0,
        resume_token_bytes: 0,
        session_flags_ack: 0,
        ..resume_ack()
    };
    if validate_session_recovery_ack(&resume_open(), &rejected).map_err(to_string)?
        != SessionRecoveryOutcome::ResumeRejected
    {
        return Err("resume_rejected outcome changed".to_string());
    }

    Ok(())
}

fn l2_runtime_ffi_client_event_flow() -> Result<(), String> {
    public_session_open_close()?;
    public_operation_lifecycle()?;
    public_operation_cancel_scope()
}

fn l2_fixture_flow_update() -> Result<(), String> {
    public_flow_update_contract()
}

fn l2_fixture_cancellation() -> Result<(), String> {
    public_operation_cancel_scope()
}

fn l2_fixture_cache_miss() -> Result<(), String> {
    let object_id = CacheObjectId {
        cache_namespace: 9,
        cache_key_hi: 1,
        cache_key_lo: 2,
        object_kind: CacheObjectKind::PromptSegment,
    };
    let dependencies = [CacheDependency {
        object_id,
        required_version: 1,
    }];
    if validate_cache_dependencies(&dependencies, &[])
        != Err(CacheValidationFailure::DependencyInvalid)
    {
        return Err("cache miss fixture no longer reports dependency invalid".to_string());
    }
    if CacheValidationFailure::Miss.error_code() == 0 {
        return Err("cache miss fixture lost its protocol error code".to_string());
    }
    Ok(())
}

fn l2_fixture_schema_mismatch() -> Result<(), String> {
    let registry = SchemaRegistry::with_standard_preview3_profiles();
    let mismatched = TypedPayloadDescriptor {
        profile_id: PROFILE_TENSOR,
        ..token_delta_descriptor()
    };
    if registry.validate_descriptor_binding(&mismatched) != Err(SchemaRegistryFailure::Incompatible)
    {
        return Err("schema mismatch fixture no longer reports incompatible binding".to_string());
    }
    Ok(())
}

fn l2_fixture_resume() -> Result<(), String> {
    let outcome =
        validate_session_recovery_ack(&resume_open(), &resume_ack()).map_err(to_string)?;
    if outcome
        != (SessionRecoveryOutcome::Resumed {
            resume_window_ms: 120_000,
        })
    {
        return Err("resume fixture no longer reports resumed outcome".to_string());
    }
    Ok(())
}

fn l2_runtime_ffi_server_event_flow() -> Result<(), String> {
    submit_result_metadata_contract()?;
    public_flow_update_contract()
}

fn public_header_roundtrip() -> Result<(), String> {
    let mut header = CommonHeader::new(MessageType::FlowUpdate, FLOW_UPDATE_METADATA_LEN as u32, 0);
    header.session_id = 42;
    header.frame_id = 7;
    header.route_id = 9;
    header.trace_id = 0x1122_3344_5566_7788;

    let bytes = header.to_bytes().map_err(to_string)?;
    let parsed = CommonHeader::parse(&bytes).map_err(to_string)?;
    if parsed != header {
        return Err("common header roundtrip changed".to_string());
    }
    Ok(())
}

fn public_header_length_reject() -> Result<(), String> {
    let mut packet = CommonHeader::new(MessageType::Ping, 4, 0)
        .to_bytes()
        .map_err(to_string)?
        .to_vec();
    packet.extend_from_slice(&[1, 2]);
    if CommonHeader::parse_packet(&packet).is_ok() {
        return Err("packet length mismatch was accepted".to_string());
    }

    let mut short_header = CommonHeader::new(MessageType::Ping, 0, 0);
    short_header.header_len = 24;
    if short_header.to_bytes().is_ok() {
        return Err("invalid header length was accepted".to_string());
    }
    Ok(())
}

fn public_handshake_basic() -> Result<(), String> {
    let hello = ClientHelloMetadata::parse(&from_hex("01010100010000000100000003000000030000002100000003000000010007000100020040000000000001007017640002000000000000006000000000000000")?)
        .map_err(to_string)?;
    let ack = ServerHelloAckMetadata {
        selected_version_major: 1,
        selected_wire_format: 0,
        auth_status: 0,
        session_id: 42,
        accepted_profile_bitmap: 0x0001,
        accepted_payload_kind_bitmap: 0x0001,
        accepted_codec_bitmap: 0x0003,
        accepted_compression_bitmap: 0x0003,
        accepted_dtype_bitmap: 0x0001,
        accepted_layout_bitmap: 0x0001,
        cache_digest_bitmap: 0x0001,
        cache_object_bitmap: 0x0007,
        max_cache_entries: 512,
        max_cache_bytes: 16 * 1024 * 1024,
        max_lane_count: 2,
        max_concurrent_frames: 2,
        target_cadence_x100: 6000,
        latency_budget_ms: 100,
        quality_tier: 2,
        degrade_policy: 2,
        max_body_bytes: 32 * 1024 * 1024,
        token_ttl_ms: 300_000,
        retry_after_ms: 0,
        control_extension_bytes: 0,
        server_flags: 1,
    };
    ack.validate_against_client_hello(&hello)
        .map_err(to_string)?;
    if ServerHelloAckMetadata::parse(&ack.to_bytes().map_err(to_string)?).map_err(to_string)? != ack
    {
        return Err("SERVER_HELLO_ACK roundtrip changed".to_string());
    }
    Ok(())
}

fn public_session_open_close() -> Result<(), String> {
    let mut connection = ConnectionLifecycle::new();
    let ack = opened_ack(42);
    connection.apply_session_open_ack(&ack).map_err(to_string)?;

    let mut close_header = CommonHeader::new(MessageType::SessionClose, 24, 0);
    close_header.session_id = 42;
    let close = SessionCloseMetadata {
        close_reason: SessionCloseReason::ClientShutdown,
        in_flight_policy: InFlightPolicy::Drain,
        drain_timeout_ms: 1000,
        last_operation_id: 7,
        session_error_code: SESSION_ERROR_NONE,
        session_close_tag: 0x1122_3344,
    };
    connection
        .begin_session_close(&close_header, &close)
        .map_err(to_string)?;

    let mut close_ack_header = CommonHeader::new(MessageType::SessionCloseAck, 16, 0);
    close_ack_header.session_id = 42;
    let close_ack = SessionCloseAckMetadata {
        close_status: SessionCloseStatus::Closed,
        last_operation_id: 7,
        session_error_code: SESSION_ERROR_NONE,
    };
    connection
        .apply_session_close_ack(&close_ack_header, &close_ack)
        .map_err(to_string)
}

fn public_session_reserved_reject() -> Result<(), String> {
    let mut open = [0u8; SESSION_OPEN_METADATA_LEN];
    open[22] = 1;
    if SessionOpenMetadata::parse(&open).is_ok() {
        return Err("SESSION_OPEN reserved field was accepted".to_string());
    }

    let mut ack = [0u8; SESSION_OPEN_ACK_METADATA_LEN];
    ack[52] = 0x20;
    if SessionOpenAckMetadata::parse(&ack).is_ok() {
        return Err("SESSION_OPEN_ACK reserved field was accepted".to_string());
    }
    Ok(())
}

fn public_multi_session_lifecycle() -> Result<(), String> {
    let mut connection = ConnectionLifecycle::new();
    connection
        .apply_session_open_ack(&opened_ack(42))
        .map_err(to_string)?;
    connection
        .apply_session_open_ack(&opened_ack(43))
        .map_err(to_string)?;
    if connection.session_count() != 2 {
        return Err("parallel sessions were not installed".to_string());
    }

    let mut close_ack_header = CommonHeader::new(MessageType::SessionCloseAck, 16, 0);
    close_ack_header.session_id = 42;
    connection
        .apply_session_close_ack(
            &close_ack_header,
            &SessionCloseAckMetadata {
                close_status: SessionCloseStatus::Closed,
                last_operation_id: 0,
                session_error_code: SESSION_ERROR_NONE,
            },
        )
        .map_err(to_string)?;
    if connection.session(43).is_none() {
        return Err("closing one session affected a sibling session".to_string());
    }
    connection.close_connection().map_err(to_string)
}

fn public_submit_result_contract() -> Result<(), String> {
    submit_result_metadata_contract()
}

fn submit_result_metadata_contract() -> Result<(), String> {
    let submit =
        FrameSubmitMetadata::parse(&[0u8; FRAME_SUBMIT_METADATA_LEN]).map_err(to_string)?;
    submit.validate_payload_shape().map_err(to_string)?;
    let result = ResultPushMetadata::parse(&[0u8; 64]).map_err(to_string)?;
    result.validate_payload_shape().map_err(to_string)
}

fn public_flow_update_contract() -> Result<(), String> {
    let mut connection = ConnectionLifecycle::new();
    connection
        .apply_session_open_ack(&opened_ack(42))
        .map_err(to_string)?;
    let mut header = CommonHeader::new(MessageType::FlowUpdate, FLOW_UPDATE_METADATA_LEN as u32, 0);
    header.session_id = 42;
    let session_flow = FlowUpdateMetadata {
        scope_kind: FlowScopeKind::Session,
        update_reason: FlowUpdateReason::Grant,
        backpressure_level: BackpressureLevel::Soft,
        connection_credit: 0,
        session_credit: 2,
        operation_credit: 0,
        operation_id: 0,
        retry_after_ms: 0,
        credit_epoch: 7,
        flow_flags: FLOW_UPDATE_FLAG_CREDIT_VALID,
    };
    connection
        .validate_flow_update(&header, &session_flow)
        .map_err(to_string)?;

    let mut op_flow = session_flow;
    op_flow.scope_kind = FlowScopeKind::Operation;
    op_flow.session_credit = 0;
    op_flow.operation_credit = 1;
    op_flow.operation_id = 99;
    op_flow.retry_after_ms = 10;
    op_flow.flow_flags |= FLOW_UPDATE_FLAG_RETRY_AFTER_VALID;
    connection
        .validate_flow_update(&header, &op_flow)
        .map_err(to_string)
}

fn public_flow_update_reserved_flags() -> Result<(), String> {
    let mut bytes = [0u8; FLOW_UPDATE_METADATA_LEN];
    bytes[28..32].copy_from_slice(&0x10u32.to_le_bytes());
    if FlowUpdateMetadata::parse(&bytes).is_ok() {
        return Err("FLOW_UPDATE reserved flags were accepted".to_string());
    }
    Ok(())
}

fn public_operation_lifecycle() -> Result<(), String> {
    let mut registry = OperationRegistry::new();
    registry
        .register(OperationDescriptor::new(42, 1))
        .map_err(to_string)?;
    registry
        .transition(1, OperationState::Running)
        .map_err(to_string)?;
    registry
        .transition(1, OperationState::WaitingTool)
        .map_err(to_string)?;
    registry
        .transition(1, OperationState::Running)
        .map_err(to_string)?;
    registry
        .transition(1, OperationState::Partial)
        .map_err(to_string)?;
    registry
        .transition(1, OperationState::Completed)
        .map_err(to_string)?;
    if registry.transition(1, OperationState::Running).is_ok() {
        return Err("terminal operation accepted a non-terminal transition".to_string());
    }
    Ok(())
}

fn public_operation_cancel_scope() -> Result<(), String> {
    let mut registry = OperationRegistry::new();
    registry
        .register(grouped_operation(1, None, 7))
        .map_err(to_string)?;
    registry
        .register(grouped_operation(2, Some(1), 7))
        .map_err(to_string)?;
    registry
        .register(grouped_operation(3, Some(2), 8))
        .map_err(to_string)?;
    let cancelled = registry
        .cancel(OperationCancelRequest {
            session_id: 42,
            operation_id: 2,
            cancel_scope: CancelScope::Subtree,
        })
        .map_err(to_string)?;
    if cancelled != [2, 3] {
        return Err(format!(
            "unexpected subtree cancellation set: {cancelled:?}"
        ));
    }
    Ok(())
}

fn public_typed_payload_contract() -> Result<(), String> {
    l0_typed_payload_token_descriptor_golden()?;
    let payload = *b"abc";
    let region = TypedPayloadRegion::from_parts(
        PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
        vec![token_delta_descriptor()],
        &payload,
    )
    .map_err(to_string)?;
    let views = region.frame_views().map_err(to_string)?;
    if views.len() != 1 || views[0].payload != payload {
        return Err("typed payload frame view changed".to_string());
    }
    Ok(())
}

fn public_token_profile_contract() -> Result<(), String> {
    let registry = SchemaRegistry::with_standard_preview3_profiles();
    registry
        .validate_descriptor_binding(&token_delta_descriptor())
        .map_err(schema_error)?;
    validate_profile_assignment(PROFILE_TOKEN).map_err(schema_error)
}

fn public_schema_contract() -> Result<(), String> {
    l0_schema_token_delta_golden()?;
    l1_schema_binding_validation()
}

fn public_cache_contract() -> Result<(), String> {
    l1_cache_lease_validation()?;
    if CacheValidationFailure::Miss.error_code() == 0
        || CacheValidationFailure::VersionMismatch.error_code() == 0
        || CacheValidationFailure::SchemaMismatch.error_code() == 0
    {
        return Err("cache error code family lost non-zero mappings".to_string());
    }
    Ok(())
}

fn public_session_resume_contract() -> Result<(), String> {
    l1_session_resume_validation()
}

fn public_tcp_transport_contract() -> Result<(), String> {
    let registry = TransportProviderRegistry::new().with_provider(TcpProvider::descriptor());
    let remote = RemoteTransportSupport::new([TransportId::Tcp]);
    let selection = registry
        .select(&remote, TransportPolicy::ForceTcp, None)
        .map_err(|error| error.to_string())?;
    if selection.selected.transport_id != TransportId::Tcp {
        return Err("TCP transport provider was not selected".to_string());
    }
    Ok(())
}

fn public_quic_transport_contract() -> Result<(), String> {
    let providers = [
        TcpProvider::descriptor(),
        QuicProvider::backend_descriptor(
            "nnrp-quic-native",
            "0.0.0",
            TransportProviderKind::NativeDynamic,
        ),
    ];
    let remote = RemoteTransportSupport::new([TransportId::Tcp, TransportId::Quic]);
    let samples = [
        ProbeSample::success(
            TransportId::Tcp,
            "nnrp.transport.tcp.native",
            10_000,
            1_500,
            512,
            512,
        ),
        ProbeSample::success(
            TransportId::Quic,
            "nnrp.transport.quic.native",
            10_000,
            800,
            512,
            512,
        ),
    ];
    let selection =
        select_transport_with_probe(&providers, &remote, TransportPolicy::Auto, None, &samples)
            .map_err(|error| error.to_string())?;
    if selection.selected.transport_id != TransportId::Quic {
        return Err("QUIC transport provider did not win the scored probe path".to_string());
    }

    let fallback_samples = [
        ProbeSample::success(
            TransportId::Tcp,
            "nnrp.transport.tcp.native",
            10_000,
            900,
            512,
            512,
        ),
        ProbeSample::failure(
            TransportId::Quic,
            "nnrp.transport.quic.native",
            10_000,
            true,
        ),
    ];
    let fallback = select_transport_with_probe(
        &providers,
        &remote,
        TransportPolicy::PreferQuic,
        None,
        &fallback_samples,
    )
    .map_err(|error| error.to_string())?;
    if fallback.selected.transport_id != TransportId::Tcp {
        return Err("scored fallback did not choose TCP when QUIC failed".to_string());
    }
    Ok(())
}

fn opened_ack(session_id: u32) -> SessionOpenAckMetadata {
    SessionOpenAckMetadata {
        session_id,
        accepted_profile_id: PROFILE_TOKEN,
        accepted_priority_class: SessionPriorityClass::Balanced,
        session_status: SessionStatus::Opened,
        schema_id: TOKEN_DELTA_SCHEMA_ID,
        schema_version: TOKEN_DELTA_SCHEMA_VERSION,
        granted_operation_credit: 2,
        max_in_flight_operations: 4,
        lease_ttl_ms: 30_000,
        resume_window_ms: 120_000,
        resume_token_bytes: 16,
        session_extension_bytes: 0,
        server_session_tag: 0x0fed_cba9_8765_4321,
        route_scope_id: 7,
        session_error_code: SESSION_ERROR_NONE,
        session_flags_ack: SESSION_ACK_FLAG_RESUME_ENABLED,
    }
}

fn grouped_operation(
    operation_id: u64,
    parent_operation_id: Option<u64>,
    operation_group_id: u64,
) -> OperationDescriptor {
    OperationDescriptor {
        session_id: 42,
        operation_id,
        parent_operation_id,
        operation_group_id: Some(operation_group_id),
    }
}

fn token_delta_descriptor() -> TypedPayloadDescriptor {
    TypedPayloadDescriptor {
        profile_id: PROFILE_TOKEN,
        descriptor_flags: 0x0002,
        schema_id: TOKEN_DELTA_SCHEMA_ID,
        schema_version: TOKEN_DELTA_SCHEMA_VERSION,
        stream_semantics: STREAM_SEMANTICS_TOKEN_DELTA,
        offset: 0,
        length: 3,
    }
}

fn resume_open() -> SessionOpenMetadata {
    SessionOpenMetadata {
        requested_session_id: 42,
        profile_id: PROFILE_TOKEN,
        priority_class: SessionPriorityClass::Balanced,
        session_flags: SESSION_FLAG_ALLOW_RESUME,
        schema_id: TOKEN_DELTA_SCHEMA_ID,
        schema_version: TOKEN_DELTA_SCHEMA_VERSION,
        default_deadline_ms: 500,
        max_in_flight_operations: 4,
        lease_ttl_hint_ms: 30_000,
        resume_token_bytes: 16,
        auth_bytes: 0,
        session_extension_bytes: 0,
        client_session_tag: 0x0123_4567_89ab_cdef,
    }
}

fn resume_ack() -> SessionOpenAckMetadata {
    SessionOpenAckMetadata {
        session_id: 42,
        accepted_profile_id: PROFILE_TOKEN,
        accepted_priority_class: SessionPriorityClass::Balanced,
        session_status: SessionStatus::Resumed,
        schema_id: TOKEN_DELTA_SCHEMA_ID,
        schema_version: TOKEN_DELTA_SCHEMA_VERSION,
        granted_operation_credit: 2,
        max_in_flight_operations: 4,
        lease_ttl_ms: 30_000,
        resume_window_ms: 120_000,
        resume_token_bytes: 16,
        session_extension_bytes: 0,
        server_session_tag: 0x0fed_cba9_8765_4321,
        route_scope_id: 7,
        session_error_code: SESSION_ERROR_NONE,
        session_flags_ack: SESSION_ACK_FLAG_RESUME_ENABLED,
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn from_hex(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("hex input must have an even length".to_string());
    }

    (0..hex.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&hex[index..index + 2], 16)
                .map_err(|error| format!("invalid hex at byte {index}: {error}"))
        })
        .collect()
}

fn to_string(error: nnrp_core::NnrpError) -> String {
    error.to_string()
}

fn schema_error(error: SchemaRegistryFailure) -> String {
    format!("schema registry failure: {error:?}")
}

fn cache_error(error: CacheValidationFailure) -> String {
    format!("cache validation failure: {error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview3_case_manifest_lists_executable_cases() {
        let manifest = preview3_fixture_manifest();
        let cases = manifest["cases"].as_array().expect("cases array");

        assert_eq!(manifest["protocol_version"], PREVIEW3_PROTOCOL_VERSION);
        assert_eq!(cases.len(), preview3_case_ids().len());
        for case_id in preview3_case_ids() {
            assert!(
                cases.iter().any(|case| case["id"] == *case_id),
                "manifest should contain {case_id}"
            );
        }
    }

    #[test]
    fn preview3_golden_vectors_are_stable_and_executable() {
        let vectors = preview3_golden_vectors();
        let vector_array = vectors["vectors"].as_array().expect("vectors array");

        assert_eq!(vector_array.len(), 4);
        assert_eq!(
            vector_array[0]["bytes"],
            "011000000300000002000000010100000000000000000200336b6f7470726e6e"
        );
        for case_id in preview3_case_ids() {
            assert_eq!(execute_preview3_case(case_id), Some(Ok(())));
        }
    }
}
