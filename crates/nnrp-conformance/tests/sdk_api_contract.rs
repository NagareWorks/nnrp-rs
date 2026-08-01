use nnrp_core::{
    CacheAckMetadata, CacheLease, CacheObjectId, CachePutMetadata, CapabilityMetadata,
    ConnectionLifecycle, NnrpError, SchemaDescriptorHeader, SchemaRegistry, SessionLifecycle,
    SessionOpenMetadata, TypedPayloadDescriptor, TypedPayloadFrameView,
};
use nnrp_runtime::{
    CacheLeaseResult, CachePolicyOptions, ClientProviderRoute, ClientTransportSecurity, NnrpClient,
    NnrpClientConfig, NnrpClientOptions, NnrpClientSession, NnrpEndpoint, NnrpResult,
    NnrpRuntimeEvent, NnrpRuntimeEventMetadata, NnrpRuntimeEventTail, NnrpServer,
    NnrpServerAcceptOptions, NnrpServerConfig, NnrpServerOptions, NnrpServerPolicy,
    NnrpServerPolicyDecision, NnrpServerSession, NnrpSubmitHeaderContext, NnrpSubmitRequest,
    NnrpTensorSubmitInput, NnrpTerminalEvent, NnrpTokenSubmitInput, NnrpTypedPayloadSubmitInput,
    OperationLifecycleEvent, ProviderEndpoint, RuntimeFrameHeader, ServerProviderRoute,
    ServerTransportSecurity,
};
use nnrp_transport_provider::{
    TransportCandidateDiagnostic, TransportProviderDescriptor, TransportProviderMetadata,
    TransportSelection, TransportSelectionError, TransportSelectionErrorCode,
    TransportSelectionOptions,
};

fn assert_type<T>() {}

fn assert_cache_result_fields(value: CacheLeaseResult) {
    let CacheLeaseResult {
        object_id,
        outcome,
        lease,
        object_version,
        diagnostic,
    } = value;
    drop((object_id, outcome, lease, object_version, diagnostic));
}

fn assert_cache_policy_fields(value: CachePolicyOptions) {
    let CachePolicyOptions {
        enabled,
        reuse_scope,
        expiration_hint_ms,
        invalidation_reason,
    } = value;
    let _ = (
        enabled,
        reuse_scope,
        expiration_hint_ms,
        invalidation_reason,
    );
}

fn assert_selection_options_fields(value: TransportSelectionOptions) {
    let TransportSelectionOptions {
        peer_supported_transports,
        policy,
        requested_max_frame_bytes,
        candidate_readiness,
        probe_observations,
    } = value;
    drop((
        peer_supported_transports,
        policy,
        requested_max_frame_bytes,
        candidate_readiness,
        probe_observations,
    ));
}

fn assert_selection_fields(value: TransportSelection) {
    let TransportSelection {
        selected_provider,
        candidates,
        policy,
        diagnostic,
    } = value;
    drop((selected_provider, candidates, policy, diagnostic));
}

fn assert_selection_failure_fields(value: TransportSelectionError) {
    let _: TransportSelectionErrorCode = value.code();
    let _: Option<nnrp_core::TransportPolicy> = value.policy();
    let _: Option<nnrp_core::TransportId> = value.transport_id();
    let _: &[TransportCandidateDiagnostic] = value.candidates();
    let _: Option<&str> = value.diagnostic();
    match value {
        TransportSelectionError::InvalidEvidence {
            policy,
            candidates,
            diagnostic,
        } => drop((policy, candidates, diagnostic)),
        TransportSelectionError::ForcedTransportUnavailable {
            policy,
            transport_id,
            candidates,
            diagnostic,
        } => drop((policy, transport_id, candidates, diagnostic)),
        TransportSelectionError::NoViableTransport {
            policy,
            candidates,
            diagnostic,
        } => drop((policy, candidates, diagnostic)),
    }
}

fn assert_client_session_options_fields(value: NnrpClientConfig) {
    let NnrpClientConfig {
        requested_session_id,
        profile_id,
        schema_id,
        schema_version,
        priority_class,
        default_deadline_ms,
        max_in_flight_operations,
        lease_ttl_hint_ms,
        allow_resume,
        resume_token_bytes,
        cache_hints,
    } = value;
    drop((
        requested_session_id,
        profile_id,
        schema_id,
        schema_version,
        priority_class,
        default_deadline_ms,
        max_in_flight_operations,
        lease_ttl_hint_ms,
        allow_resume,
        resume_token_bytes,
        cache_hints,
    ));
}

fn assert_server_session_options_fields(value: NnrpServerConfig) {
    let NnrpServerConfig {
        supported_profiles,
        supported_cache_objects,
        max_cache_objects,
        max_cache_object_bytes,
        schema_registry,
        resume_token_bytes,
        max_in_flight_operations,
        granted_operation_credit,
        lease_ttl_ms,
        resume_window_ms,
        application_policy,
    } = value;
    drop((
        supported_profiles,
        supported_cache_objects,
        max_cache_objects,
        max_cache_object_bytes,
        schema_registry,
        resume_token_bytes,
        max_in_flight_operations,
        granted_operation_credit,
        lease_ttl_ms,
        resume_window_ms,
        application_policy,
    ));
}

fn assert_server_policy_decision_fields(value: NnrpServerPolicyDecision) {
    let NnrpServerPolicyDecision {
        accepted,
        session_error_code,
        diagnostic,
    } = value;
    drop((accepted, session_error_code, diagnostic));
}

async fn evaluate_server_policy(
    policy: &dyn NnrpServerPolicy,
    open: &SessionOpenMetadata,
) -> NnrpServerPolicyDecision {
    policy.evaluate(open).await
}

async fn assert_dedicated_role_methods(
    client: &mut NnrpClientSession,
    server: &mut NnrpServerSession,
    put: CachePutMetadata,
    ack: CacheAckMetadata,
) -> Result<(), nnrp_runtime::RuntimeError> {
    client.send_cache_put(put, Vec::new()).await?;
    client.send_cache_ack(ack).await?;
    let _ = client.receive_cache_put().await?;
    let _ = client.receive_cache_ack().await?;
    client.send_ping().await?;
    client.send_pong().await?;
    client.receive_ping().await?;
    client.receive_pong().await?;

    server.send_cache_put(put, Vec::new()).await?;
    server.send_cache_ack(ack).await?;
    let _ = server.receive_cache_put().await?;
    let _ = server.receive_cache_ack().await?;
    server.send_ping().await?;
    server.send_pong().await?;
    server.receive_ping().await?;
    server.receive_pong().await?;
    Ok(())
}

fn assert_closed_runtime_event_metadata(value: NnrpRuntimeEventMetadata) {
    match value {
        NnrpRuntimeEventMetadata::None
        | NnrpRuntimeEventMetadata::FrameSubmit(_)
        | NnrpRuntimeEventMetadata::ResultPush(_)
        | NnrpRuntimeEventMetadata::ResultHint(_)
        | NnrpRuntimeEventMetadata::ControlRequest(_)
        | NnrpRuntimeEventMetadata::Scheduling(_)
        | NnrpRuntimeEventMetadata::Supersede(_)
        | NnrpRuntimeEventMetadata::Budget(_)
        | NnrpRuntimeEventMetadata::Progress(_)
        | NnrpRuntimeEventMetadata::PartialResult(_)
        | NnrpRuntimeEventMetadata::Pressure(_)
        | NnrpRuntimeEventMetadata::Capability(_)
        | NnrpRuntimeEventMetadata::RouteHint(_)
        | NnrpRuntimeEventMetadata::TraceContext(_)
        | NnrpRuntimeEventMetadata::ResultDropReason(_)
        | NnrpRuntimeEventMetadata::RecoverableError(_)
        | NnrpRuntimeEventMetadata::RetryAfter(_)
        | NnrpRuntimeEventMetadata::FlowUpdate(_)
        | NnrpRuntimeEventMetadata::ObjectDescriptor(_)
        | NnrpRuntimeEventMetadata::ObjectReference(_)
        | NnrpRuntimeEventMetadata::ObjectRelease(_)
        | NnrpRuntimeEventMetadata::ObjectDelta(_)
        | NnrpRuntimeEventMetadata::CacheReference(_)
        | NnrpRuntimeEventMetadata::CacheMiss(_)
        | NnrpRuntimeEventMetadata::CacheInvalidate(_)
        | NnrpRuntimeEventMetadata::SessionClose(_) => {}
    }
}

fn assert_closed_runtime_event_tail(value: NnrpRuntimeEventTail) {
    match value {
        NnrpRuntimeEventTail::None
        | NnrpRuntimeEventTail::Body(_)
        | NnrpRuntimeEventTail::Diagnostic(_)
        | NnrpRuntimeEventTail::MetadataBodyAndDelta { .. } => {}
    }
}

#[test]
fn frozen_rust_projection_resolves_every_public_target() {
    assert_type::<NnrpSubmitRequest>();
    assert_type::<NnrpSubmitHeaderContext>();
    let _: fn(NnrpTensorSubmitInput) -> Result<NnrpSubmitRequest, NnrpError> =
        NnrpSubmitRequest::tensor;
    let _: fn(NnrpTokenSubmitInput) -> Result<NnrpSubmitRequest, NnrpError> =
        NnrpSubmitRequest::token;
    let _: fn(NnrpTypedPayloadSubmitInput) -> Result<NnrpSubmitRequest, NnrpError> =
        NnrpSubmitRequest::typed_payload;

    assert_type::<RuntimeFrameHeader>();
    assert_type::<NnrpRuntimeEvent>();
    assert_type::<OperationLifecycleEvent>();
    assert_type::<NnrpTerminalEvent>();
    assert_type::<NnrpResult>();
    assert_type::<NnrpClient>();
    assert_type::<NnrpClientSession>();
    assert_type::<NnrpServer>();
    assert_type::<NnrpServerSession>();

    assert_type::<CapabilityMetadata>();
    assert_type::<ConnectionLifecycle>();
    assert_type::<SessionLifecycle>();
    assert_type::<TypedPayloadDescriptor>();
    assert_type::<TypedPayloadFrameView<'static>>();
    assert_type::<CacheObjectId>();
    assert_type::<CacheLease>();
    assert_type::<CacheLeaseResult>();
    assert_type::<CachePolicyOptions>();

    assert_type::<TransportProviderMetadata>();
    assert_type::<TransportProviderDescriptor>();
    assert_type::<TransportSelectionOptions>();
    assert_type::<TransportSelection>();
    assert_type::<TransportSelectionError>();
    assert_type::<TransportSelectionErrorCode>();
    assert_type::<TransportCandidateDiagnostic>();

    assert_type::<NnrpEndpoint>();
    assert_type::<ProviderEndpoint>();
    assert_type::<ClientTransportSecurity>();
    assert_type::<ServerTransportSecurity>();
    assert_type::<ClientProviderRoute>();
    assert_type::<ServerProviderRoute>();
    assert_type::<SchemaDescriptorHeader>();
    assert_type::<SchemaRegistry>();
    assert_type::<NnrpClientOptions>();
    assert_type::<NnrpClientConfig>();
    assert_type::<NnrpServerOptions>();
    assert_type::<NnrpServerConfig>();
    assert_type::<NnrpServerAcceptOptions>();

    let _ = assert_cache_result_fields;
    let _ = assert_cache_policy_fields;
    let _ = assert_selection_options_fields;
    let _ = assert_selection_fields;
    let _ = assert_selection_failure_fields;
    let _ = assert_server_policy_decision_fields;
    let _ = assert_client_session_options_fields;
    let _ = assert_server_session_options_fields;
    let _ = evaluate_server_policy;
    let _ = assert_dedicated_role_methods;
    let _ = assert_closed_runtime_event_metadata;
    let _ = assert_closed_runtime_event_tail;
}

#[test]
fn bootstrap_and_accept_options_use_transport_neutral_frozen_fields() {
    let endpoint: NnrpEndpoint = "nnrp://runtime.example/session".parse().unwrap();
    let client = NnrpClientOptions {
        endpoint: endpoint.clone(),
        provider_routes: Default::default(),
        transport_policy: Default::default(),
        session_defaults: Default::default(),
    };
    let server = NnrpServerOptions {
        endpoint,
        provider_routes: Default::default(),
        transport_policy: Default::default(),
        session_defaults: Default::default(),
    };
    let accept = NnrpServerAcceptOptions { timeout_ms: 1 };

    assert_eq!(client.session_defaults, NnrpClientConfig::default());
    assert_eq!(
        server.session_defaults.supported_profiles,
        NnrpServerConfig::default().supported_profiles
    );
    assert_eq!(accept.timeout_ms, 1);
}
