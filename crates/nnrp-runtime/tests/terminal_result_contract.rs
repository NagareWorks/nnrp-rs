use nnrp_core::{CommonHeader, MessageType, OperationState, ResultTerminalState};
use nnrp_runtime::{
    NnrpResult, NnrpRuntimeEvent, NnrpRuntimeEventMetadata, NnrpRuntimeEventTail,
    NnrpTerminalEvent, OperationLifecycleEvent, RuntimeError, RuntimeFrameHeader,
};

#[test]
fn public_terminal_result_union_preserves_runtime_and_lifecycle_evidence() {
    let mut header = CommonHeader::new(MessageType::ResultDrop, 0, 0);
    header.session_id = 7;
    header.frame_id = 11;
    header.trace_id = 13;
    let runtime_event = NnrpRuntimeEvent {
        header: RuntimeFrameHeader::from(header),
        metadata: NnrpRuntimeEventMetadata::None,
        tail: NnrpRuntimeEventTail::None,
    };
    let runtime_result = NnrpResult::from_runtime(17, runtime_event.clone()).unwrap();
    assert_eq!(runtime_result.operation_id, 17);
    assert_eq!(runtime_result.terminal_state, ResultTerminalState::Dropped);
    assert_eq!(
        runtime_result.event,
        NnrpTerminalEvent::Runtime(runtime_event)
    );

    let lifecycle_event = OperationLifecycleEvent::new(19, OperationState::Cancelled).unwrap();
    let lifecycle_result = NnrpResult::from_lifecycle(lifecycle_event).unwrap();
    assert_eq!(lifecycle_result.operation_id, 19);
    assert_eq!(
        lifecycle_result.terminal_state,
        ResultTerminalState::Cancelled
    );
    assert_eq!(
        lifecycle_result.event,
        NnrpTerminalEvent::Lifecycle(lifecycle_event)
    );
}

#[test]
fn public_terminal_result_union_rejects_invalid_evidence() {
    assert!(OperationLifecycleEvent::new(0, OperationState::Failed).is_err());
    assert!(matches!(
        NnrpResult::from_lifecycle(
            OperationLifecycleEvent::new(23, OperationState::Running).unwrap()
        ),
        Err(RuntimeError::UnexpectedMessage(_))
    ));

    let header = RuntimeFrameHeader::from(CommonHeader::new(MessageType::Progress, 0, 0));
    assert!(matches!(
        NnrpResult::from_runtime(
            29,
            NnrpRuntimeEvent {
                header,
                metadata: NnrpRuntimeEventMetadata::None,
                tail: NnrpRuntimeEventTail::None,
            },
        ),
        Err(RuntimeError::UnexpectedMessage(_))
    ));
}
