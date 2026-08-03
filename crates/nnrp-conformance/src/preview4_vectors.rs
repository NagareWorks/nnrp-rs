use nnrp_core::{
    validate_control_request_semantics, validate_partial_result_semantics,
    validate_pressure_semantics, validate_progress_semantics,
    validate_result_drop_reason_semantics, validate_scheduling_semantics,
    validate_trace_context_semantics, BudgetMetadata, CacheInvalidateMetadata,
    CacheInvalidateScope, CacheMissMetadata, CacheMissReason, CacheReferenceMetadata,
    CacheReuseScope, CapabilityMetadata, CommonHeader, ControlRequestMetadata, ErrorScope,
    MemoryLocationHint, MessageType, ObjectDeltaMetadata, ObjectDescriptorMetadata,
    ObjectReferenceMetadata, ObjectReleaseMetadata, ObjectReleaseReason, OwnershipHint,
    PartialResultMetadata, PayloadKind, PressureMetadata, ProgressMetadata,
    RecoverableErrorMetadata, ResultDropReasonMetadata, RouteHintMetadata, RuntimeObjectKind,
    RuntimeRole, SchedulingMetadata, SupersedeMetadata, TraceContextMetadata,
    TypedPayloadDescriptor, CACHE_REFERENCE, CONTROL_BUDGET_UPDATE, CONTROL_CANCEL_ABORT,
    CONTROL_CAPABILITY_COSTS, CONTROL_CREDIT_BACKPRESSURE, CONTROL_DEADLINE_EXPIRE,
    CONTROL_DEGRADE_PROFILE, CONTROL_PRIORITY_UPDATE, CONTROL_PROGRESS_PARTIAL,
    CONTROL_RECOVERABLE_ERROR, CONTROL_REQUEST_FLAG_COOPERATIVE_ALLOWED,
    CONTROL_REQUEST_FLAG_HARD_ABORT_ALLOWED, CONTROL_RESULT_DROP_REASON,
    CONTROL_ROUTE_EXECUTION_HINT, CONTROL_SUPERSEDE, CONTROL_TRACE_CONTEXT, OBJECT_COST,
    OBJECT_DELTA, OBJECT_LIFECYCLE, OBJECT_OWNERSHIP, RECOVERABLE_ERROR_FLAGS_KNOWN_MASK,
    SCHEDULING_FLAG_DISCARD_STALE, SCHEDULING_FLAG_EMIT_DROP_REASON, SUPERSEDE_FLAGS_KNOWN_MASK,
};
use serde_json::{json, Value};

use nnrp_core::object::{
    object_delta_packet_bytes, object_reference_packet_bytes, parse_object_delta_packet,
    parse_object_reference_packet,
};

pub const PREVIEW4_PROTOCOL_VERSION: &str = "nnrp-1-preview4";

pub fn preview4_public_case_ids() -> &'static [&'static str] {
    &[
        "l0.header.fixed_shape.golden",
        "l0.typed_payload.descriptor.current.golden",
        "l1.control.cancel-abort",
        "l1.control.priority-deadline",
        "l1.control.progress-backpressure",
        "l1.control.capability-costs",
        "l1.object.lifecycle",
        "l1.object.delta",
        "l1.control.route-execution-hint",
        "l1.control.cache-reference",
        "l1.control.degrade-budget",
        "l1.control.supersede",
        "l1.control.recoverable-error",
    ]
}

pub fn preview4_capability_tokens() -> &'static [&'static str] {
    &[
        "payload.typed",
        CONTROL_CANCEL_ABORT,
        CONTROL_SUPERSEDE,
        CONTROL_PRIORITY_UPDATE,
        CONTROL_DEADLINE_EXPIRE,
        CONTROL_PROGRESS_PARTIAL,
        CONTROL_CREDIT_BACKPRESSURE,
        CONTROL_CAPABILITY_COSTS,
        CONTROL_ROUTE_EXECUTION_HINT,
        CONTROL_TRACE_CONTEXT,
        CONTROL_RESULT_DROP_REASON,
        CONTROL_DEGRADE_PROFILE,
        CONTROL_BUDGET_UPDATE,
        CONTROL_RECOVERABLE_ERROR,
        OBJECT_LIFECYCLE,
        OBJECT_DELTA,
        OBJECT_COST,
        OBJECT_OWNERSHIP,
        CACHE_REFERENCE,
    ]
}

pub fn execute_preview4_public_case(case_id: &str) -> Option<Result<(), String>> {
    let result = match case_id {
        "l0.header.fixed_shape.golden" => current_header_golden_validation(),
        "l0.typed_payload.descriptor.current.golden" => {
            current_typed_payload_descriptor_golden_validation()
        }
        "l1.control.cancel-abort" => control_cancel_abort_public_validation(),
        "l1.control.priority-deadline" => control_priority_deadline_public_validation(),
        "l1.control.progress-backpressure" => control_progress_pressure_validation(),
        "l1.control.capability-costs" => control_capability_costs_validation(),
        "l1.object.lifecycle" => runtime_object_lifecycle_validation(),
        "l1.object.delta" => runtime_object_delta_validation(),
        "l1.control.route-execution-hint" => control_route_validation(),
        "l1.control.cache-reference" => cache_reference_validation(),
        "l1.control.degrade-budget" => control_degrade_budget_validation(),
        "l1.control.supersede" => control_supersede_public_validation(),
        "l1.control.recoverable-error" => control_recoverable_error_validation(),
        _ => return None,
    };
    Some(result)
}

fn current_header_golden_validation() -> Result<(), String> {
    const EXPECTED: [u8; 40] = [
        0x4e, 0x4e, 0x52, 0x50, 0x01, 0x00, 0x10, 0x28, 0x21, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00,
        0x00, 0x00, 0x10, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x02, 0x00,
        0x00, 0x00, 0x15, 0xcd, 0x5b, 0x07, 0x00, 0x00, 0x00, 0x00,
    ];
    let header = CommonHeader::parse(&EXPECTED).map_err(to_string)?;
    if header.to_bytes().map_err(to_string)? != EXPECTED {
        return Err("current common header golden bytes changed".to_string());
    }
    Ok(())
}

fn current_typed_payload_descriptor_golden_validation() -> Result<(), String> {
    const EXPECTED: [u8; 24] = [
        0x02, 0x00, 0x02, 0x02, 0x01, 0x10, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
        0x00, 0x08, 0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00,
    ];
    let descriptor = TypedPayloadDescriptor {
        profile_id: 2,
        payload_kind: PayloadKind::TokenChunk,
        descriptor_flags: 0x02,
        schema_id: 0x1001,
        schema_version: 3,
        stream_semantics: 2,
        offset: 8,
        length: 24,
    };
    let encoded = descriptor.to_bytes().map_err(to_string)?;
    if encoded != EXPECTED {
        return Err("current typed payload descriptor golden bytes changed".to_string());
    }
    let parsed = TypedPayloadDescriptor::parse(&encoded).map_err(to_string)?;
    if parsed != descriptor {
        return Err("current typed payload descriptor roundtrip changed".to_string());
    }
    Ok(())
}

pub fn preview4_fixture_manifest() -> Value {
    let cases: Vec<Value> = preview4_case_ids()
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "protocol_version": PREVIEW4_PROTOCOL_VERSION,
                "implementation_role": "canonical-rust",
                "suite_type": "control-frame-fixture",
            })
        })
        .collect();

    json!({
        "protocol_version": PREVIEW4_PROTOCOL_VERSION,
        "implementation_name": "nnrp-rs",
        "cases": cases,
    })
}

pub fn preview4_case_ids() -> &'static [&'static str] {
    &[
        "l1.preview4.control.cancel_abort.validation",
        "l1.preview4.control.scheduling.validation",
        "l1.preview4.control.progress_pressure.validation",
        "l1.preview4.control.capability_route.validation",
        "l1.preview4.control.diagnostics.validation",
    ]
}

pub fn execute_preview4_case(case_id: &str) -> Option<Result<(), String>> {
    let result = match case_id {
        "l1.preview4.control.cancel_abort.validation" => control_cancel_abort_validation(),
        "l1.preview4.control.scheduling.validation" => control_scheduling_validation(),
        "l1.preview4.control.progress_pressure.validation" => {
            control_progress_pressure_validation()
        }
        "l1.preview4.control.capability_route.validation" => control_capability_route_validation(),
        "l1.preview4.control.diagnostics.validation" => control_diagnostics_validation(),
        _ => return None,
    };
    Some(result)
}

fn control_cancel_abort_validation() -> Result<(), String> {
    let cancel = ControlRequestMetadata {
        operation_id: 42,
        control_sequence: 1,
        reason_code: 7,
        source_role: 1,
        flags: CONTROL_REQUEST_FLAG_COOPERATIVE_ALLOWED,
        diagnostic_bytes: 6,
    };
    validate_control_request_semantics(MessageType::Cancel, &cancel).map_err(to_string)?;
    let cancel_diagnostics = b"cancel";
    let cancel_bytes = cancel
        .to_vec_with_diagnostics(cancel_diagnostics)
        .map_err(to_string)?;
    let (parsed_cancel, parsed_cancel_diagnostics) =
        ControlRequestMetadata::parse_with_diagnostics(&cancel_bytes).map_err(to_string)?;
    if parsed_cancel != cancel || parsed_cancel_diagnostics != cancel_diagnostics {
        return Err("CANCEL control metadata roundtrip changed".to_string());
    }

    let abort = ControlRequestMetadata {
        operation_id: 42,
        control_sequence: 2,
        reason_code: 9,
        source_role: 2,
        flags: CONTROL_REQUEST_FLAG_HARD_ABORT_ALLOWED,
        diagnostic_bytes: 5,
    };
    validate_control_request_semantics(MessageType::Abort, &abort).map_err(to_string)?;
    let abort_diagnostics = b"abort";
    let abort_bytes = abort
        .to_vec_with_diagnostics(abort_diagnostics)
        .map_err(to_string)?;
    let (parsed_abort, parsed_abort_diagnostics) =
        ControlRequestMetadata::parse_with_diagnostics(&abort_bytes).map_err(to_string)?;
    if parsed_abort != abort || parsed_abort_diagnostics != abort_diagnostics {
        return Err("ABORT control metadata roundtrip changed".to_string());
    }

    Ok(())
}

fn control_cancel_abort_public_validation() -> Result<(), String> {
    control_cancel_abort_validation()?;
    control_diagnostics_validation()
}

fn control_scheduling_validation() -> Result<(), String> {
    let priority = SchedulingMetadata {
        operation_id: 42,
        control_sequence: 3,
        priority_class: 5,
        priority_delta: 2,
        deadline_unix_ms: 0,
        flags: SCHEDULING_FLAG_DISCARD_STALE,
    };
    validate_scheduling_semantics(MessageType::PriorityUpdate, &priority).map_err(to_string)?;
    if SchedulingMetadata::parse(&priority.to_bytes().map_err(to_string)?).map_err(to_string)?
        != priority
    {
        return Err("PRIORITY_UPDATE metadata roundtrip changed".to_string());
    }

    let deadline = SchedulingMetadata {
        operation_id: 42,
        control_sequence: 4,
        priority_class: 5,
        priority_delta: 0,
        deadline_unix_ms: 1_894_348_800_000,
        flags: SCHEDULING_FLAG_DISCARD_STALE | SCHEDULING_FLAG_EMIT_DROP_REASON,
    };
    for message_type in [MessageType::Deadline, MessageType::ExpireAt] {
        validate_scheduling_semantics(message_type, &deadline).map_err(to_string)?;
    }
    if SchedulingMetadata::parse(&deadline.to_bytes().map_err(to_string)?).map_err(to_string)?
        != deadline
    {
        return Err("DEADLINE/EXPIRE_AT metadata roundtrip changed".to_string());
    }

    Ok(())
}

fn control_priority_deadline_public_validation() -> Result<(), String> {
    control_scheduling_validation()?;
    control_diagnostics_validation()
}

fn control_progress_pressure_validation() -> Result<(), String> {
    let progress_body = b"decode-stage";
    let progress = ProgressMetadata {
        operation_id: 42,
        progress_sequence: 5,
        stage_code: 3,
        percent_x100: 4250,
        object_id: 99,
        body_bytes: progress_body.len() as u32,
    };
    validate_progress_semantics(&progress).map_err(to_string)?;
    let progress_bytes = progress
        .to_vec_with_body(progress_body)
        .map_err(to_string)?;
    let (parsed_progress, parsed_progress_body) =
        ProgressMetadata::parse_with_body(&progress_bytes).map_err(to_string)?;
    if parsed_progress != progress || parsed_progress_body != progress_body {
        return Err("PROGRESS metadata roundtrip changed".to_string());
    }

    let partial_body = b"partial";
    let partial = PartialResultMetadata {
        operation_id: 42,
        result_sequence: 6,
        object_id: 0,
        delta_sequence: 1,
        body_bytes: partial_body.len() as u32,
        flags: 0,
    };
    validate_partial_result_semantics(&partial).map_err(to_string)?;
    let partial_bytes = partial.to_vec_with_body(partial_body).map_err(to_string)?;
    let (parsed_partial, parsed_partial_body) =
        PartialResultMetadata::parse_with_body(&partial_bytes).map_err(to_string)?;
    if parsed_partial != partial || parsed_partial_body != partial_body {
        return Err("PARTIAL_RESULT metadata roundtrip changed".to_string());
    }

    let backpressure = PressureMetadata {
        scope_id: 42,
        credit_window: 8,
        pressure_level: 2,
        pressure_reason: 4,
        retry_after_ms: 25,
        flags: 1,
    };
    validate_pressure_semantics(MessageType::Backpressure, &backpressure).map_err(to_string)?;
    if PressureMetadata::parse(&backpressure.to_bytes().map_err(to_string)?).map_err(to_string)?
        != backpressure
    {
        return Err("BACKPRESSURE metadata roundtrip changed".to_string());
    }

    let credit = PressureMetadata {
        scope_id: 42,
        credit_window: 128,
        pressure_level: 0,
        pressure_reason: 0,
        retry_after_ms: 0,
        flags: 0,
    };
    validate_pressure_semantics(MessageType::CreditUpdate, &credit).map_err(to_string)?;
    if PressureMetadata::parse(&credit.to_bytes().map_err(to_string)?).map_err(to_string)? != credit
    {
        return Err("CREDIT_UPDATE metadata roundtrip changed".to_string());
    }

    Ok(())
}

fn control_capability_route_validation() -> Result<(), String> {
    control_capability_costs_validation()?;
    control_degrade_budget_validation()?;
    control_route_validation()
}

fn control_capability_costs_validation() -> Result<(), String> {
    let capability_body = br#"{"supports":["partial-result","cache-reference"]}"#;
    let capability = CapabilityMetadata {
        profile_id: 0x1001,
        capability_count: 2,
        cost_model_id: 3,
        preference_rank: 1,
        limit_bytes: 64 * 1024 * 1024,
        limit_units: 4096,
        body_bytes: capability_body.len() as u32,
        flags: 1,
    };
    let capability_bytes = capability
        .to_vec_with_body(capability_body)
        .map_err(to_string)?;
    let (parsed_capability, parsed_capability_body) =
        CapabilityMetadata::parse_with_body(&capability_bytes).map_err(to_string)?;
    if parsed_capability != capability || parsed_capability_body != capability_body {
        return Err("CAPABILITY_NEGOTIATION metadata roundtrip changed".to_string());
    }

    Ok(())
}

fn control_route_validation() -> Result<(), String> {
    let route_body = br#"{"executor":"gpu-local","affinity":"same-node"}"#;
    let route = RouteHintMetadata {
        operation_id: 42,
        route_id: 7,
        executor_class: 3,
        affinity_class: 2,
        deadline_unix_ms: 1_894_348_800_000,
        body_bytes: route_body.len() as u32,
        flags: 1,
    };
    let route_bytes = route.to_vec_with_body(route_body).map_err(to_string)?;
    let (parsed_route, parsed_route_body) =
        RouteHintMetadata::parse_with_body(&route_bytes).map_err(to_string)?;
    if parsed_route != route || parsed_route_body != route_body {
        return Err("ROUTE_HINT metadata roundtrip changed".to_string());
    }

    let execution_hint = RouteHintMetadata { flags: 2, ..route };
    let execution_bytes = execution_hint
        .to_vec_with_body(route_body)
        .map_err(to_string)?;
    let (parsed_execution_hint, parsed_execution_body) =
        RouteHintMetadata::parse_with_body(&execution_bytes).map_err(to_string)?;
    if parsed_execution_hint != execution_hint || parsed_execution_body != route_body {
        return Err("EXECUTION_HINT metadata roundtrip changed".to_string());
    }

    Ok(())
}

fn control_degrade_budget_validation() -> Result<(), String> {
    let body = br#"{"profile":"tensor-low-cost"}"#;
    let degrade = CapabilityMetadata {
        profile_id: 0x1001,
        capability_count: 1,
        cost_model_id: 3,
        preference_rank: 9,
        limit_bytes: 16 * 1024 * 1024,
        limit_units: 1024,
        body_bytes: body.len() as u32,
        flags: 2,
    };
    let bytes = degrade.to_vec_with_body(body).map_err(to_string)?;
    let (parsed, parsed_body) = CapabilityMetadata::parse_with_body(&bytes).map_err(to_string)?;
    if parsed != degrade || parsed_body != body {
        return Err("DEGRADE_PROFILE metadata roundtrip changed".to_string());
    }

    let budget = BudgetMetadata {
        operation_id: 42,
        compute_budget_units: 8_000,
        memory_budget_bytes: 64 * 1024 * 1024,
        bandwidth_budget_bytes: 8 * 1024 * 1024,
        token_budget: 2048,
        flags: 0,
    };
    if BudgetMetadata::parse(&budget.to_bytes().map_err(to_string)?).map_err(to_string)? != budget {
        return Err("BUDGET_UPDATE metadata roundtrip changed".to_string());
    }
    Ok(())
}

fn control_supersede_validation() -> Result<(), String> {
    let diagnostics = b"newer-operation";
    let metadata = SupersedeMetadata {
        old_operation_id: 41,
        new_operation_id: 42,
        control_sequence: 7,
        drop_reason_code: 2,
        flags: SUPERSEDE_FLAGS_KNOWN_MASK,
        diagnostic_bytes: diagnostics.len() as u32,
    };
    let bytes = metadata
        .to_vec_with_diagnostics(diagnostics)
        .map_err(to_string)?;
    let (parsed, parsed_diagnostics) =
        SupersedeMetadata::parse_with_diagnostics(&bytes).map_err(to_string)?;
    if parsed != metadata || parsed_diagnostics != diagnostics {
        return Err("SUPERSEDE metadata roundtrip changed".to_string());
    }
    Ok(())
}

fn control_supersede_public_validation() -> Result<(), String> {
    control_supersede_validation()?;
    control_diagnostics_validation()
}

fn control_recoverable_error_validation() -> Result<(), String> {
    let diagnostics = b"retry-on-next-credit";
    let metadata = RecoverableErrorMetadata {
        error_code: 17,
        error_scope: ErrorScope::Frame,
        recovery_action: 3,
        source_role: RuntimeRole::Runtime as u8,
        flags: RECOVERABLE_ERROR_FLAGS_KNOWN_MASK,
        retry_after_ms: 25,
        related_session_id: 7,
        related_frame_id: 11,
        related_view_id: 2,
        diagnostic_bytes: diagnostics.len() as u32,
    };
    let bytes = metadata
        .to_vec_with_diagnostics(diagnostics)
        .map_err(to_string)?;
    let (parsed, parsed_diagnostics) =
        RecoverableErrorMetadata::parse_with_diagnostics(&bytes).map_err(to_string)?;
    if parsed != metadata || parsed_diagnostics != diagnostics {
        return Err("RECOVERABLE_ERROR metadata roundtrip changed".to_string());
    }
    Ok(())
}

fn runtime_object_lifecycle_validation() -> Result<(), String> {
    let descriptor_extension = br#"{"shape":[1,1024],"dtype":"f16"}"#;
    let descriptor = ObjectDescriptorMetadata {
        object_id: 91,
        object_kind: RuntimeObjectKind::Tensor,
        producer_role: RuntimeRole::Runtime,
        consumer_role: RuntimeRole::Client,
        session_id: 7,
        byte_size: 2048,
        compute_cost_units: 32,
        memory_location_hint: MemoryLocationHint::DeviceMemory,
        ownership_hint: OwnershipHint::Borrowed,
        lifetime_hint_ms: 250,
        metadata_bytes: descriptor_extension.len() as u32,
    };
    let descriptor_bytes = descriptor
        .to_vec_with_extension(descriptor_extension)
        .map_err(to_string)?;
    let (parsed_descriptor, parsed_extension) =
        ObjectDescriptorMetadata::parse_with_extension(&descriptor_bytes).map_err(to_string)?;
    if parsed_descriptor != descriptor || parsed_extension != descriptor_extension {
        return Err("OBJECT_DECLARE metadata roundtrip changed".to_string());
    }

    let reference_extension = br#"{"range":"full"}"#;
    let reference = ObjectReferenceMetadata {
        object_id: 91,
        operation_id: 42,
        object_version: 1,
        offset: 0,
        length: 2048,
        flags: 0,
        metadata_bytes: reference_extension.len() as u32,
    };
    let reference_bytes =
        object_reference_packet_bytes(&reference, reference_extension).map_err(to_string)?;
    let parsed_reference = parse_object_reference_packet(&reference_bytes).map_err(to_string)?;
    if parsed_reference.metadata != reference
        || parsed_reference.extension_metadata != reference_extension
    {
        return Err("OBJECT_REF metadata roundtrip changed".to_string());
    }

    let release_diagnostics = b"completed";
    let release = ObjectReleaseMetadata {
        object_id: 91,
        operation_id: 42,
        release_reason: ObjectReleaseReason::Completed,
        source_role: RuntimeRole::Runtime,
        flags: 0,
        diagnostic_bytes: release_diagnostics.len() as u32,
    };
    let release_bytes = release
        .to_vec_with_diagnostics(release_diagnostics)
        .map_err(to_string)?;
    let (parsed_release, parsed_release_diagnostics) =
        ObjectReleaseMetadata::parse_with_diagnostics(&release_bytes).map_err(to_string)?;
    if parsed_release != release || parsed_release_diagnostics != release_diagnostics {
        return Err("OBJECT_RELEASE metadata roundtrip changed".to_string());
    }
    Ok(())
}

fn runtime_object_delta_validation() -> Result<(), String> {
    let extension = br#"{"codec":"xor"}"#;
    let payload = [1, 3, 5, 7];
    let delta = ObjectDeltaMetadata {
        object_id: 91,
        delta_sequence: 2,
        region_offset: 512,
        region_bytes: 256,
        delta_bytes: payload.len() as u32,
        flags: 0,
        metadata_bytes: extension.len() as u32,
    };
    let bytes = object_delta_packet_bytes(&delta, extension, &payload).map_err(to_string)?;
    let parsed = parse_object_delta_packet(&bytes).map_err(to_string)?;
    if parsed.metadata != delta
        || parsed.extension_metadata != extension
        || parsed.delta_payload != payload
    {
        return Err("OBJECT_DELTA metadata roundtrip changed".to_string());
    }
    Ok(())
}

fn cache_reference_validation() -> Result<(), String> {
    let extension = br#"{"tenant":"conformance"}"#;
    let reference = CacheReferenceMetadata {
        cache_namespace: 42,
        cache_key_hi: 1,
        cache_key_lo: 2,
        profile_id: 0x1001,
        reuse_scope: CacheReuseScope::Session,
        lease_id: 9,
        producer_trace_id: 11,
        expiration_hint_ms: 500,
        metadata_bytes: extension.len() as u32,
        flags: 0,
    };
    let bytes = reference
        .to_vec_with_extension(extension)
        .map_err(to_string)?;
    let (parsed_reference, parsed_extension) =
        CacheReferenceMetadata::parse_with_extension(&bytes).map_err(to_string)?;
    if parsed_reference != reference || parsed_extension != extension {
        return Err("CACHE_REFERENCE metadata roundtrip changed".to_string());
    }

    let diagnostics = b"not-found";
    let miss = CacheMissMetadata {
        cache_namespace: 42,
        cache_key_hi: 1,
        cache_key_lo: 2,
        miss_reason: CacheMissReason::NotFound,
        profile_id: 0x1001,
        diagnostic_bytes: diagnostics.len() as u32,
    };
    let bytes = miss
        .to_vec_with_diagnostics(diagnostics)
        .map_err(to_string)?;
    let (parsed_miss, parsed_diagnostics) =
        CacheMissMetadata::parse_with_diagnostics(&bytes).map_err(to_string)?;
    if parsed_miss != miss || parsed_diagnostics != diagnostics {
        return Err("CACHE_MISS metadata roundtrip changed".to_string());
    }

    let invalidate = CacheInvalidateMetadata {
        invalidate_scope: CacheInvalidateScope::ObjectKey,
        cache_namespace: 42,
        cache_key_hi: 1,
        cache_key_lo: 2,
        reason_code: 3,
    };
    if CacheInvalidateMetadata::parse(&invalidate.to_bytes().map_err(to_string)?)
        .map_err(to_string)?
        != invalidate
    {
        return Err("CACHE_INVALIDATE metadata roundtrip changed".to_string());
    }
    Ok(())
}

fn control_diagnostics_validation() -> Result<(), String> {
    let trace_body = br#"{"component":"scheduler"}"#;
    let trace = TraceContextMetadata {
        trace_id: 0xfeed,
        span_id: 0xbeef,
        parent_span_id: 0x100,
        stage_code: 3,
        flags: 1,
        body_bytes: trace_body.len() as u32,
    };
    validate_trace_context_semantics(&trace).map_err(to_string)?;
    let trace_bytes = trace.to_vec_with_body(trace_body).map_err(to_string)?;
    let (parsed_trace, parsed_trace_body) =
        TraceContextMetadata::parse_with_body_and_error_context(&trace_bytes)
            .map_err(|error| error.error.to_string())?;
    if parsed_trace != trace || parsed_trace_body != trace_body {
        return Err("TRACE_CONTEXT metadata roundtrip changed".to_string());
    }

    let drop_diagnostics = b"deadline";
    let drop_reason = ResultDropReasonMetadata {
        operation_id: 42,
        result_sequence: 7,
        drop_reason_code: 1,
        source_role: 2,
        flags: 1,
        diagnostic_bytes: drop_diagnostics.len() as u32,
    };
    validate_result_drop_reason_semantics(&drop_reason).map_err(to_string)?;
    let drop_bytes = drop_reason
        .to_vec_with_diagnostics(drop_diagnostics)
        .map_err(to_string)?;
    let (parsed_drop_reason, parsed_drop_diagnostics) =
        ResultDropReasonMetadata::parse_with_diagnostics(&drop_bytes).map_err(to_string)?;
    if parsed_drop_reason != drop_reason || parsed_drop_diagnostics != drop_diagnostics {
        return Err("RESULT_DROP_REASON metadata roundtrip changed".to_string());
    }

    Ok(())
}

fn to_string(error: nnrp_core::NnrpError) -> String {
    error.to_string()
}
