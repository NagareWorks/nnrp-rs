use nnrp_core::{
    BackpressureLevel, CancelScope, CommonHeader, ConnectionLifecycle, FlowScopeKind,
    FlowUpdateMetadata, FlowUpdateReason, InFlightPolicy, MessageType, OperationCancelRequest,
    OperationDescriptor, OperationRegistry, OperationState, SessionCloseAckMetadata,
    SessionCloseMetadata, SessionCloseReason, SessionCloseStatus, SessionLifecycleState,
    SessionOpenAckMetadata, SessionPriorityClass, SessionStatus, FLOW_UPDATE_FLAG_CREDIT_VALID,
};

#[test]
fn one_connection_orchestrates_independent_sessions_and_operation_scopes() {
    let mut connection = ConnectionLifecycle::new();
    connection.apply_session_open_ack(&open_ack(42)).unwrap();
    connection.apply_session_open_ack(&open_ack(43)).unwrap();

    let mut operations = OperationRegistry::new();
    operations
        .register(OperationDescriptor::new(42, 4201))
        .unwrap();
    operations
        .register(OperationDescriptor::new(43, 4301))
        .unwrap();
    operations
        .transition(4201, OperationState::Running)
        .unwrap();
    operations
        .transition(4301, OperationState::Running)
        .unwrap();

    let mut session_flow = CommonHeader::new(MessageType::FlowUpdate, 32, 0);
    session_flow.session_id = 42;
    connection
        .validate_flow_update(&session_flow, &flow_update(FlowScopeKind::Session, 0))
        .unwrap();

    let mut operation_flow = CommonHeader::new(MessageType::FlowUpdate, 32, 0);
    operation_flow.session_id = 43;
    connection
        .validate_flow_update(
            &operation_flow,
            &flow_update(FlowScopeKind::Operation, 4301),
        )
        .unwrap();

    let mut close_header = CommonHeader::new(MessageType::SessionClose, 24, 0);
    close_header.session_id = 42;
    connection
        .begin_session_close(&close_header, &close_metadata(4201))
        .unwrap();

    assert_eq!(
        connection.session(42).unwrap().state,
        SessionLifecycleState::Closing
    );
    assert_eq!(
        connection.session(43).unwrap().state,
        SessionLifecycleState::Open
    );
    assert_eq!(
        operations.cancel(OperationCancelRequest {
            session_id: 42,
            operation_id: 4201,
            cancel_scope: CancelScope::Session,
        }),
        Ok(vec![4201])
    );
    assert_eq!(
        operations.operation(4301).unwrap().state,
        OperationState::Running
    );

    let mut close_ack_header = CommonHeader::new(MessageType::SessionCloseAck, 16, 0);
    close_ack_header.session_id = 42;
    connection
        .apply_session_close_ack(
            &close_ack_header,
            &SessionCloseAckMetadata {
                close_status: SessionCloseStatus::Closed,
                last_operation_id: 4201,
                session_error_code: 0,
            },
        )
        .unwrap();

    assert_eq!(
        connection.session(42).unwrap().state,
        SessionLifecycleState::Closed
    );
    connection
        .validate_flow_update(
            &operation_flow,
            &flow_update(FlowScopeKind::Operation, 4301),
        )
        .unwrap();
}

fn open_ack(session_id: u32) -> SessionOpenAckMetadata {
    SessionOpenAckMetadata {
        session_id,
        accepted_profile_id: 2,
        accepted_priority_class: SessionPriorityClass::Balanced,
        session_status: SessionStatus::Opened,
        schema_id: 0x1001,
        schema_version: 3,
        granted_operation_credit: 2,
        max_in_flight_operations: 4,
        lease_ttl_ms: 30_000,
        resume_window_ms: 120_000,
        resume_token_bytes: 16,
        session_extension_bytes: 0,
        server_session_tag: session_id as u64,
        route_scope_id: 7,
        session_error_code: 0,
        session_flags_ack: 1,
    }
}

fn close_metadata(last_operation_id: u64) -> SessionCloseMetadata {
    SessionCloseMetadata {
        close_reason: SessionCloseReason::ClientShutdown,
        in_flight_policy: InFlightPolicy::Drain,
        drain_timeout_ms: 1000,
        last_operation_id,
        session_error_code: 0,
        session_close_tag: 0x1122_3344,
    }
}

fn flow_update(scope_kind: FlowScopeKind, operation_id: u64) -> FlowUpdateMetadata {
    FlowUpdateMetadata {
        scope_kind,
        update_reason: FlowUpdateReason::Grant,
        backpressure_level: BackpressureLevel::None,
        connection_credit: 0,
        session_credit: u16::from(scope_kind == FlowScopeKind::Session),
        operation_credit: u16::from(scope_kind == FlowScopeKind::Operation),
        operation_id,
        retry_after_ms: 0,
        credit_epoch: 1,
        flow_flags: FLOW_UPDATE_FLAG_CREDIT_VALID,
    }
}
