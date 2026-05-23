use nnrp_core::{
    token_delta_schema_descriptor, validate_cache_dependencies, validate_session_recovery_ack,
    validate_session_recovery_request, CacheDependency, CacheDependencyState, CacheLease,
    CacheLeaseOwnerScope, CacheObjectId, CacheObjectKind, CacheValidationFailure, PayloadFamily,
    PayloadKindBitmap, SchemaRegistry, SchemaRegistryFailure, SessionOpenAckMetadata,
    SessionOpenMetadata, SessionPriorityClass, SessionRecoveryOutcome, SessionStatus,
    TypedPayloadDescriptor, PROFILE_TOKEN, SESSION_ACK_FLAG_RESUME_ENABLED, SESSION_ERROR_NONE,
    SESSION_ERROR_RESUME_REJECTED, SESSION_FLAG_ALLOW_RESUME, STREAM_SEMANTICS_TOKEN_DELTA,
    TOKEN_DELTA_SCHEMA_ID, TOKEN_DELTA_SCHEMA_VERSION,
};
use serde_json::{json, Value};

pub const PREVIEW3_PROTOCOL_VERSION: &str = "nnrp-1-preview3";

pub fn export_preview3_golden_vectors() -> Value {
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

pub fn export_preview3_fixture_manifest() -> Value {
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
    ]
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
        let manifest = export_preview3_fixture_manifest();
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
        let vectors = export_preview3_golden_vectors();
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
