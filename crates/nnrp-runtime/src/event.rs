use nnrp_core::{
    BudgetMetadata, CacheInvalidateMetadata, CacheMissMetadata, CacheReferenceMetadata,
    CapabilityMetadata, ControlRequestMetadata, FlowUpdateMetadata, FrameSubmitMetadata, NnrpError,
    ObjectDeltaMetadata, ObjectDescriptorMetadata, ObjectReferenceMetadata, ObjectReleaseMetadata,
    PartialResultMetadata, PressureMetadata, ProgressMetadata, RecoverableErrorMetadata,
    ResultDropReasonMetadata, ResultHintMetadata, ResultPushMetadata, RetryAfterMetadata,
    RouteHintMetadata, SchedulingMetadata, SessionCloseMetadata, SupersedeMetadata,
    TraceContextMetadata,
};

use crate::{client::NnrpClientEvent, server::NnrpServerEvent, RuntimeFrameHeader};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NnrpRuntimeEventMetadata {
    None,
    FrameSubmit(FrameSubmitMetadata),
    ResultPush(ResultPushMetadata),
    ResultHint(ResultHintMetadata),
    ControlRequest(ControlRequestMetadata),
    Scheduling(SchedulingMetadata),
    Supersede(SupersedeMetadata),
    Budget(BudgetMetadata),
    Progress(ProgressMetadata),
    PartialResult(PartialResultMetadata),
    Pressure(PressureMetadata),
    Capability(CapabilityMetadata),
    RouteHint(RouteHintMetadata),
    TraceContext(TraceContextMetadata),
    ResultDropReason(ResultDropReasonMetadata),
    RecoverableError(RecoverableErrorMetadata),
    RetryAfter(RetryAfterMetadata),
    FlowUpdate(FlowUpdateMetadata),
    ObjectDescriptor(ObjectDescriptorMetadata),
    ObjectReference(ObjectReferenceMetadata),
    ObjectRelease(ObjectReleaseMetadata),
    ObjectDelta(ObjectDeltaMetadata),
    CacheReference(CacheReferenceMetadata),
    CacheMiss(CacheMissMetadata),
    CacheInvalidate(CacheInvalidateMetadata),
    SessionClose(SessionCloseMetadata),
}

impl NnrpRuntimeEventMetadata {
    fn to_bytes(&self) -> Result<Vec<u8>, NnrpError> {
        Ok(match self {
            Self::None => Vec::new(),
            Self::FrameSubmit(value) => value.to_bytes()?.to_vec(),
            Self::ResultPush(value) => value.to_bytes()?.to_vec(),
            Self::ResultHint(value) => value.to_bytes()?.to_vec(),
            Self::ControlRequest(value) => value.to_bytes()?.to_vec(),
            Self::Scheduling(value) => value.to_bytes()?.to_vec(),
            Self::Supersede(value) => value.to_bytes()?.to_vec(),
            Self::Budget(value) => value.to_bytes()?.to_vec(),
            Self::Progress(value) => value.to_bytes()?.to_vec(),
            Self::PartialResult(value) => value.to_bytes()?.to_vec(),
            Self::Pressure(value) => value.to_bytes()?.to_vec(),
            Self::Capability(value) => value.to_bytes()?.to_vec(),
            Self::RouteHint(value) => value.to_bytes()?.to_vec(),
            Self::TraceContext(value) => value.to_bytes()?.to_vec(),
            Self::ResultDropReason(value) => value.to_bytes()?.to_vec(),
            Self::RecoverableError(value) => value.to_bytes()?.to_vec(),
            Self::RetryAfter(value) => value.to_bytes()?.to_vec(),
            Self::FlowUpdate(value) => value.to_bytes()?.to_vec(),
            Self::ObjectDescriptor(value) => value.to_bytes()?.to_vec(),
            Self::ObjectReference(value) => value.to_bytes()?.to_vec(),
            Self::ObjectRelease(value) => value.to_bytes()?.to_vec(),
            Self::ObjectDelta(value) => value.to_bytes()?.to_vec(),
            Self::CacheReference(value) => value.to_bytes()?.to_vec(),
            Self::CacheMiss(value) => value.to_bytes()?.to_vec(),
            Self::CacheInvalidate(value) => value.to_bytes()?.to_vec(),
            Self::SessionClose(value) => value.to_bytes()?.to_vec(),
        })
    }

    pub fn operation_id(&self) -> Option<u64> {
        match self {
            Self::FrameSubmit(value) => Some(value.operation_id),
            Self::ControlRequest(value) => Some(value.operation_id),
            Self::Scheduling(value) => Some(value.operation_id),
            Self::Supersede(value) => Some(value.old_operation_id),
            Self::Budget(value) => Some(value.operation_id),
            Self::Progress(value) => Some(value.operation_id),
            Self::PartialResult(value) => Some(value.operation_id),
            Self::RouteHint(value) if value.operation_id != 0 => Some(value.operation_id),
            Self::ResultDropReason(value) => Some(value.operation_id),
            Self::ObjectReference(value) => Some(value.operation_id),
            Self::ObjectRelease(value) => Some(value.operation_id),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NnrpRuntimeEventTail {
    None,
    Body(Vec<u8>),
    Diagnostic(Vec<u8>),
    MetadataBodyAndDelta {
        metadata_body: Vec<u8>,
        delta: Vec<u8>,
    },
}

impl NnrpRuntimeEventTail {
    fn append_to(self, payload: &mut Vec<u8>) {
        match self {
            Self::None => {}
            Self::Body(body) | Self::Diagnostic(body) => payload.extend_from_slice(&body),
            Self::MetadataBodyAndDelta {
                metadata_body,
                delta,
            } => {
                payload.extend_from_slice(&metadata_body);
                payload.extend_from_slice(&delta);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpRuntimeEvent {
    pub header: RuntimeFrameHeader,
    pub metadata: NnrpRuntimeEventMetadata,
    pub tail: NnrpRuntimeEventTail,
}

impl NnrpRuntimeEvent {
    pub fn into_payload(self) -> Result<Vec<u8>, NnrpError> {
        let mut payload = self.metadata.to_bytes()?;
        self.tail.append_to(&mut payload);
        Ok(payload)
    }

    pub(crate) fn from_client(header: RuntimeFrameHeader, event: NnrpClientEvent) -> Self {
        if let NnrpClientEvent::Result(result) = event {
            debug_assert_eq!(header, result.event.header);
            return result.event;
        }
        let (metadata, tail) = match event {
            NnrpClientEvent::Result(_) => unreachable!("terminal result handled above"),
            NnrpClientEvent::PartialResult { metadata, body } => (
                NnrpRuntimeEventMetadata::PartialResult(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpClientEvent::Progress { metadata, body } => (
                NnrpRuntimeEventMetadata::Progress(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpClientEvent::Control { metadata, body, .. } => (
                NnrpRuntimeEventMetadata::ControlRequest(metadata),
                NnrpRuntimeEventTail::Diagnostic(body),
            ),
            NnrpClientEvent::Scheduling { metadata, .. } => (
                NnrpRuntimeEventMetadata::Scheduling(metadata),
                NnrpRuntimeEventTail::None,
            ),
            NnrpClientEvent::Supersede { metadata, body } => (
                NnrpRuntimeEventMetadata::Supersede(metadata),
                NnrpRuntimeEventTail::Diagnostic(body),
            ),
            NnrpClientEvent::Budget(metadata) => (
                NnrpRuntimeEventMetadata::Budget(metadata),
                NnrpRuntimeEventTail::None,
            ),
            NnrpClientEvent::FlowUpdate(metadata) => (
                NnrpRuntimeEventMetadata::FlowUpdate(metadata),
                NnrpRuntimeEventTail::None,
            ),
            NnrpClientEvent::Backpressure(metadata) | NnrpClientEvent::CreditUpdate(metadata) => (
                NnrpRuntimeEventMetadata::Pressure(metadata),
                NnrpRuntimeEventTail::None,
            ),
            NnrpClientEvent::ObjectDeclare { metadata, body } => (
                NnrpRuntimeEventMetadata::ObjectDescriptor(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpClientEvent::ObjectRef { metadata, body } => (
                NnrpRuntimeEventMetadata::ObjectReference(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpClientEvent::ObjectRelease { metadata, body } => (
                NnrpRuntimeEventMetadata::ObjectRelease(metadata),
                NnrpRuntimeEventTail::Diagnostic(body),
            ),
            NnrpClientEvent::ObjectDelta {
                metadata, mut body, ..
            } => {
                let delta = body.split_off(metadata.metadata_bytes as usize);
                (
                    NnrpRuntimeEventMetadata::ObjectDelta(metadata),
                    NnrpRuntimeEventTail::MetadataBodyAndDelta {
                        metadata_body: body,
                        delta,
                    },
                )
            }
            NnrpClientEvent::CacheReference { metadata, body } => (
                NnrpRuntimeEventMetadata::CacheReference(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpClientEvent::CacheMiss { metadata, body } => (
                NnrpRuntimeEventMetadata::CacheMiss(metadata),
                NnrpRuntimeEventTail::Diagnostic(body),
            ),
            NnrpClientEvent::CacheInvalidate(metadata) => (
                NnrpRuntimeEventMetadata::CacheInvalidate(metadata),
                NnrpRuntimeEventTail::None,
            ),
            NnrpClientEvent::Capability { metadata, body, .. } => (
                NnrpRuntimeEventMetadata::Capability(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpClientEvent::RouteHint { metadata, body, .. } => (
                NnrpRuntimeEventMetadata::RouteHint(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpClientEvent::TraceContext { metadata, body, .. } => (
                NnrpRuntimeEventMetadata::TraceContext(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpClientEvent::RecoverableError { metadata, body } => (
                NnrpRuntimeEventMetadata::RecoverableError(metadata),
                NnrpRuntimeEventTail::Diagnostic(body),
            ),
            NnrpClientEvent::RetryAfter { metadata, body } => (
                NnrpRuntimeEventMetadata::RetryAfter(metadata),
                NnrpRuntimeEventTail::Diagnostic(body),
            ),
            NnrpClientEvent::ResultHint(metadata) => (
                NnrpRuntimeEventMetadata::ResultHint(metadata),
                NnrpRuntimeEventTail::None,
            ),
        };
        Self {
            header,
            metadata,
            tail,
        }
    }

    pub(crate) fn from_server(header: RuntimeFrameHeader, event: NnrpServerEvent) -> Self {
        let (metadata, tail) = match event {
            NnrpServerEvent::Submit(submit) => (
                NnrpRuntimeEventMetadata::FrameSubmit(submit.metadata),
                NnrpRuntimeEventTail::Body(submit.body),
            ),
            NnrpServerEvent::FrameCancel(_) => {
                (NnrpRuntimeEventMetadata::None, NnrpRuntimeEventTail::None)
            }
            NnrpServerEvent::PartialResult { metadata, body } => (
                NnrpRuntimeEventMetadata::PartialResult(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpServerEvent::Progress { metadata, body } => (
                NnrpRuntimeEventMetadata::Progress(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpServerEvent::ResultDropReason { metadata, body } => (
                NnrpRuntimeEventMetadata::ResultDropReason(metadata),
                NnrpRuntimeEventTail::Diagnostic(body),
            ),
            NnrpServerEvent::Control(control) => (
                NnrpRuntimeEventMetadata::ControlRequest(control.metadata),
                NnrpRuntimeEventTail::Diagnostic(control.body),
            ),
            NnrpServerEvent::Scheduling(update) => (
                NnrpRuntimeEventMetadata::Scheduling(update.metadata),
                NnrpRuntimeEventTail::None,
            ),
            NnrpServerEvent::Supersede { metadata, body } => (
                NnrpRuntimeEventMetadata::Supersede(metadata),
                NnrpRuntimeEventTail::Diagnostic(body),
            ),
            NnrpServerEvent::Budget(metadata) => (
                NnrpRuntimeEventMetadata::Budget(metadata),
                NnrpRuntimeEventTail::None,
            ),
            NnrpServerEvent::FlowUpdate(metadata) => (
                NnrpRuntimeEventMetadata::FlowUpdate(metadata),
                NnrpRuntimeEventTail::None,
            ),
            NnrpServerEvent::Pressure(update) => (
                NnrpRuntimeEventMetadata::Pressure(update.metadata),
                NnrpRuntimeEventTail::None,
            ),
            NnrpServerEvent::Capability { metadata, body, .. } => (
                NnrpRuntimeEventMetadata::Capability(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpServerEvent::RouteHint { metadata, body, .. } => (
                NnrpRuntimeEventMetadata::RouteHint(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpServerEvent::TraceContext { metadata, body, .. } => (
                NnrpRuntimeEventMetadata::TraceContext(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpServerEvent::RecoverableError { metadata, body } => (
                NnrpRuntimeEventMetadata::RecoverableError(metadata),
                NnrpRuntimeEventTail::Diagnostic(body),
            ),
            NnrpServerEvent::RetryAfter { metadata, body } => (
                NnrpRuntimeEventMetadata::RetryAfter(metadata),
                NnrpRuntimeEventTail::Diagnostic(body),
            ),
            NnrpServerEvent::ObjectDeclare { metadata, body } => (
                NnrpRuntimeEventMetadata::ObjectDescriptor(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpServerEvent::ObjectRef { metadata, body } => (
                NnrpRuntimeEventMetadata::ObjectReference(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpServerEvent::ObjectRelease { metadata, body } => (
                NnrpRuntimeEventMetadata::ObjectRelease(metadata),
                NnrpRuntimeEventTail::Diagnostic(body),
            ),
            NnrpServerEvent::ObjectDelta {
                metadata, mut body, ..
            } => {
                let delta = body.split_off(metadata.metadata_bytes as usize);
                (
                    NnrpRuntimeEventMetadata::ObjectDelta(metadata),
                    NnrpRuntimeEventTail::MetadataBodyAndDelta {
                        metadata_body: body,
                        delta,
                    },
                )
            }
            NnrpServerEvent::CacheReference { metadata, body } => (
                NnrpRuntimeEventMetadata::CacheReference(metadata),
                NnrpRuntimeEventTail::Body(body),
            ),
            NnrpServerEvent::CacheMiss { metadata, body } => (
                NnrpRuntimeEventMetadata::CacheMiss(metadata),
                NnrpRuntimeEventTail::Diagnostic(body),
            ),
            NnrpServerEvent::CacheInvalidate(metadata) => (
                NnrpRuntimeEventMetadata::CacheInvalidate(metadata),
                NnrpRuntimeEventTail::None,
            ),
            NnrpServerEvent::Close(metadata) => (
                NnrpRuntimeEventMetadata::SessionClose(metadata),
                NnrpRuntimeEventTail::None,
            ),
        };
        Self {
            header,
            metadata,
            tail,
        }
    }
}
