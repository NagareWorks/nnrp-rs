use nnrp_core::{
    validate_control_request_semantics, validate_partial_result_semantics,
    validate_pressure_semantics, validate_progress_semantics,
    validate_result_drop_reason_semantics, validate_scheduling_semantics,
    validate_trace_context_semantics, CapabilityMetadata, ControlRequestMetadata, MessageType,
    PartialResultMetadata, PressureMetadata, ProgressMetadata, ResultDropReasonMetadata,
    RouteHintMetadata, SchedulingMetadata, TraceContextMetadata,
    CONTROL_REQUEST_FLAG_COOPERATIVE_ALLOWED, CONTROL_REQUEST_FLAG_HARD_ABORT_ALLOWED,
    SCHEDULING_FLAG_DISCARD_STALE, SCHEDULING_FLAG_EMIT_DROP_REASON,
};
use serde_json::{json, Value};

pub const PREVIEW4_PROTOCOL_VERSION: &str = "nnrp-1-preview4";

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

    let degrade = CapabilityMetadata {
        preference_rank: 9,
        flags: 2,
        ..capability
    };
    let degrade_bytes = degrade
        .to_vec_with_body(capability_body)
        .map_err(to_string)?;
    let (parsed_degrade, parsed_degrade_body) =
        CapabilityMetadata::parse_with_body(&degrade_bytes).map_err(to_string)?;
    if parsed_degrade != degrade || parsed_degrade_body != capability_body {
        return Err("DEGRADE_PROFILE metadata roundtrip changed".to_string());
    }

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
