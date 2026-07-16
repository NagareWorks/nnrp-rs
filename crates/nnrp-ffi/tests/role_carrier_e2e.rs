use std::ptr;
use std::slice;
use std::thread;

use nnrp_core::{
    BackpressureLevel, BudgetMetadata, CacheInvalidateMetadata, CacheInvalidateScope,
    CacheMissMetadata, CacheMissReason, CacheReferenceMetadata, CacheReuseScope,
    CapabilityMetadata, ControlRequestMetadata, ErrorScope, FlowScopeKind, FlowUpdateMetadata,
    FlowUpdateReason, FrameSubmitMetadata, InputProfile, MemoryLocationHint, MessageType,
    ObjectDeltaMetadata, ObjectDescriptorMetadata, ObjectReferenceMetadata, ObjectReleaseMetadata,
    ObjectReleaseReason, OwnershipHint, PartialResultMetadata, PayloadKindBitmap, PressureMetadata,
    ProgressMetadata, RecoverableErrorMetadata, ResultClass, ResultDropReasonMetadata,
    ResultHintBudgetPolicy, ResultHintCongestionState, ResultHintMetadata, ResultHintReason,
    ResultPushMetadata, RetryAfterMetadata, RouteHintMetadata, RuntimeObjectKind, RuntimeRole,
    SchedulingMetadata, SubmitMode, SupersedeMetadata, TileIndexMode, TraceContextMetadata,
    TransportId, FLOW_UPDATE_FLAG_CREDIT_VALID, PROFILE_TOKEN, TOKEN_DELTA_SCHEMA_ID,
    TOKEN_DELTA_SCHEMA_VERSION,
};
use nnrp_ffi::{
    nnrp_buffer_release, nnrp_client_await_event, nnrp_client_await_events, nnrp_client_cancel,
    nnrp_client_close, nnrp_client_connect, nnrp_client_open_session, nnrp_client_submit,
    nnrp_connection_close, nnrp_runtime_frame_send, nnrp_server_accept, nnrp_server_await_events,
    nnrp_server_bind, nnrp_server_close, nnrp_server_send_partial_result, nnrp_server_send_result,
    NnrpBufferView, NnrpClientCancelRequest, NnrpClientConnectRequest, NnrpEvent, NnrpEventKind,
    NnrpFfiStatus, NnrpHandle, NnrpHandleKind, NnrpPollResult, NnrpRoleEventPollRequest,
    NnrpRuntimeFrameSendRequest, NnrpServerAcceptRequest, NnrpServerBindRequest,
    NnrpServerSendPartialResultRequest, NnrpServerSendResultRequest, NnrpSessionOpenRequest,
    NnrpSubmitRequest, NnrpTransportFrameBatch, NnrpTransportOpenRequest,
    NnrpTransportReadBatchRequest,
};

unsafe extern "C" {
    fn nnrp_transport_connect(
        request: NnrpTransportOpenRequest,
        out_connection: *mut NnrpHandle,
    ) -> NnrpFfiStatus;
    fn nnrp_transport_listen(
        request: NnrpTransportOpenRequest,
        out_listener: *mut NnrpHandle,
    ) -> NnrpFfiStatus;
    fn nnrp_transport_listener_endpoint(
        listener: NnrpHandle,
        out_buffer: *mut NnrpHandle,
        out_endpoint: *mut NnrpBufferView,
    ) -> NnrpFfiStatus;
    fn nnrp_transport_read_batch(
        request: NnrpTransportReadBatchRequest,
        out_batch: *mut NnrpTransportFrameBatch,
    ) -> NnrpFfiStatus;
}

fn view(bytes: &[u8]) -> NnrpBufferView {
    NnrpBufferView {
        ptr: bytes.as_ptr(),
        len: bytes.len(),
    }
}

fn open_request(transport_id: TransportId, endpoint: &str) -> NnrpTransportOpenRequest {
    NnrpTransportOpenRequest {
        transport_id: transport_id as u32,
        flags: 0,
        endpoint: view(endpoint.as_bytes()),
        config: NnrpHandle::invalid(),
        max_packet_bytes: 0,
        timeout_ms: 5_000,
        reserved0: 0,
    }
}

fn token_submit(operation_id: u64) -> FrameSubmitMetadata {
    FrameSubmitMetadata {
        src_width: 0,
        src_height: 0,
        tile_width: 0,
        tile_height: 0,
        tile_count: 0,
        section_count: 0,
        frame_class: 0,
        input_profile: InputProfile::Unspecified,
        tile_index_mode: TileIndexMode::DenseRange,
        latency_budget_ms: 25,
        target_fps_x100: 0,
        retry_of_frame: 0,
        tile_base_id: 0,
        camera_bytes: 0,
        tile_index_bytes: 0,
        operation_id,
        submit_mode: SubmitMode::Inline,
        budget_policy: 0,
        loss_tolerance_policy: 0,
        object_ref_mask: 0,
        dependency_frame_id: 0,
        payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
        payload_frame_count: 1,
    }
}

fn token_result() -> ResultPushMetadata {
    ResultPushMetadata {
        status_code: 200,
        result_flags: 0,
        section_count: 0,
        tile_count: 0,
        active_profile_id: PROFILE_TOKEN,
        inference_ms: 3,
        queue_ms: 1,
        server_total_ms: 4,
        tile_base_id: 0,
        tile_index_bytes: 0,
        result_class: ResultClass::Complete,
        applied_budget_policy: 0,
        reused_frame_id: 0,
        covered_tile_count: 0,
        dropped_tile_count: 0,
        payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
        payload_frame_count: 1,
    }
}

fn poll_request(scope: NnrpHandle) -> NnrpRoleEventPollRequest {
    NnrpRoleEventPollRequest {
        scope,
        max_events: 1,
        timeout_ms: 5_000,
        flags: 0,
        reserved0: 0,
    }
}

unsafe fn poll_client_event(session: NnrpHandle) -> NnrpEvent {
    let mut event = NnrpEvent::none();
    let mut count = 0usize;
    assert_eq!(
        nnrp_client_await_events(poll_request(session), &mut event, 1, &mut count),
        NnrpFfiStatus::ok()
    );
    assert_eq!(count, 1);
    event
}

unsafe fn poll_server_event(session: NnrpHandle) -> NnrpEvent {
    let mut event = NnrpEvent::none();
    let mut count = 0usize;
    assert_eq!(
        nnrp_server_await_events(poll_request(session), &mut event, 1, &mut count),
        NnrpFfiStatus::ok()
    );
    assert_eq!(count, 1);
    event
}

unsafe fn send_runtime_frame(
    handle: NnrpHandle,
    message_type: MessageType,
    frame_id: u32,
    payload: &[u8],
) {
    assert_eq!(
        nnrp_runtime_frame_send(NnrpRuntimeFrameSendRequest {
            handle,
            message_type: message_type as u32,
            frame_id,
            payload: view(payload),
        }),
        NnrpFfiStatus::ok(),
        "failed to send {message_type:?}"
    );
}

unsafe fn assert_runtime_event(
    event: NnrpEvent,
    message_type: MessageType,
    expected_operation: Option<NnrpHandle>,
    payload: &[u8],
) {
    assert_eq!(event.message_type, message_type as u32);
    assert_eq!(
        event.kind,
        if message_type == MessageType::PartialResult {
            NnrpEventKind::PartialResult as u32
        } else if message_type == MessageType::ResultDropReason {
            NnrpEventKind::ResultDropped as u32
        } else if message_type == MessageType::FlowUpdate {
            NnrpEventKind::FlowUpdated as u32
        } else if message_type == MessageType::ResultHint {
            NnrpEventKind::ResultHint as u32
        } else {
            NnrpEventKind::RuntimeFrame as u32
        }
    );
    if let Some(operation) = expected_operation {
        assert_eq!(event.operation, operation);
    }
    assert_eq!(
        slice::from_raw_parts(event.payload.ptr, event.payload.len),
        payload
    );
    assert_eq!(
        nnrp_buffer_release(event.payload_owner),
        NnrpFfiStatus::ok()
    );
}

fn control_payload(operation_id: u64, source_role: RuntimeRole) -> Vec<u8> {
    ControlRequestMetadata {
        operation_id,
        control_sequence: 1,
        reason_code: 1,
        source_role: source_role as u8,
        flags: 0,
        diagnostic_bytes: 2,
    }
    .to_vec_with_diagnostics(b"ok")
    .expect("control payload")
}

fn scheduling_payload(operation_id: u64) -> Vec<u8> {
    SchedulingMetadata {
        operation_id,
        control_sequence: 2,
        priority_class: 2,
        priority_delta: 1,
        deadline_unix_ms: 0,
        flags: 0,
    }
    .to_bytes()
    .expect("scheduling payload")
    .to_vec()
}

fn supersede_payload(operation_id: u64) -> Vec<u8> {
    SupersedeMetadata {
        old_operation_id: operation_id,
        new_operation_id: operation_id + 100,
        control_sequence: 3,
        drop_reason_code: 1,
        flags: 0,
        diagnostic_bytes: 2,
    }
    .to_vec_with_diagnostics(b"ok")
    .expect("supersede payload")
}

fn budget_payload(operation_id: u64) -> Vec<u8> {
    BudgetMetadata {
        operation_id,
        compute_budget_units: 100,
        memory_budget_bytes: 4096,
        bandwidth_budget_bytes: 8192,
        token_budget: 32,
        flags: 0,
    }
    .to_bytes()
    .expect("budget payload")
    .to_vec()
}

fn progress_payload(operation_id: u64) -> Vec<u8> {
    ProgressMetadata {
        operation_id,
        progress_sequence: 1,
        stage_code: 2,
        percent_x100: 2500,
        object_id: 0,
        body_bytes: 4,
    }
    .to_vec_with_body(b"step")
    .expect("progress payload")
}

fn partial_payload(operation_id: u64, sequence: u64) -> Vec<u8> {
    PartialResultMetadata {
        operation_id,
        result_sequence: sequence,
        object_id: 0,
        delta_sequence: 0,
        body_bytes: 4,
        flags: 0,
    }
    .to_vec_with_body(b"part")
    .expect("partial payload")
}

fn pressure_payload(backpressure: bool) -> Vec<u8> {
    PressureMetadata {
        scope_id: 1,
        credit_window: if backpressure { 2 } else { 9 },
        pressure_level: if backpressure {
            BackpressureLevel::Soft as u16
        } else {
            BackpressureLevel::None as u16
        },
        pressure_reason: if backpressure { 1 } else { 0 },
        retry_after_ms: if backpressure { 5 } else { 0 },
        flags: 0,
    }
    .to_bytes()
    .expect("pressure payload")
    .to_vec()
}

fn flow_update_payload() -> Vec<u8> {
    FlowUpdateMetadata {
        scope_kind: FlowScopeKind::Session,
        update_reason: FlowUpdateReason::Grant,
        backpressure_level: BackpressureLevel::None,
        connection_credit: 0,
        session_credit: 7,
        operation_credit: 0,
        operation_id: 0,
        retry_after_ms: 0,
        credit_epoch: 1,
        flow_flags: FLOW_UPDATE_FLAG_CREDIT_VALID,
    }
    .to_bytes()
    .expect("flow update payload")
    .to_vec()
}

fn capability_payload() -> Vec<u8> {
    CapabilityMetadata {
        profile_id: PROFILE_TOKEN,
        capability_count: 1,
        cost_model_id: 1,
        preference_rank: 1,
        limit_bytes: 4096,
        limit_units: 8,
        body_bytes: 4,
        flags: 0,
    }
    .to_vec_with_body(b"caps")
    .expect("capability payload")
}

fn route_payload(operation_id: u64) -> Vec<u8> {
    RouteHintMetadata {
        operation_id,
        route_id: 9,
        executor_class: 3,
        affinity_class: 4,
        deadline_unix_ms: 1_800_000_000_000,
        body_bytes: 4,
        flags: 0,
    }
    .to_vec_with_body(b"hint")
    .expect("route payload")
}

fn trace_payload() -> Vec<u8> {
    TraceContextMetadata {
        trace_id: 11,
        span_id: 12,
        parent_span_id: 10,
        stage_code: 3,
        flags: 0,
        body_bytes: 5,
    }
    .to_vec_with_body(b"trace")
    .expect("trace payload")
}

fn drop_reason_payload(operation_id: u64, source_role: RuntimeRole) -> Vec<u8> {
    ResultDropReasonMetadata {
        operation_id,
        result_sequence: 1,
        drop_reason_code: 1,
        source_role: source_role as u8,
        flags: 0,
        diagnostic_bytes: 4,
    }
    .to_vec_with_diagnostics(b"drop")
    .expect("drop reason payload")
}

fn recoverable_error_payload(source_role: RuntimeRole, frame_id: u32) -> Vec<u8> {
    RecoverableErrorMetadata {
        error_code: 1,
        error_scope: ErrorScope::Frame,
        recovery_action: 1,
        source_role: source_role as u8,
        flags: 0,
        retry_after_ms: 5,
        related_session_id: 0,
        related_frame_id: frame_id,
        related_view_id: 0,
        diagnostic_bytes: 3,
    }
    .to_vec_with_diagnostics(b"err")
    .expect("recoverable error payload")
}

fn retry_after_payload(source_role: RuntimeRole) -> Vec<u8> {
    RetryAfterMetadata {
        scope_id: 1,
        control_sequence: 4,
        retry_after_ms: 5,
        jitter_ms: 1,
        reason_code: 1,
        source_role: source_role as u8,
        flags: 0,
        diagnostic_bytes: 4,
    }
    .to_vec_with_diagnostics(b"wait")
    .expect("retry-after payload")
}

fn result_hint_payload() -> Vec<u8> {
    ResultHintMetadata {
        applied_budget_policy: ResultHintBudgetPolicy::Partial,
        congestion_state: ResultHintCongestionState::Elevated,
        reason: ResultHintReason::ServerBusy,
        retry_after_ms: 8,
    }
    .to_bytes()
    .expect("result hint payload")
    .to_vec()
}

fn object_declare_payload(
    session_id: u32,
    producer: RuntimeRole,
    consumer: RuntimeRole,
) -> Vec<u8> {
    ObjectDescriptorMetadata {
        object_id: 900,
        object_kind: RuntimeObjectKind::ImageTile,
        producer_role: producer,
        consumer_role: consumer,
        session_id,
        byte_size: 4,
        compute_cost_units: 2,
        memory_location_hint: MemoryLocationHint::HostMemory,
        ownership_hint: OwnershipHint::SessionOwned,
        lifetime_hint_ms: 1_000,
        metadata_bytes: 4,
    }
    .to_vec_with_extension(b"meta")
    .expect("object declaration payload")
}

fn object_ref_payload(operation_id: u64) -> Vec<u8> {
    ObjectReferenceMetadata {
        object_id: 900,
        operation_id,
        object_version: 1,
        offset: 0,
        length: 4,
        flags: 0,
        metadata_bytes: 0,
    }
    .to_vec_with_extension(&[])
    .expect("object reference payload")
}

fn object_release_payload(operation_id: u64, source_role: RuntimeRole) -> Vec<u8> {
    ObjectReleaseMetadata {
        object_id: 900,
        operation_id,
        release_reason: ObjectReleaseReason::Completed,
        source_role,
        flags: 0,
        diagnostic_bytes: 0,
    }
    .to_vec_with_diagnostics(&[])
    .expect("object release payload")
}

fn object_delta_payload() -> Vec<u8> {
    let metadata = ObjectDeltaMetadata {
        object_id: 900,
        delta_sequence: 1,
        region_offset: 0,
        region_bytes: 4,
        delta_bytes: 4,
        flags: 0,
        metadata_bytes: 0,
    };
    let mut payload = metadata.to_bytes().expect("object delta metadata").to_vec();
    payload.extend_from_slice(b"data");
    payload
}

fn cache_reference_payload() -> Vec<u8> {
    CacheReferenceMetadata {
        cache_key_hi: 0x1234,
        cache_key_lo: 0x5678,
        profile_id: PROFILE_TOKEN,
        reuse_scope: CacheReuseScope::Session,
        lease_id: 0,
        producer_trace_id: 99,
        expiration_hint_ms: 1_000,
        metadata_bytes: 4,
        flags: 0,
    }
    .to_vec_with_extension(b"meta")
    .expect("cache reference payload")
}

fn cache_miss_payload() -> Vec<u8> {
    CacheMissMetadata {
        cache_key_hi: 0x1234,
        cache_key_lo: 0x5678,
        miss_reason: CacheMissReason::SchemaMismatch,
        profile_id: PROFILE_TOKEN,
        diagnostic_bytes: 4,
    }
    .to_vec_with_diagnostics(b"miss")
    .expect("cache miss payload")
}

fn cache_invalidate_payload() -> Vec<u8> {
    CacheInvalidateMetadata {
        invalidate_scope: CacheInvalidateScope::ObjectKey,
        cache_namespace: 42,
        cache_key_hi: 0x1234,
        cache_key_lo: 0x5678,
        reason_code: 77,
    }
    .to_bytes()
    .expect("cache invalidate payload")
    .to_vec()
}

struct RuntimeFrameCase {
    message_type: MessageType,
    frame_id: u32,
    operation_scoped: bool,
    payload: Vec<u8>,
}

fn bidirectional_runtime_frames(
    operation_id: u64,
    frame_id: u32,
    session_id: u32,
    source_role: RuntimeRole,
    peer_role: RuntimeRole,
) -> Vec<RuntimeFrameCase> {
    vec![
        RuntimeFrameCase {
            message_type: MessageType::PriorityUpdate,
            frame_id,
            operation_scoped: true,
            payload: scheduling_payload(operation_id),
        },
        RuntimeFrameCase {
            message_type: MessageType::Deadline,
            frame_id,
            operation_scoped: true,
            payload: SchedulingMetadata {
                operation_id,
                control_sequence: 3,
                priority_class: 0,
                priority_delta: 0,
                deadline_unix_ms: 1_800_000_000_000,
                flags: 0,
            }
            .to_bytes()
            .expect("deadline payload")
            .to_vec(),
        },
        RuntimeFrameCase {
            message_type: MessageType::ExpireAt,
            frame_id,
            operation_scoped: true,
            payload: SchedulingMetadata {
                operation_id,
                control_sequence: 4,
                priority_class: 0,
                priority_delta: 0,
                deadline_unix_ms: 1_800_000_000_001,
                flags: 0,
            }
            .to_bytes()
            .expect("expire-at payload")
            .to_vec(),
        },
        RuntimeFrameCase {
            message_type: MessageType::Supersede,
            frame_id,
            operation_scoped: true,
            payload: supersede_payload(operation_id),
        },
        RuntimeFrameCase {
            message_type: MessageType::BudgetUpdate,
            frame_id,
            operation_scoped: true,
            payload: budget_payload(operation_id),
        },
        RuntimeFrameCase {
            message_type: MessageType::Progress,
            frame_id,
            operation_scoped: true,
            payload: progress_payload(operation_id),
        },
        RuntimeFrameCase {
            message_type: MessageType::PartialResult,
            frame_id,
            operation_scoped: true,
            payload: partial_payload(operation_id, 10),
        },
        RuntimeFrameCase {
            message_type: MessageType::FlowUpdate,
            frame_id: 0,
            operation_scoped: false,
            payload: flow_update_payload(),
        },
        RuntimeFrameCase {
            message_type: MessageType::Backpressure,
            frame_id: 0,
            operation_scoped: false,
            payload: pressure_payload(true),
        },
        RuntimeFrameCase {
            message_type: MessageType::CreditUpdate,
            frame_id: 0,
            operation_scoped: false,
            payload: pressure_payload(false),
        },
        RuntimeFrameCase {
            message_type: MessageType::CapabilityNegotiation,
            frame_id: 0,
            operation_scoped: false,
            payload: capability_payload(),
        },
        RuntimeFrameCase {
            message_type: MessageType::DegradeProfile,
            frame_id: 0,
            operation_scoped: false,
            payload: capability_payload(),
        },
        RuntimeFrameCase {
            message_type: MessageType::RouteHint,
            frame_id,
            operation_scoped: true,
            payload: route_payload(operation_id),
        },
        RuntimeFrameCase {
            message_type: MessageType::ExecutionHint,
            frame_id,
            operation_scoped: true,
            payload: route_payload(operation_id),
        },
        RuntimeFrameCase {
            message_type: MessageType::TraceContext,
            frame_id,
            operation_scoped: true,
            payload: trace_payload(),
        },
        RuntimeFrameCase {
            message_type: MessageType::ErrorRecoverable,
            frame_id,
            operation_scoped: true,
            payload: recoverable_error_payload(source_role, frame_id),
        },
        RuntimeFrameCase {
            message_type: MessageType::RetryAfter,
            frame_id: 0,
            operation_scoped: false,
            payload: retry_after_payload(source_role),
        },
        RuntimeFrameCase {
            message_type: MessageType::ObjectDeclare,
            frame_id: 0,
            operation_scoped: false,
            payload: object_declare_payload(session_id, source_role, peer_role),
        },
        RuntimeFrameCase {
            message_type: MessageType::ObjectRef,
            frame_id,
            operation_scoped: true,
            payload: object_ref_payload(operation_id),
        },
        RuntimeFrameCase {
            message_type: MessageType::ObjectRelease,
            frame_id,
            operation_scoped: true,
            payload: object_release_payload(operation_id, source_role),
        },
        RuntimeFrameCase {
            message_type: MessageType::ObjectPatch,
            frame_id: 0,
            operation_scoped: false,
            payload: object_delta_payload(),
        },
        RuntimeFrameCase {
            message_type: MessageType::ObjectDelta,
            frame_id: 0,
            operation_scoped: false,
            payload: object_delta_payload(),
        },
        RuntimeFrameCase {
            message_type: MessageType::CacheReference,
            frame_id: 0,
            operation_scoped: false,
            payload: cache_reference_payload(),
        },
        RuntimeFrameCase {
            message_type: MessageType::CacheMiss,
            frame_id: 0,
            operation_scoped: false,
            payload: cache_miss_payload(),
        },
        RuntimeFrameCase {
            message_type: MessageType::CacheInvalidate,
            frame_id: 0,
            operation_scoped: false,
            payload: cache_invalidate_payload(),
        },
    ]
}

unsafe fn submit_role_operation(
    client_session: NnrpHandle,
    server_session: NnrpHandle,
    operation_id: u64,
    frame_id: u32,
) -> (NnrpHandle, NnrpHandle) {
    let mut payload = token_submit(operation_id)
        .to_bytes()
        .expect("submit metadata")
        .to_vec();
    payload.extend_from_slice(b"operation");
    let mut client_operation = NnrpHandle::invalid();
    assert_eq!(
        nnrp_client_submit(
            NnrpSubmitRequest {
                session: client_session,
                operation_id,
                frame_id,
                payload: view(&payload),
            },
            &mut client_operation,
        ),
        NnrpFfiStatus::ok()
    );
    let server_event = poll_server_event(server_session);
    assert_eq!(server_event.kind, NnrpEventKind::SubmitAccepted as u32);
    assert_eq!(server_event.frame_id, frame_id);
    assert_eq!(
        slice::from_raw_parts(server_event.payload.ptr, server_event.payload.len),
        payload
    );
    assert_eq!(
        nnrp_buffer_release(server_event.payload_owner),
        NnrpFfiStatus::ok()
    );
    (client_operation, server_event.operation)
}

unsafe fn assert_role_handshake(transport_id: TransportId, listen_endpoint: &str, id_base: u64) {
    let mut listener = NnrpHandle::invalid();
    assert_eq!(
        nnrp_transport_listen(open_request(transport_id, listen_endpoint), &mut listener,),
        NnrpFfiStatus::ok()
    );

    let mut endpoint_owner = NnrpHandle::invalid();
    let mut endpoint_view = NnrpBufferView::empty();
    assert_eq!(
        nnrp_transport_listener_endpoint(listener, &mut endpoint_owner, &mut endpoint_view,),
        NnrpFfiStatus::ok()
    );
    let endpoint =
        String::from_utf8(slice::from_raw_parts(endpoint_view.ptr, endpoint_view.len).to_vec())
            .expect("listener endpoint must be UTF-8");
    assert_eq!(nnrp_buffer_release(endpoint_owner), NnrpFfiStatus::ok());

    let mut foreign_listener = listener;
    foreign_listener.flags ^= u32::MAX;
    let mut rejected_server = NnrpHandle::invalid();
    assert_eq!(
        nnrp_server_bind(
            NnrpServerBindRequest {
                server_id: id_base,
                generation: 1,
                reserved0: 0,
                transport_listener: foreign_listener,
            },
            &mut rejected_server,
        ),
        NnrpFfiStatus::invalid_handle(NnrpHandleKind::TransportListener as u32)
    );

    let mut server = NnrpHandle::invalid();
    assert_eq!(
        nnrp_server_bind(
            NnrpServerBindRequest {
                server_id: id_base + 1,
                generation: 1,
                reserved0: 0,
                transport_listener: listener,
            },
            &mut server,
        ),
        NnrpFfiStatus::ok()
    );
    assert_eq!(
        nnrp_transport_listener_endpoint(listener, &mut endpoint_owner, &mut endpoint_view,),
        NnrpFfiStatus::invalid_handle(NnrpHandleKind::TransportListener as u32)
    );

    let accept = thread::spawn(move || {
        let mut session = NnrpHandle::invalid();
        let status = nnrp_server_accept(
            NnrpServerAcceptRequest {
                server,
                session_handle_id: id_base + 4,
                generation: 1,
                timeout_ms: 5_000,
            },
            &mut session,
        );
        (status, session, server)
    });

    let mut transport_connection = NnrpHandle::invalid();
    assert_eq!(
        nnrp_transport_connect(
            open_request(transport_id, &endpoint),
            &mut transport_connection,
        ),
        NnrpFfiStatus::ok()
    );
    let mut foreign_connection = transport_connection;
    foreign_connection.flags ^= u32::MAX;
    let mut rejected_client = NnrpHandle::invalid();
    assert_eq!(
        nnrp_client_connect(
            NnrpClientConnectRequest {
                connection_id: id_base + 5,
                generation: 1,
                reserved0: 0,
                transport_connection: foreign_connection,
            },
            &mut rejected_client,
        ),
        NnrpFfiStatus::invalid_handle(NnrpHandleKind::TransportConnection as u32)
    );
    let mut client = NnrpHandle::invalid();
    assert_eq!(
        nnrp_client_connect(
            NnrpClientConnectRequest {
                connection_id: id_base + 2,
                generation: 1,
                reserved0: 0,
                transport_connection,
            },
            &mut client,
        ),
        NnrpFfiStatus::ok()
    );
    let mut consumed_batch = NnrpTransportFrameBatch::empty();
    assert_eq!(
        nnrp_transport_read_batch(
            NnrpTransportReadBatchRequest {
                connection: transport_connection,
                max_frames: 1,
                timeout_ms: 1,
                max_bytes: 0,
            },
            &mut consumed_batch,
        ),
        NnrpFfiStatus::invalid_handle(NnrpHandleKind::TransportConnection as u32)
    );

    let mut client_session = NnrpHandle::invalid();
    assert_eq!(
        nnrp_client_open_session(
            NnrpSessionOpenRequest {
                connection: client,
                requested_session_id: (id_base + 3) as u32,
                generation: 1,
                profile_id: PROFILE_TOKEN,
                schema_id: TOKEN_DELTA_SCHEMA_ID,
                schema_version: TOKEN_DELTA_SCHEMA_VERSION,
            },
            &mut client_session,
        ),
        NnrpFfiStatus::ok()
    );

    let (server_status, server_session, server) = accept.join().expect("accept thread joins");
    assert_eq!(server_status, NnrpFfiStatus::ok());
    assert_ne!(client_session, NnrpHandle::invalid());
    assert_ne!(server_session, NnrpHandle::invalid());

    let submit_body = b"role-carrier-submit";
    let mut submit_payload = token_submit(id_base + 6)
        .to_bytes()
        .expect("submit metadata")
        .to_vec();
    submit_payload.extend_from_slice(submit_body);
    let mut client_operation = NnrpHandle::invalid();
    let submit_request = NnrpSubmitRequest {
        session: client_session,
        operation_id: id_base + 6,
        frame_id: 42,
        payload: view(&submit_payload),
    };
    assert_eq!(
        nnrp_client_submit(submit_request, ptr::null_mut()),
        NnrpFfiStatus::invalid_argument(12)
    );
    assert_eq!(
        nnrp_client_submit(
            NnrpSubmitRequest {
                payload: NnrpBufferView {
                    ptr: ptr::null(),
                    len: 1,
                },
                ..submit_request
            },
            &mut client_operation,
        ),
        NnrpFfiStatus::invalid_argument(1)
    );
    assert_eq!(
        nnrp_client_submit(
            NnrpSubmitRequest {
                payload: view(&submit_payload[..1]),
                ..submit_request
            },
            &mut client_operation,
        ),
        NnrpFfiStatus::invalid_argument(144)
    );
    let malformed_submit = [u8::MAX; 72];
    assert_ne!(
        nnrp_client_submit(
            NnrpSubmitRequest {
                payload: view(&malformed_submit),
                ..submit_request
            },
            &mut client_operation,
        ),
        NnrpFfiStatus::ok()
    );
    assert_ne!(
        nnrp_client_submit(
            NnrpSubmitRequest {
                session: NnrpHandle::invalid(),
                ..submit_request
            },
            &mut client_operation,
        ),
        NnrpFfiStatus::ok()
    );
    assert_eq!(
        nnrp_client_submit(submit_request, &mut client_operation),
        NnrpFfiStatus::ok()
    );
    assert_ne!(client_operation.id, submit_request.operation_id);
    assert_eq!(
        nnrp_client_submit(submit_request, &mut NnrpHandle::invalid()),
        NnrpFfiStatus::invalid_argument(145)
    );
    assert_ne!(
        nnrp_client_submit(
            NnrpSubmitRequest {
                operation_id: id_base + 7,
                frame_id: 41,
                ..submit_request
            },
            &mut NnrpHandle::invalid(),
        ),
        NnrpFfiStatus::ok()
    );

    let mut event_count = 0usize;
    assert_eq!(
        nnrp_client_await_event(client_session, ptr::null_mut()),
        NnrpFfiStatus::invalid_argument(17)
    );
    assert_eq!(
        nnrp_client_await_events(
            poll_request(client_session),
            ptr::null_mut(),
            0,
            &mut event_count,
        ),
        NnrpFfiStatus::invalid_argument(32)
    );
    assert_eq!(
        nnrp_client_await_events(
            poll_request(client_session),
            &mut NnrpEvent::none(),
            1,
            ptr::null_mut(),
        ),
        NnrpFfiStatus::invalid_argument(31)
    );
    assert_eq!(
        nnrp_client_await_events(
            NnrpRoleEventPollRequest {
                flags: 1,
                ..poll_request(client_session)
            },
            &mut NnrpEvent::none(),
            1,
            &mut event_count,
        ),
        NnrpFfiStatus::invalid_argument(146)
    );
    assert_ne!(
        nnrp_client_await_events(
            poll_request(server_session),
            &mut NnrpEvent::none(),
            1,
            &mut event_count,
        ),
        NnrpFfiStatus::ok()
    );
    assert_ne!(
        nnrp_client_await_events(
            NnrpRoleEventPollRequest {
                scope: NnrpHandle::invalid(),
                max_events: 0,
                ..poll_request(client_session)
            },
            &mut NnrpEvent::none(),
            1,
            &mut event_count,
        ),
        NnrpFfiStatus::ok()
    );
    let mut invalid_poll = NnrpPollResult {
        status: NnrpFfiStatus::ok(),
        has_event: 0,
        event: NnrpEvent::none(),
    };
    let invalid_poll_status = nnrp_client_await_event(NnrpHandle::invalid(), &mut invalid_poll);
    assert_ne!(invalid_poll_status, NnrpFfiStatus::ok());
    assert_eq!(invalid_poll.status, invalid_poll_status);
    assert_eq!(invalid_poll.has_event, 0);
    assert_eq!(
        nnrp_server_await_events(
            poll_request(server_session),
            ptr::null_mut(),
            0,
            &mut event_count,
        ),
        NnrpFfiStatus::invalid_argument(32)
    );

    let mut server_events = [NnrpEvent::none(); 2];
    let mut server_event_count = 0usize;
    assert_eq!(
        nnrp_server_await_events(
            NnrpRoleEventPollRequest {
                max_events: 2,
                ..poll_request(server_session)
            },
            server_events.as_mut_ptr(),
            server_events.len(),
            &mut server_event_count,
        ),
        NnrpFfiStatus::ok()
    );
    let server_event = server_events[0];
    assert_eq!(server_event_count, 1);
    assert_eq!(server_event.kind, NnrpEventKind::SubmitAccepted as u32);
    assert_eq!(server_event.frame_id, 42);
    assert_ne!(server_event.operation.id, submit_request.operation_id);
    assert_ne!(server_event.operation, client_operation);
    assert_eq!(
        slice::from_raw_parts(server_event.payload.ptr, server_event.payload.len),
        submit_payload
    );
    assert_eq!(
        nnrp_buffer_release(server_event.payload_owner),
        NnrpFfiStatus::ok()
    );

    for case in bidirectional_runtime_frames(
        submit_request.operation_id,
        submit_request.frame_id,
        (id_base + 3) as u32,
        RuntimeRole::Client,
        RuntimeRole::Server,
    ) {
        send_runtime_frame(
            if case.operation_scoped {
                client_operation
            } else {
                client_session
            },
            case.message_type,
            case.frame_id,
            &case.payload,
        );
        assert_runtime_event(
            poll_server_event(server_session),
            case.message_type,
            case.operation_scoped.then_some(server_event.operation),
            &case.payload,
        );
    }

    let client_drop_reason = drop_reason_payload(submit_request.operation_id, RuntimeRole::Client);
    send_runtime_frame(
        client_operation,
        MessageType::ResultDropReason,
        submit_request.frame_id,
        &client_drop_reason,
    );
    assert_runtime_event(
        poll_server_event(server_session),
        MessageType::ResultDropReason,
        Some(server_event.operation),
        &client_drop_reason,
    );

    for case in bidirectional_runtime_frames(
        submit_request.operation_id,
        submit_request.frame_id,
        (id_base + 3) as u32,
        RuntimeRole::Server,
        RuntimeRole::Client,
    ) {
        send_runtime_frame(
            if case.operation_scoped {
                server_event.operation
            } else {
                server_session
            },
            case.message_type,
            case.frame_id,
            &case.payload,
        );
        assert_runtime_event(
            poll_client_event(client_session),
            case.message_type,
            case.operation_scoped.then_some(client_operation),
            &case.payload,
        );
    }

    let result_hint = result_hint_payload();
    assert_ne!(
        nnrp_runtime_frame_send(NnrpRuntimeFrameSendRequest {
            handle: client_session,
            message_type: MessageType::ResultHint as u32,
            frame_id: 0,
            payload: view(&result_hint),
        }),
        NnrpFfiStatus::ok()
    );
    send_runtime_frame(server_session, MessageType::ResultHint, 0, &result_hint);
    assert_runtime_event(
        poll_client_event(client_session),
        MessageType::ResultHint,
        None,
        &result_hint,
    );

    for message_type in [MessageType::Cancel, MessageType::Abort] {
        let payload = control_payload(submit_request.operation_id, RuntimeRole::Server);
        send_runtime_frame(
            server_event.operation,
            message_type,
            submit_request.frame_id,
            &payload,
        );
        assert_runtime_event(
            poll_client_event(client_session),
            message_type,
            Some(client_operation),
            &payload,
        );
    }

    let partial_body = b"partial";
    let mut partial_payload = PartialResultMetadata {
        operation_id: submit_request.operation_id,
        result_sequence: 1,
        object_id: 0,
        delta_sequence: 0,
        body_bytes: partial_body.len() as u32,
        flags: 0,
    }
    .to_bytes()
    .expect("partial metadata")
    .to_vec();
    partial_payload.extend_from_slice(partial_body);
    assert_eq!(
        nnrp_runtime_frame_send(NnrpRuntimeFrameSendRequest {
            handle: server_event.operation,
            message_type: nnrp_core::MessageType::PartialResult as u32,
            frame_id: server_event.frame_id,
            payload: view(&partial_payload),
        }),
        NnrpFfiStatus::ok()
    );
    let mut partial_event = NnrpEvent::none();
    let mut partial_event_count = 0usize;
    assert_eq!(
        nnrp_client_await_events(
            poll_request(client_session),
            &mut partial_event,
            1,
            &mut partial_event_count,
        ),
        NnrpFfiStatus::ok()
    );
    assert_eq!(partial_event_count, 1);
    assert_eq!(partial_event.kind, NnrpEventKind::PartialResult as u32);
    assert_eq!(partial_event.operation, client_operation);
    assert_eq!(partial_event.frame_id, submit_request.frame_id);
    assert_eq!(
        slice::from_raw_parts(partial_event.payload.ptr, partial_event.payload.len),
        partial_payload
    );
    assert_eq!(
        nnrp_buffer_release(partial_event.payload_owner),
        NnrpFfiStatus::ok()
    );

    let direct_partial_body = b"direct";
    let direct_partial = PartialResultMetadata {
        operation_id: submit_request.operation_id,
        result_sequence: 2,
        object_id: 0,
        delta_sequence: 0,
        body_bytes: direct_partial_body.len() as u32,
        flags: 0,
    };
    let mut direct_send_result = NnrpPollResult {
        status: NnrpFfiStatus::ok(),
        has_event: 1,
        event: NnrpEvent::none(),
    };
    assert_eq!(
        nnrp_server_send_partial_result(
            NnrpServerSendPartialResultRequest {
                operation: server_event.operation,
                partial_result: direct_partial.into(),
                partial_body: view(direct_partial_body),
                max_events: 1,
            },
            &mut direct_send_result,
        ),
        NnrpFfiStatus::ok()
    );
    assert_eq!(direct_send_result.has_event, 0);
    let mut direct_partial_event = NnrpEvent::none();
    let mut direct_partial_event_count = 0usize;
    assert_eq!(
        nnrp_client_await_events(
            poll_request(client_session),
            &mut direct_partial_event,
            1,
            &mut direct_partial_event_count,
        ),
        NnrpFfiStatus::ok()
    );
    assert_eq!(direct_partial_event_count, 1);
    assert_eq!(
        direct_partial_event.kind,
        NnrpEventKind::PartialResult as u32
    );
    assert_eq!(direct_partial_event.operation, client_operation);
    assert_eq!(
        slice::from_raw_parts(
            direct_partial_event.payload.ptr,
            direct_partial_event.payload.len,
        ),
        direct_partial
            .to_vec_with_body(direct_partial_body)
            .expect("direct partial payload")
    );
    assert_eq!(
        nnrp_buffer_release(direct_partial_event.payload_owner),
        NnrpFfiStatus::ok()
    );

    let result_body = b"role-carrier-result";
    let mut result_payload = token_result().to_bytes().expect("result metadata").to_vec();
    result_payload.extend_from_slice(result_body);
    assert_eq!(
        nnrp_server_send_result(NnrpServerSendResultRequest {
            operation: NnrpHandle::invalid(),
            payload: view(&result_payload),
        })
        .status_code,
        nnrp_ffi::NnrpFfiStatusCode::InvalidHandle as u32
    );
    assert_eq!(
        nnrp_server_send_result(NnrpServerSendResultRequest {
            operation: server_event.operation,
            payload: view(&result_payload[..1]),
        }),
        NnrpFfiStatus::invalid_argument(148)
    );
    let malformed_result = [u8::MAX; 64];
    assert_ne!(
        nnrp_server_send_result(NnrpServerSendResultRequest {
            operation: server_event.operation,
            payload: view(&malformed_result),
        }),
        NnrpFfiStatus::ok()
    );
    assert_eq!(
        nnrp_server_send_result(NnrpServerSendResultRequest {
            operation: server_event.operation,
            payload: view(&result_payload),
        }),
        NnrpFfiStatus::ok()
    );

    let mut client_events = [NnrpEvent::none(); 2];
    let mut client_event_count = 0usize;
    assert_eq!(
        nnrp_client_await_events(
            NnrpRoleEventPollRequest {
                max_events: 2,
                ..poll_request(client_session)
            },
            client_events.as_mut_ptr(),
            client_events.len(),
            &mut client_event_count,
        ),
        NnrpFfiStatus::ok()
    );
    let client_event = client_events[0];
    assert_eq!(client_event_count, 1);
    assert_eq!(client_event.kind, NnrpEventKind::ResultPushed as u32);
    assert_eq!(client_event.operation, client_operation);
    assert_eq!(client_event.frame_id, 42);
    assert_eq!(
        slice::from_raw_parts(client_event.payload.ptr, client_event.payload.len),
        result_payload
    );
    assert_eq!(
        nnrp_buffer_release(client_event.payload_owner),
        NnrpFfiStatus::ok()
    );

    for (offset, message_type) in [MessageType::Cancel, MessageType::Abort]
        .into_iter()
        .enumerate()
    {
        let operation_id = id_base + 20 + offset as u64;
        let frame_id = 50 + offset as u32;
        let (client_control_operation, server_control_operation) =
            submit_role_operation(client_session, server_session, operation_id, frame_id);
        let payload = control_payload(operation_id, RuntimeRole::Client);
        send_runtime_frame(client_control_operation, message_type, frame_id, &payload);
        assert_runtime_event(
            poll_server_event(server_session),
            message_type,
            Some(server_control_operation),
            &payload,
        );
    }

    let frame_cancel_id = 59;
    let (_client_frame_cancel_operation, server_frame_cancel_operation) = submit_role_operation(
        client_session,
        server_session,
        id_base + 29,
        frame_cancel_id,
    );
    assert_eq!(
        nnrp_client_cancel(NnrpClientCancelRequest {
            session: client_session,
            frame_id: frame_cancel_id,
        }),
        NnrpFfiStatus::ok()
    );
    let frame_cancel_event = poll_server_event(server_session);
    assert_eq!(frame_cancel_event.kind, NnrpEventKind::Control as u32);
    assert_eq!(
        frame_cancel_event.message_type,
        MessageType::FrameCancel as u32
    );
    assert_eq!(frame_cancel_event.operation, server_frame_cancel_operation);
    assert_eq!(frame_cancel_event.frame_id, frame_cancel_id);
    assert_eq!(frame_cancel_event.payload.len, 0);

    let drop_operation_id = id_base + 30;
    let drop_frame_id = 60;
    let (client_drop_operation, server_drop_operation) = submit_role_operation(
        client_session,
        server_session,
        drop_operation_id,
        drop_frame_id,
    );
    let server_drop_reason = drop_reason_payload(drop_operation_id, RuntimeRole::Server);
    send_runtime_frame(
        server_drop_operation,
        MessageType::ResultDropReason,
        drop_frame_id,
        &server_drop_reason,
    );
    assert_runtime_event(
        poll_client_event(client_session),
        MessageType::ResultDropReason,
        Some(client_drop_operation),
        &server_drop_reason,
    );

    let client_close = thread::spawn(move || nnrp_client_close(client_session));
    let close_event = poll_server_event(server_session);
    assert_eq!(close_event.kind, NnrpEventKind::SessionClosed as u32);
    assert_eq!(close_event.message_type, MessageType::SessionClose as u32);
    assert_eq!(
        nnrp_buffer_release(close_event.payload_owner),
        NnrpFfiStatus::ok()
    );
    assert_eq!(nnrp_server_close(server_session), NnrpFfiStatus::ok());
    assert_eq!(
        client_close.join().expect("client close thread joins"),
        NnrpFfiStatus::ok()
    );
    assert_eq!(nnrp_connection_close(client), NnrpFfiStatus::ok());
    assert_eq!(nnrp_connection_close(server), NnrpFfiStatus::ok());
}

#[test]
fn tcp_role_runtime_adopts_carriers_and_completes_real_handshake() {
    unsafe {
        assert_role_handshake(TransportId::Tcp, "tcp://127.0.0.1:0", 700_000);
    }
}

#[test]
fn role_runtime_rejects_invalid_arguments_and_cross_role_handles() {
    unsafe {
        let mut output = NnrpHandle::invalid();
        assert_eq!(
            nnrp_client_connect(
                NnrpClientConnectRequest {
                    connection_id: 0,
                    generation: 1,
                    reserved0: 0,
                    transport_connection: NnrpHandle::invalid(),
                },
                &mut output,
            ),
            NnrpFfiStatus::invalid_argument(10)
        );
        assert_eq!(
            nnrp_client_open_session(
                NnrpSessionOpenRequest {
                    connection: NnrpHandle::invalid(),
                    requested_session_id: 0,
                    generation: 1,
                    profile_id: PROFILE_TOKEN,
                    schema_id: TOKEN_DELTA_SCHEMA_ID,
                    schema_version: TOKEN_DELTA_SCHEMA_VERSION,
                },
                &mut output,
            ),
            NnrpFfiStatus::invalid_argument(11)
        );
        assert_eq!(
            nnrp_server_bind(
                NnrpServerBindRequest {
                    server_id: 0,
                    generation: 1,
                    reserved0: 0,
                    transport_listener: NnrpHandle::invalid(),
                },
                &mut output,
            ),
            NnrpFfiStatus::invalid_argument(18)
        );
        assert_eq!(
            nnrp_server_accept(
                NnrpServerAcceptRequest {
                    server: NnrpHandle::invalid(),
                    session_handle_id: 0,
                    generation: 1,
                    timeout_ms: 1,
                },
                &mut output,
            ),
            NnrpFfiStatus::invalid_argument(19)
        );

        let mut listener = NnrpHandle::invalid();
        assert_eq!(
            nnrp_transport_listen(
                open_request(TransportId::Tcp, "tcp://127.0.0.1:0"),
                &mut listener,
            ),
            NnrpFfiStatus::ok()
        );
        let mut endpoint_owner = NnrpHandle::invalid();
        let mut endpoint_view = NnrpBufferView::empty();
        assert_eq!(
            nnrp_transport_listener_endpoint(listener, &mut endpoint_owner, &mut endpoint_view),
            NnrpFfiStatus::ok()
        );
        let endpoint =
            String::from_utf8(slice::from_raw_parts(endpoint_view.ptr, endpoint_view.len).to_vec())
                .expect("listener endpoint must be UTF-8");
        assert_eq!(nnrp_buffer_release(endpoint_owner), NnrpFfiStatus::ok());

        let mut server = NnrpHandle::invalid();
        assert_eq!(
            nnrp_server_bind(
                NnrpServerBindRequest {
                    server_id: 730_000,
                    generation: 1,
                    reserved0: 0,
                    transport_listener: listener,
                },
                &mut server,
            ),
            NnrpFfiStatus::ok()
        );
        let mut transport_connection = NnrpHandle::invalid();
        assert_eq!(
            nnrp_transport_connect(
                open_request(TransportId::Tcp, &endpoint),
                &mut transport_connection,
            ),
            NnrpFfiStatus::ok()
        );
        let mut client = NnrpHandle::invalid();
        assert_eq!(
            nnrp_client_connect(
                NnrpClientConnectRequest {
                    connection_id: 730_001,
                    generation: 1,
                    reserved0: 0,
                    transport_connection,
                },
                &mut client,
            ),
            NnrpFfiStatus::ok()
        );

        let session_request = |connection| NnrpSessionOpenRequest {
            connection,
            requested_session_id: 730_002,
            generation: 1,
            profile_id: PROFILE_TOKEN,
            schema_id: TOKEN_DELTA_SCHEMA_ID,
            schema_version: TOKEN_DELTA_SCHEMA_VERSION,
        };
        assert_eq!(
            nnrp_client_open_session(session_request(server), &mut output),
            NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
        );
        assert_eq!(
            nnrp_client_open_session(session_request(NnrpHandle::invalid()), &mut output),
            NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
        );

        let accept_request = |server| NnrpServerAcceptRequest {
            server,
            session_handle_id: 730_003,
            generation: 1,
            timeout_ms: 1,
        };
        assert_eq!(
            nnrp_server_accept(accept_request(client), &mut output),
            NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
        );
        assert_eq!(
            nnrp_server_accept(accept_request(NnrpHandle::invalid()), &mut output),
            NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
        );

        assert_eq!(nnrp_connection_close(client), NnrpFfiStatus::ok());
        assert_eq!(nnrp_connection_close(server), NnrpFfiStatus::ok());
    }
}

#[cfg(feature = "transport-websocket")]
#[test]
fn websocket_role_runtime_adopts_carriers_and_completes_real_handshake() {
    unsafe {
        assert_role_handshake(TransportId::WebSocket, "ws://127.0.0.1:0/nnrp", 710_000);
    }
}

#[cfg(all(feature = "transport-ipc", windows))]
#[test]
fn named_pipe_role_runtime_adopts_carriers_and_completes_real_handshake() {
    let endpoint = format!("npipe://nnrp-role-{}", std::process::id());
    unsafe {
        assert_role_handshake(TransportId::Ipc, &endpoint, 720_000);
    }
}

#[cfg(all(feature = "transport-ipc", unix))]
#[test]
fn unix_ipc_role_runtime_adopts_carriers_and_completes_real_handshake() {
    let path = std::env::temp_dir().join(format!("nnrp-role-{}.sock", std::process::id()));
    let endpoint = format!("unix://{}", path.display());
    unsafe {
        assert_role_handshake(TransportId::Ipc, &endpoint, 720_000);
    }
    let _ = std::fs::remove_file(path);
}
