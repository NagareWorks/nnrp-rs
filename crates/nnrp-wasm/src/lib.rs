use nnrp_core::{
    BudgetMetadata, CacheMissMetadata, CacheMissReason, CacheReferenceMetadata, CacheReuseScope,
    CapabilityMetadata, CommonHeader, ControlRequestMetadata, HeaderFlags, MemoryLocationHint,
    MessageType, NnrpError, ObjectDeltaMetadata, ObjectDescriptorMetadata, ObjectReferenceMetadata,
    ObjectReleaseMetadata, ObjectReleaseReason, OwnershipHint, PartialResultMetadata,
    PressureMetadata, ProgressMetadata, ProtocolVersion, RecoverableErrorMetadata,
    ResultDropReasonMetadata, RetryAfterMetadata, RouteHintMetadata, RuntimeObjectKind,
    RuntimeRole, SchedulingMetadata, SupersedeMetadata, TraceContextMetadata, TransportId,
    COMMON_HEADER_LEN,
};
use nnrp_transport_provider::{
    select_transport_with_probe, summarize_provider_probe, ProbeMetrics, ProbeSample, ProbeState,
    ProviderCost, ProviderLimitation, ProviderLimits, RemoteTransportSupport,
    TransportCandidateDiagnostic, TransportPolicy, TransportProviderDescriptor,
    TransportProviderKind, TransportProviderMetadata, TransportRejectionReason,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
mod browser_role;
#[cfg(target_arch = "wasm32")]
pub use browser_role::{open_browser_client_role, BrowserClientEventPacket, BrowserClientRole};

#[cfg(not(any(
    feature = "transport-tcp",
    feature = "transport-quic",
    feature = "transport-ipc",
    feature = "transport-websocket"
)))]
compile_error!("nnrp-wasm must be built with at least one transport feature enabled.");

#[wasm_bindgen]
pub fn nnrp_wasm_protocol_major() -> u8 {
    ProtocolVersion::CURRENT.major
}

#[wasm_bindgen]
pub fn nnrp_wasm_wire_format() -> u8 {
    ProtocolVersion::CURRENT.wire_format
}

#[wasm_bindgen(js_name = selectTransportWithProbeJson)]
pub fn select_transport_with_probe_json(
    providers_json: &str,
    remote_transports_json: &str,
    policy: &str,
    requested_max_frame_bytes: Option<String>,
    samples_json: &str,
) -> Result<String, JsValue> {
    let providers = parse_providers(providers_json)?;
    let remote = RemoteTransportSupport::new(parse_transport_ids(remote_transports_json)?);
    let policy = parse_policy(policy)?;
    let requested_max_frame_bytes = requested_max_frame_bytes
        .as_deref()
        .map(parse_canonical_u64)
        .transpose()?;
    let samples = parse_probe_samples(samples_json)?;
    let selection = select_transport_with_probe(
        &providers,
        &remote,
        policy,
        requested_max_frame_bytes,
        &samples,
    )
    .map_err(|error| js_error(&error.to_string()))?;

    serde_json::to_string(&WasmTransportSelection::from(selection))
        .map_err(|error| js_error(&error.to_string()))
}

#[wasm_bindgen(js_name = summarizeProviderProbeJson)]
pub fn summarize_provider_probe_json(
    provider_json: &str,
    samples_json: &str,
) -> Result<String, JsValue> {
    let provider = parse_provider(provider_json)?;
    let samples = parse_probe_samples(samples_json)?;
    let metrics = summarize_provider_probe(&provider, &samples)
        .ok_or_else(|| js_error("no matching probe samples for provider"))?;

    serde_json::to_string(&WasmProbeMetrics::from(metrics))
        .map_err(|error| js_error(&error.to_string()))
}

#[wasm_bindgen(js_name = encodeWebSocketBinaryFrameJson)]
pub fn encode_websocket_binary_frame_json(
    header_json: &str,
    metadata: &[u8],
    body: &[u8],
) -> Result<Vec<u8>, JsValue> {
    let input = serde_json::from_str::<WasmFrameHeaderInput>(header_json)
        .map_err(|error| js_error(&error.to_string()))?;
    let header = header_from_input(input, metadata.len(), body.len())?;
    let mut frame = Vec::with_capacity(COMMON_HEADER_LEN + metadata.len() + body.len());
    frame.extend_from_slice(&header.to_bytes().map_err(js_nnrp_error)?);
    frame.extend_from_slice(metadata);
    frame.extend_from_slice(body);
    Ok(frame)
}

#[wasm_bindgen(js_name = decodeWebSocketBinaryFrameJson)]
pub fn decode_websocket_binary_frame_json(frame: &[u8]) -> Result<String, JsValue> {
    let (header, metadata, body) = CommonHeader::parse_packet(frame).map_err(js_nnrp_error)?;
    let output = WasmFrameOutput {
        header: WasmFrameHeaderOutput::from(header),
        metadata_offset: COMMON_HEADER_LEN,
        metadata_len: metadata.len(),
        body_offset: COMMON_HEADER_LEN + metadata.len(),
        body_len: body.len(),
    };
    serde_json::to_string(&output).map_err(|error| js_error(&error.to_string()))
}

#[wasm_bindgen(js_name = decodeWebSocketBinaryFrameBatchJson)]
pub fn decode_websocket_binary_frame_batch_json(
    frames: &[u8],
    max_frames: u32,
) -> Result<String, JsValue> {
    let mut cursor = 0;
    let mut outputs = Vec::new();

    while cursor < frames.len() && (max_frames == 0 || outputs.len() < max_frames as usize) {
        let source = &frames[cursor..];
        let header = CommonHeader::parse(source).map_err(js_nnrp_error)?;
        let frame_len = header.packet_len().map_err(js_nnrp_error)?;
        if source.len() < frame_len {
            return Err(js_error("incomplete binary frame in batch"));
        }

        let metadata_offset = cursor + COMMON_HEADER_LEN;
        let metadata_len = header.meta_len as usize;
        let body_offset = metadata_offset + metadata_len;
        let body_len = header.body_len as usize;
        outputs.push(WasmFrameBatchEntry {
            frame_offset: cursor,
            frame_len,
            header: WasmFrameHeaderOutput::from(header),
            metadata_offset,
            metadata_len,
            body_offset,
            body_len,
        });
        cursor += frame_len;
    }

    serde_json::to_string(&WasmFrameBatchOutput {
        frames: outputs,
        consumed_len: cursor,
        remaining_len: frames.len() - cursor,
    })
    .map_err(|error| js_error(&error.to_string()))
}

#[wasm_bindgen(js_name = encodeRuntimeControlMetadataJson)]
pub fn encode_runtime_control_metadata_json(
    message_type: u8,
    metadata_json: &str,
    tail: &[u8],
) -> Result<Vec<u8>, JsValue> {
    let message_type = MessageType::try_from_u8(message_type).map_err(js_nnrp_error)?;
    match message_type {
        MessageType::Cancel | MessageType::Abort => {
            serde_json::from_str::<WasmControlRequest>(metadata_json)
                .map_err(|error| js_error(&error.to_string()))?
                .into_core()
                .to_vec_with_diagnostics(tail)
                .map_err(js_nnrp_error)
        }
        MessageType::PriorityUpdate | MessageType::Deadline | MessageType::ExpireAt => {
            reject_tail_for_fixed_metadata(tail)?;
            serde_json::from_str::<WasmScheduling>(metadata_json)
                .map_err(|error| js_error(&error.to_string()))?
                .into_core()
                .to_bytes()
                .map(|bytes| bytes.to_vec())
                .map_err(js_nnrp_error)
        }
        MessageType::Supersede => serde_json::from_str::<WasmSupersede>(metadata_json)
            .map_err(|error| js_error(&error.to_string()))?
            .into_core()
            .to_vec_with_diagnostics(tail)
            .map_err(js_nnrp_error),
        MessageType::BudgetUpdate => {
            reject_tail_for_fixed_metadata(tail)?;
            serde_json::from_str::<WasmBudget>(metadata_json)
                .map_err(|error| js_error(&error.to_string()))?
                .into_core()
                .to_bytes()
                .map(|bytes| bytes.to_vec())
                .map_err(js_nnrp_error)
        }
        MessageType::Progress => serde_json::from_str::<WasmProgress>(metadata_json)
            .map_err(|error| js_error(&error.to_string()))?
            .into_core()
            .to_vec_with_body(tail)
            .map_err(js_nnrp_error),
        MessageType::PartialResult => serde_json::from_str::<WasmPartialResult>(metadata_json)
            .map_err(|error| js_error(&error.to_string()))?
            .into_core()
            .to_vec_with_body(tail)
            .map_err(js_nnrp_error),
        MessageType::Backpressure | MessageType::CreditUpdate => {
            reject_tail_for_fixed_metadata(tail)?;
            serde_json::from_str::<WasmPressure>(metadata_json)
                .map_err(|error| js_error(&error.to_string()))?
                .into_core()
                .to_bytes()
                .map(|bytes| bytes.to_vec())
                .map_err(js_nnrp_error)
        }
        MessageType::CapabilityNegotiation | MessageType::DegradeProfile => {
            serde_json::from_str::<WasmCapability>(metadata_json)
                .map_err(|error| js_error(&error.to_string()))?
                .into_core()
                .to_vec_with_body(tail)
                .map_err(js_nnrp_error)
        }
        MessageType::RouteHint | MessageType::ExecutionHint => {
            serde_json::from_str::<WasmRouteHint>(metadata_json)
                .map_err(|error| js_error(&error.to_string()))?
                .into_core()
                .to_vec_with_body(tail)
                .map_err(js_nnrp_error)
        }
        MessageType::TraceContext => serde_json::from_str::<WasmTraceContext>(metadata_json)
            .map_err(|error| js_error(&error.to_string()))?
            .into_core()
            .to_vec_with_body(tail)
            .map_err(js_nnrp_error),
        MessageType::ResultDropReason => {
            serde_json::from_str::<WasmResultDropReason>(metadata_json)
                .map_err(|error| js_error(&error.to_string()))?
                .into_core()
                .to_vec_with_diagnostics(tail)
                .map_err(js_nnrp_error)
        }
        MessageType::ErrorRecoverable => {
            serde_json::from_str::<WasmRecoverableError>(metadata_json)
                .map_err(|error| js_error(&error.to_string()))?
                .into_core()?
                .to_vec_with_diagnostics(tail)
                .map_err(js_nnrp_error)
        }
        MessageType::RetryAfter => serde_json::from_str::<WasmRetryAfter>(metadata_json)
            .map_err(|error| js_error(&error.to_string()))?
            .into_core()
            .to_vec_with_diagnostics(tail)
            .map_err(js_nnrp_error),
        other => Err(js_error(&format!(
            "message type {other:?} does not carry runtime control metadata"
        ))),
    }
}

#[wasm_bindgen(js_name = decodeRuntimeControlMetadataJson)]
pub fn decode_runtime_control_metadata_json(
    message_type: u8,
    metadata: &[u8],
) -> Result<String, JsValue> {
    let message_type = MessageType::try_from_u8(message_type).map_err(js_nnrp_error)?;
    match message_type {
        MessageType::Cancel | MessageType::Abort => {
            let (value, tail) =
                ControlRequestMetadata::parse_with_diagnostics(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmControlRequest::from_core(value), metadata, tail)
        }
        MessageType::PriorityUpdate | MessageType::Deadline | MessageType::ExpireAt => {
            decoded_fixed_metadata_json(WasmScheduling::from_core(
                SchedulingMetadata::parse(metadata).map_err(js_nnrp_error)?,
            ))
        }
        MessageType::Supersede => {
            let (value, tail) =
                SupersedeMetadata::parse_with_diagnostics(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmSupersede::from_core(value), metadata, tail)
        }
        MessageType::BudgetUpdate => decoded_fixed_metadata_json(WasmBudget::from_core(
            BudgetMetadata::parse(metadata).map_err(js_nnrp_error)?,
        )),
        MessageType::Progress => {
            let (value, tail) =
                ProgressMetadata::parse_with_body(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmProgress::from_core(value), metadata, tail)
        }
        MessageType::PartialResult => {
            let (value, tail) =
                PartialResultMetadata::parse_with_body(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmPartialResult::from_core(value), metadata, tail)
        }
        MessageType::Backpressure | MessageType::CreditUpdate => decoded_fixed_metadata_json(
            WasmPressure::from_core(PressureMetadata::parse(metadata).map_err(js_nnrp_error)?),
        ),
        MessageType::CapabilityNegotiation | MessageType::DegradeProfile => {
            let (value, tail) =
                CapabilityMetadata::parse_with_body(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmCapability::from_core(value), metadata, tail)
        }
        MessageType::RouteHint | MessageType::ExecutionHint => {
            let (value, tail) =
                RouteHintMetadata::parse_with_body(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmRouteHint::from_core(value), metadata, tail)
        }
        MessageType::TraceContext => {
            let (value, tail) =
                TraceContextMetadata::parse_with_body(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmTraceContext::from_core(value), metadata, tail)
        }
        MessageType::ResultDropReason => {
            let (value, tail) = ResultDropReasonMetadata::parse_with_diagnostics(metadata)
                .map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmResultDropReason::from_core(value), metadata, tail)
        }
        MessageType::ErrorRecoverable => {
            let (value, tail) = RecoverableErrorMetadata::parse_with_diagnostics(metadata)
                .map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmRecoverableError::from_core(value), metadata, tail)
        }
        MessageType::RetryAfter => {
            let (value, tail) =
                RetryAfterMetadata::parse_with_diagnostics(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmRetryAfter::from_core(value), metadata, tail)
        }
        other => Err(js_error(&format!(
            "message type {other:?} does not carry runtime control metadata"
        ))),
    }
}

#[wasm_bindgen(js_name = encodeRuntimeObjectMetadataJson)]
pub fn encode_runtime_object_metadata_json(
    message_type: u8,
    metadata_json: &str,
    tail: &[u8],
) -> Result<Vec<u8>, JsValue> {
    let message_type = MessageType::try_from_u8(message_type).map_err(js_nnrp_error)?;
    match message_type {
        MessageType::ObjectDeclare => serde_json::from_str::<WasmObjectDescriptor>(metadata_json)
            .map_err(|error| js_error(&error.to_string()))?
            .into_core()?
            .to_vec_with_extension(tail)
            .map_err(js_nnrp_error),
        MessageType::ObjectRef => serde_json::from_str::<WasmObjectReference>(metadata_json)
            .map_err(|error| js_error(&error.to_string()))?
            .into_core()
            .to_vec_with_extension(tail)
            .map_err(js_nnrp_error),
        MessageType::ObjectRelease => serde_json::from_str::<WasmObjectRelease>(metadata_json)
            .map_err(|error| js_error(&error.to_string()))?
            .into_core()?
            .to_vec_with_diagnostics(tail)
            .map_err(js_nnrp_error),
        MessageType::ObjectPatch | MessageType::ObjectDelta => {
            serde_json::from_str::<WasmObjectDelta>(metadata_json)
                .map_err(|error| js_error(&error.to_string()))?
                .into_core()
                .to_vec_with_extension(tail)
                .map_err(js_nnrp_error)
        }
        MessageType::CacheReference => serde_json::from_str::<WasmCacheReference>(metadata_json)
            .map_err(|error| js_error(&error.to_string()))?
            .into_core()?
            .to_vec_with_extension(tail)
            .map_err(js_nnrp_error),
        MessageType::CacheMiss => serde_json::from_str::<WasmCacheMiss>(metadata_json)
            .map_err(|error| js_error(&error.to_string()))?
            .into_core()?
            .to_vec_with_diagnostics(tail)
            .map_err(js_nnrp_error),
        other => Err(js_error(&format!(
            "message type {other:?} does not carry runtime object metadata"
        ))),
    }
}

#[wasm_bindgen(js_name = decodeRuntimeObjectMetadataJson)]
pub fn decode_runtime_object_metadata_json(
    message_type: u8,
    metadata: &[u8],
) -> Result<String, JsValue> {
    let message_type = MessageType::try_from_u8(message_type).map_err(js_nnrp_error)?;
    match message_type {
        MessageType::ObjectDeclare => {
            let (value, tail) =
                ObjectDescriptorMetadata::parse_with_extension(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmObjectDescriptor::from_core(value), metadata, tail)
        }
        MessageType::ObjectRef => {
            let (value, tail) =
                ObjectReferenceMetadata::parse_with_extension(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmObjectReference::from_core(value), metadata, tail)
        }
        MessageType::ObjectRelease => {
            let (value, tail) =
                ObjectReleaseMetadata::parse_with_diagnostics(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmObjectRelease::from_core(value), metadata, tail)
        }
        MessageType::ObjectPatch | MessageType::ObjectDelta => {
            let (value, tail) =
                ObjectDeltaMetadata::parse_with_extension(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmObjectDelta::from_core(value), metadata, tail)
        }
        MessageType::CacheReference => {
            let (value, tail) =
                CacheReferenceMetadata::parse_with_extension(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmCacheReference::from_core(value), metadata, tail)
        }
        MessageType::CacheMiss => {
            let (value, tail) =
                CacheMissMetadata::parse_with_diagnostics(metadata).map_err(js_nnrp_error)?;
            decoded_metadata_with_tail_json(WasmCacheMiss::from_core(value), metadata, tail)
        }
        other => Err(js_error(&format!(
            "message type {other:?} does not carry runtime object metadata"
        ))),
    }
}

#[derive(Debug, Deserialize)]
struct WasmProviderInput {
    name: String,
    version: String,
    transport_id: u32,
    kind: Option<String>,
    available: Option<bool>,
    metadata: WasmProviderMetadata,
    diagnostic: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WasmProbeSampleInput {
    transport_id: u32,
    provider_id: String,
    elapsed_us: u64,
    rtt_us: Option<u64>,
    bytes_sent: u64,
    bytes_received: u64,
    timed_out: Option<bool>,
    failed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct WasmProviderCost {
    model_id: u16,
    units: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct WasmProviderLimits {
    max_frame_bytes: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct WasmProviderMetadata {
    id: String,
    cost: WasmProviderCost,
    preference_rank: u16,
    limits: WasmProviderLimits,
    limitations: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WasmFrameHeaderInput {
    message_type: u8,
    flags: Option<u32>,
    session_id: Option<u32>,
    frame_id: Option<u32>,
    view_id: Option<u16>,
    route_id: Option<u16>,
    trace_id: Option<u64>,
}

#[derive(Debug, Serialize)]
struct WasmFrameOutput {
    header: WasmFrameHeaderOutput,
    metadata_offset: usize,
    metadata_len: usize,
    body_offset: usize,
    body_len: usize,
}

#[derive(Debug, Serialize)]
struct WasmFrameBatchOutput {
    frames: Vec<WasmFrameBatchEntry>,
    consumed_len: usize,
    remaining_len: usize,
}

#[derive(Debug, Serialize)]
struct WasmFrameBatchEntry {
    frame_offset: usize,
    frame_len: usize,
    header: WasmFrameHeaderOutput,
    metadata_offset: usize,
    metadata_len: usize,
    body_offset: usize,
    body_len: usize,
}

#[derive(Debug, Serialize)]
struct WasmFrameHeaderOutput {
    version_major: u8,
    wire_format: u8,
    message_type: u8,
    header_len: u8,
    flags: u32,
    meta_len: u32,
    body_len: u32,
    session_id: u32,
    frame_id: u32,
    view_id: u16,
    route_id: u16,
    trace_id: u64,
}

impl From<CommonHeader> for WasmFrameHeaderOutput {
    fn from(value: CommonHeader) -> Self {
        Self {
            version_major: value.version_major,
            wire_format: value.wire_format,
            message_type: value.message_type as u8,
            header_len: value.header_len,
            flags: value.flags.0,
            meta_len: value.meta_len,
            body_len: value.body_len,
            session_id: value.session_id,
            frame_id: value.frame_id,
            view_id: value.view_id,
            route_id: value.route_id,
            trace_id: value.trace_id,
        }
    }
}

#[derive(Debug, Serialize)]
struct WasmDecodedMetadata<T: Serialize> {
    metadata: T,
    tail_offset: usize,
    tail_len: usize,
}

impl<T: Serialize> WasmDecodedMetadata<T> {
    fn fixed(metadata: T) -> Self {
        Self {
            metadata,
            tail_offset: 0,
            tail_len: 0,
        }
    }

    fn with_tail(metadata: T, source: &[u8], tail: &[u8]) -> Self {
        Self {
            metadata,
            tail_offset: source.len() - tail.len(),
            tail_len: tail.len(),
        }
    }
}

fn decoded_fixed_metadata_json<T: Serialize>(metadata: T) -> Result<String, JsValue> {
    serde_json::to_string(&WasmDecodedMetadata::fixed(metadata))
        .map_err(|error| js_error(&error.to_string()))
}

fn decoded_metadata_with_tail_json<T: Serialize>(
    metadata: T,
    source: &[u8],
    tail: &[u8],
) -> Result<String, JsValue> {
    serde_json::to_string(&WasmDecodedMetadata::with_tail(metadata, source, tail))
        .map_err(|error| js_error(&error.to_string()))
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmControlRequest {
    #[serde(with = "canonical_u64")]
    operation_id: u64,
    #[serde(with = "canonical_u64")]
    control_sequence: u64,
    reason_code: u16,
    source_role: u8,
    flags: u8,
    diagnostic_bytes: u32,
}

impl WasmControlRequest {
    fn into_core(self) -> ControlRequestMetadata {
        ControlRequestMetadata {
            operation_id: self.operation_id,
            control_sequence: self.control_sequence,
            reason_code: self.reason_code,
            source_role: self.source_role,
            flags: self.flags,
            diagnostic_bytes: self.diagnostic_bytes,
        }
    }

    fn from_core(value: ControlRequestMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            control_sequence: value.control_sequence,
            reason_code: value.reason_code,
            source_role: value.source_role,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmScheduling {
    #[serde(with = "canonical_u64")]
    operation_id: u64,
    #[serde(with = "canonical_u64")]
    control_sequence: u64,
    priority_class: u16,
    priority_delta: i16,
    #[serde(with = "canonical_u64")]
    deadline_unix_ms: u64,
    flags: u32,
}

impl WasmScheduling {
    fn into_core(self) -> SchedulingMetadata {
        SchedulingMetadata {
            operation_id: self.operation_id,
            control_sequence: self.control_sequence,
            priority_class: self.priority_class,
            priority_delta: self.priority_delta,
            deadline_unix_ms: self.deadline_unix_ms,
            flags: self.flags,
        }
    }

    fn from_core(value: SchedulingMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            control_sequence: value.control_sequence,
            priority_class: value.priority_class,
            priority_delta: value.priority_delta,
            deadline_unix_ms: value.deadline_unix_ms,
            flags: value.flags,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmSupersede {
    #[serde(with = "canonical_u64")]
    old_operation_id: u64,
    #[serde(with = "canonical_u64")]
    new_operation_id: u64,
    #[serde(with = "canonical_u64")]
    control_sequence: u64,
    drop_reason_code: u16,
    flags: u16,
    diagnostic_bytes: u32,
}

impl WasmSupersede {
    fn into_core(self) -> SupersedeMetadata {
        SupersedeMetadata {
            old_operation_id: self.old_operation_id,
            new_operation_id: self.new_operation_id,
            control_sequence: self.control_sequence,
            drop_reason_code: self.drop_reason_code,
            flags: self.flags,
            diagnostic_bytes: self.diagnostic_bytes,
        }
    }

    fn from_core(value: SupersedeMetadata) -> Self {
        Self {
            old_operation_id: value.old_operation_id,
            new_operation_id: value.new_operation_id,
            control_sequence: value.control_sequence,
            drop_reason_code: value.drop_reason_code,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmBudget {
    #[serde(with = "canonical_u64")]
    operation_id: u64,
    #[serde(with = "canonical_u64")]
    compute_budget_units: u64,
    #[serde(with = "canonical_u64")]
    memory_budget_bytes: u64,
    #[serde(with = "canonical_u64")]
    bandwidth_budget_bytes: u64,
    token_budget: u32,
    flags: u32,
}

impl WasmBudget {
    fn into_core(self) -> BudgetMetadata {
        BudgetMetadata {
            operation_id: self.operation_id,
            compute_budget_units: self.compute_budget_units,
            memory_budget_bytes: self.memory_budget_bytes,
            bandwidth_budget_bytes: self.bandwidth_budget_bytes,
            token_budget: self.token_budget,
            flags: self.flags,
        }
    }

    fn from_core(value: BudgetMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            compute_budget_units: value.compute_budget_units,
            memory_budget_bytes: value.memory_budget_bytes,
            bandwidth_budget_bytes: value.bandwidth_budget_bytes,
            token_budget: value.token_budget,
            flags: value.flags,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmProgress {
    #[serde(with = "canonical_u64")]
    operation_id: u64,
    #[serde(with = "canonical_u64")]
    progress_sequence: u64,
    stage_code: u16,
    percent_x100: u16,
    #[serde(with = "canonical_u64")]
    object_id: u64,
    body_bytes: u32,
}

impl WasmProgress {
    fn into_core(self) -> ProgressMetadata {
        ProgressMetadata {
            operation_id: self.operation_id,
            progress_sequence: self.progress_sequence,
            stage_code: self.stage_code,
            percent_x100: self.percent_x100,
            object_id: self.object_id,
            body_bytes: self.body_bytes,
        }
    }

    fn from_core(value: ProgressMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            progress_sequence: value.progress_sequence,
            stage_code: value.stage_code,
            percent_x100: value.percent_x100,
            object_id: value.object_id,
            body_bytes: value.body_bytes,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmPartialResult {
    #[serde(with = "canonical_u64")]
    operation_id: u64,
    #[serde(with = "canonical_u64")]
    result_sequence: u64,
    #[serde(with = "canonical_u64")]
    object_id: u64,
    #[serde(with = "canonical_u64")]
    delta_sequence: u64,
    body_bytes: u32,
    flags: u32,
}

impl WasmPartialResult {
    fn into_core(self) -> PartialResultMetadata {
        PartialResultMetadata {
            operation_id: self.operation_id,
            result_sequence: self.result_sequence,
            object_id: self.object_id,
            delta_sequence: self.delta_sequence,
            body_bytes: self.body_bytes,
            flags: self.flags,
        }
    }

    fn from_core(value: PartialResultMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            result_sequence: value.result_sequence,
            object_id: value.object_id,
            delta_sequence: value.delta_sequence,
            body_bytes: value.body_bytes,
            flags: value.flags,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmPressure {
    #[serde(with = "canonical_u64")]
    scope_id: u64,
    #[serde(with = "canonical_u64")]
    credit_window: u64,
    pressure_level: u16,
    pressure_reason: u16,
    retry_after_ms: u32,
    flags: u32,
}

impl WasmPressure {
    fn into_core(self) -> PressureMetadata {
        PressureMetadata {
            scope_id: self.scope_id,
            credit_window: self.credit_window,
            pressure_level: self.pressure_level,
            pressure_reason: self.pressure_reason,
            retry_after_ms: self.retry_after_ms,
            flags: self.flags,
        }
    }

    fn from_core(value: PressureMetadata) -> Self {
        Self {
            scope_id: value.scope_id,
            credit_window: value.credit_window,
            pressure_level: value.pressure_level,
            pressure_reason: value.pressure_reason,
            retry_after_ms: value.retry_after_ms,
            flags: value.flags,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmCapability {
    profile_id: u16,
    capability_count: u16,
    cost_model_id: u16,
    preference_rank: u16,
    #[serde(with = "canonical_u64")]
    limit_bytes: u64,
    #[serde(with = "canonical_u64")]
    limit_units: u64,
    body_bytes: u32,
    flags: u32,
}

impl WasmCapability {
    fn into_core(self) -> CapabilityMetadata {
        CapabilityMetadata {
            profile_id: self.profile_id,
            capability_count: self.capability_count,
            cost_model_id: self.cost_model_id,
            preference_rank: self.preference_rank,
            limit_bytes: self.limit_bytes,
            limit_units: self.limit_units,
            body_bytes: self.body_bytes,
            flags: self.flags,
        }
    }

    fn from_core(value: CapabilityMetadata) -> Self {
        Self {
            profile_id: value.profile_id,
            capability_count: value.capability_count,
            cost_model_id: value.cost_model_id,
            preference_rank: value.preference_rank,
            limit_bytes: value.limit_bytes,
            limit_units: value.limit_units,
            body_bytes: value.body_bytes,
            flags: value.flags,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmRouteHint {
    #[serde(with = "canonical_u64")]
    operation_id: u64,
    route_id: u32,
    executor_class: u16,
    affinity_class: u16,
    #[serde(with = "canonical_u64")]
    deadline_unix_ms: u64,
    body_bytes: u32,
    flags: u32,
}

impl WasmRouteHint {
    fn into_core(self) -> RouteHintMetadata {
        RouteHintMetadata {
            operation_id: self.operation_id,
            route_id: self.route_id,
            executor_class: self.executor_class,
            affinity_class: self.affinity_class,
            deadline_unix_ms: self.deadline_unix_ms,
            body_bytes: self.body_bytes,
            flags: self.flags,
        }
    }

    fn from_core(value: RouteHintMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            route_id: value.route_id,
            executor_class: value.executor_class,
            affinity_class: value.affinity_class,
            deadline_unix_ms: value.deadline_unix_ms,
            body_bytes: value.body_bytes,
            flags: value.flags,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmTraceContext {
    #[serde(with = "canonical_u64")]
    trace_id: u64,
    #[serde(with = "canonical_u64")]
    span_id: u64,
    #[serde(with = "canonical_u64")]
    parent_span_id: u64,
    stage_code: u16,
    flags: u16,
    body_bytes: u32,
}

impl WasmTraceContext {
    fn into_core(self) -> TraceContextMetadata {
        TraceContextMetadata {
            trace_id: self.trace_id,
            span_id: self.span_id,
            parent_span_id: self.parent_span_id,
            stage_code: self.stage_code,
            flags: self.flags,
            body_bytes: self.body_bytes,
        }
    }

    fn from_core(value: TraceContextMetadata) -> Self {
        Self {
            trace_id: value.trace_id,
            span_id: value.span_id,
            parent_span_id: value.parent_span_id,
            stage_code: value.stage_code,
            flags: value.flags,
            body_bytes: value.body_bytes,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmResultDropReason {
    #[serde(with = "canonical_u64")]
    operation_id: u64,
    #[serde(with = "canonical_u64")]
    result_sequence: u64,
    drop_reason_code: u16,
    source_role: u8,
    flags: u8,
    diagnostic_bytes: u32,
}

impl WasmResultDropReason {
    fn into_core(self) -> ResultDropReasonMetadata {
        ResultDropReasonMetadata {
            operation_id: self.operation_id,
            result_sequence: self.result_sequence,
            drop_reason_code: self.drop_reason_code,
            source_role: self.source_role,
            flags: self.flags,
            diagnostic_bytes: self.diagnostic_bytes,
        }
    }

    fn from_core(value: ResultDropReasonMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            result_sequence: value.result_sequence,
            drop_reason_code: value.drop_reason_code,
            source_role: value.source_role,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmRecoverableError {
    error_code: u32,
    error_scope: u32,
    recovery_action: u16,
    source_role: u8,
    flags: u8,
    retry_after_ms: u32,
    related_session_id: u32,
    related_frame_id: u32,
    related_view_id: u32,
    diagnostic_bytes: u32,
}

impl WasmRecoverableError {
    fn into_core(self) -> Result<RecoverableErrorMetadata, JsValue> {
        Ok(RecoverableErrorMetadata {
            error_code: self.error_code,
            error_scope: nnrp_core::ErrorScope::try_from_u32(self.error_scope)
                .map_err(js_nnrp_error)?,
            recovery_action: self.recovery_action,
            source_role: self.source_role,
            flags: self.flags,
            retry_after_ms: self.retry_after_ms,
            related_session_id: self.related_session_id,
            related_frame_id: self.related_frame_id,
            related_view_id: self.related_view_id,
            diagnostic_bytes: self.diagnostic_bytes,
        })
    }

    fn from_core(value: RecoverableErrorMetadata) -> Self {
        Self {
            error_code: value.error_code,
            error_scope: value.error_scope as u32,
            recovery_action: value.recovery_action,
            source_role: value.source_role,
            flags: value.flags,
            retry_after_ms: value.retry_after_ms,
            related_session_id: value.related_session_id,
            related_frame_id: value.related_frame_id,
            related_view_id: value.related_view_id,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmRetryAfter {
    #[serde(with = "canonical_u64")]
    scope_id: u64,
    #[serde(with = "canonical_u64")]
    control_sequence: u64,
    retry_after_ms: u32,
    jitter_ms: u32,
    reason_code: u16,
    source_role: u8,
    flags: u8,
    diagnostic_bytes: u32,
}

impl WasmRetryAfter {
    fn into_core(self) -> RetryAfterMetadata {
        RetryAfterMetadata {
            scope_id: self.scope_id,
            control_sequence: self.control_sequence,
            retry_after_ms: self.retry_after_ms,
            jitter_ms: self.jitter_ms,
            reason_code: self.reason_code,
            source_role: self.source_role,
            flags: self.flags,
            diagnostic_bytes: self.diagnostic_bytes,
        }
    }

    fn from_core(value: RetryAfterMetadata) -> Self {
        Self {
            scope_id: value.scope_id,
            control_sequence: value.control_sequence,
            retry_after_ms: value.retry_after_ms,
            jitter_ms: value.jitter_ms,
            reason_code: value.reason_code,
            source_role: value.source_role,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmObjectDescriptor {
    #[serde(with = "canonical_u64")]
    object_id: u64,
    object_kind: u16,
    producer_role: u8,
    consumer_role: u8,
    session_id: u32,
    #[serde(with = "canonical_u64")]
    byte_size: u64,
    compute_cost_units: u32,
    memory_location_hint: u16,
    ownership_hint: u16,
    lifetime_hint_ms: u32,
    metadata_bytes: u32,
}

impl WasmObjectDescriptor {
    fn into_core(self) -> Result<ObjectDescriptorMetadata, JsValue> {
        Ok(ObjectDescriptorMetadata {
            object_id: self.object_id,
            object_kind: RuntimeObjectKind::try_from_u16(self.object_kind)
                .map_err(js_nnrp_error)?,
            producer_role: RuntimeRole::try_from_u8(self.producer_role).map_err(js_nnrp_error)?,
            consumer_role: RuntimeRole::try_from_u8(self.consumer_role).map_err(js_nnrp_error)?,
            session_id: self.session_id,
            byte_size: self.byte_size,
            compute_cost_units: self.compute_cost_units,
            memory_location_hint: MemoryLocationHint::try_from_u16(self.memory_location_hint)
                .map_err(js_nnrp_error)?,
            ownership_hint: OwnershipHint::try_from_u16(self.ownership_hint)
                .map_err(js_nnrp_error)?,
            lifetime_hint_ms: self.lifetime_hint_ms,
            metadata_bytes: self.metadata_bytes,
        })
    }

    fn from_core(value: ObjectDescriptorMetadata) -> Self {
        Self {
            object_id: value.object_id,
            object_kind: value.object_kind as u16,
            producer_role: value.producer_role as u8,
            consumer_role: value.consumer_role as u8,
            session_id: value.session_id,
            byte_size: value.byte_size,
            compute_cost_units: value.compute_cost_units,
            memory_location_hint: value.memory_location_hint as u16,
            ownership_hint: value.ownership_hint as u16,
            lifetime_hint_ms: value.lifetime_hint_ms,
            metadata_bytes: value.metadata_bytes,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmObjectReference {
    #[serde(with = "canonical_u64")]
    object_id: u64,
    #[serde(with = "canonical_u64")]
    operation_id: u64,
    #[serde(with = "canonical_u64")]
    object_version: u64,
    #[serde(with = "canonical_u64")]
    offset: u64,
    #[serde(with = "canonical_u64")]
    length: u64,
    flags: u32,
    metadata_bytes: u32,
}

impl WasmObjectReference {
    fn into_core(self) -> ObjectReferenceMetadata {
        ObjectReferenceMetadata {
            object_id: self.object_id,
            operation_id: self.operation_id,
            object_version: self.object_version,
            offset: self.offset,
            length: self.length,
            flags: self.flags,
            metadata_bytes: self.metadata_bytes,
        }
    }

    fn from_core(value: ObjectReferenceMetadata) -> Self {
        Self {
            object_id: value.object_id,
            operation_id: value.operation_id,
            object_version: value.object_version,
            offset: value.offset,
            length: value.length,
            flags: value.flags,
            metadata_bytes: value.metadata_bytes,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmObjectRelease {
    #[serde(with = "canonical_u64")]
    object_id: u64,
    #[serde(with = "canonical_u64")]
    operation_id: u64,
    release_reason: u16,
    source_role: u8,
    flags: u8,
    diagnostic_bytes: u32,
}

impl WasmObjectRelease {
    fn into_core(self) -> Result<ObjectReleaseMetadata, JsValue> {
        Ok(ObjectReleaseMetadata {
            object_id: self.object_id,
            operation_id: self.operation_id,
            release_reason: ObjectReleaseReason::try_from_u16(self.release_reason)
                .map_err(js_nnrp_error)?,
            source_role: RuntimeRole::try_from_u8(self.source_role).map_err(js_nnrp_error)?,
            flags: self.flags,
            diagnostic_bytes: self.diagnostic_bytes,
        })
    }

    fn from_core(value: ObjectReleaseMetadata) -> Self {
        Self {
            object_id: value.object_id,
            operation_id: value.operation_id,
            release_reason: value.release_reason as u16,
            source_role: value.source_role as u8,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmObjectDelta {
    #[serde(with = "canonical_u64")]
    object_id: u64,
    #[serde(with = "canonical_u64")]
    delta_sequence: u64,
    #[serde(with = "canonical_u64")]
    region_offset: u64,
    region_bytes: u32,
    delta_bytes: u32,
    flags: u32,
    metadata_bytes: u32,
}

impl WasmObjectDelta {
    fn into_core(self) -> ObjectDeltaMetadata {
        ObjectDeltaMetadata {
            object_id: self.object_id,
            delta_sequence: self.delta_sequence,
            region_offset: self.region_offset,
            region_bytes: self.region_bytes,
            delta_bytes: self.delta_bytes,
            flags: self.flags,
            metadata_bytes: self.metadata_bytes,
        }
    }

    fn from_core(value: ObjectDeltaMetadata) -> Self {
        Self {
            object_id: value.object_id,
            delta_sequence: value.delta_sequence,
            region_offset: value.region_offset,
            region_bytes: value.region_bytes,
            delta_bytes: value.delta_bytes,
            flags: value.flags,
            metadata_bytes: value.metadata_bytes,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmCacheReference {
    cache_namespace: u32,
    #[serde(with = "canonical_u64")]
    cache_key_hi: u64,
    #[serde(with = "canonical_u64")]
    cache_key_lo: u64,
    profile_id: u16,
    reuse_scope: u16,
    #[serde(with = "canonical_u64")]
    lease_id: u64,
    #[serde(with = "canonical_u64")]
    producer_trace_id: u64,
    expiration_hint_ms: u32,
    metadata_bytes: u32,
    flags: u32,
}

impl WasmCacheReference {
    fn into_core(self) -> Result<CacheReferenceMetadata, JsValue> {
        Ok(CacheReferenceMetadata {
            cache_namespace: self.cache_namespace,
            cache_key_hi: self.cache_key_hi,
            cache_key_lo: self.cache_key_lo,
            profile_id: self.profile_id,
            reuse_scope: CacheReuseScope::try_from_u16(self.reuse_scope).map_err(js_nnrp_error)?,
            lease_id: self.lease_id,
            producer_trace_id: self.producer_trace_id,
            expiration_hint_ms: self.expiration_hint_ms,
            metadata_bytes: self.metadata_bytes,
            flags: self.flags,
        })
    }

    fn from_core(value: CacheReferenceMetadata) -> Self {
        Self {
            cache_namespace: value.cache_namespace,
            cache_key_hi: value.cache_key_hi,
            cache_key_lo: value.cache_key_lo,
            profile_id: value.profile_id,
            reuse_scope: value.reuse_scope as u16,
            lease_id: value.lease_id,
            producer_trace_id: value.producer_trace_id,
            expiration_hint_ms: value.expiration_hint_ms,
            metadata_bytes: value.metadata_bytes,
            flags: value.flags,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct WasmCacheMiss {
    cache_namespace: u32,
    #[serde(with = "canonical_u64")]
    cache_key_hi: u64,
    #[serde(with = "canonical_u64")]
    cache_key_lo: u64,
    miss_reason: u16,
    profile_id: u16,
    diagnostic_bytes: u32,
}

impl WasmCacheMiss {
    fn into_core(self) -> Result<CacheMissMetadata, JsValue> {
        Ok(CacheMissMetadata {
            cache_namespace: self.cache_namespace,
            cache_key_hi: self.cache_key_hi,
            cache_key_lo: self.cache_key_lo,
            miss_reason: CacheMissReason::try_from_u16(self.miss_reason).map_err(js_nnrp_error)?,
            profile_id: self.profile_id,
            diagnostic_bytes: self.diagnostic_bytes,
        })
    }

    fn from_core(value: CacheMissMetadata) -> Self {
        Self {
            cache_namespace: value.cache_namespace,
            cache_key_hi: value.cache_key_hi,
            cache_key_lo: value.cache_key_lo,
            miss_reason: value.miss_reason as u16,
            profile_id: value.profile_id,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[derive(Debug, Serialize)]
struct WasmTransportSelection {
    selected: WasmProviderOutput,
    candidates: Vec<WasmCandidateDiagnostic>,
}

impl From<nnrp_transport_provider::TransportSelection> for WasmTransportSelection {
    fn from(value: nnrp_transport_provider::TransportSelection) -> Self {
        Self {
            selected: value.selected.into(),
            candidates: value.candidates.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct WasmProviderOutput {
    name: String,
    version: String,
    transport_id: u32,
    kind: String,
    available: bool,
    metadata: WasmProviderMetadata,
    diagnostic: Option<String>,
}

impl From<TransportProviderDescriptor> for WasmProviderOutput {
    fn from(value: TransportProviderDescriptor) -> Self {
        Self {
            name: value.name,
            version: value.version,
            transport_id: value.transport_id as u32,
            kind: provider_kind_name(value.kind).to_string(),
            available: value.available,
            metadata: value.metadata.into(),
            diagnostic: value.diagnostic,
        }
    }
}

#[derive(Debug, Serialize)]
struct WasmCandidateDiagnostic {
    transport_id: u32,
    provider: WasmProviderMetadata,
    local_available: bool,
    peer_supported: bool,
    within_limits: bool,
    probe_state: String,
    probe: Option<WasmProbeMetrics>,
    selection_rank: Option<u32>,
    rejection_reason: Option<String>,
    diagnostic: Option<String>,
}

impl From<TransportCandidateDiagnostic> for WasmCandidateDiagnostic {
    fn from(value: TransportCandidateDiagnostic) -> Self {
        Self {
            transport_id: value.transport_id as u32,
            provider: value.provider.into(),
            local_available: value.local_available,
            peer_supported: value.peer_supported,
            within_limits: value.within_limits,
            probe_state: probe_state_name(value.probe_state).to_owned(),
            probe: value.probe.map(Into::into),
            selection_rank: value.selection_rank,
            rejection_reason: value
                .rejection_reason
                .map(transport_rejection_reason_name)
                .map(str::to_owned),
            diagnostic: value.diagnostic,
        }
    }
}

#[derive(Debug, Serialize)]
struct WasmProbeMetrics {
    sample_count: u32,
    success_count: u32,
    median_throughput_bytes_per_sec: String,
    median_rtt_us: String,
}

impl From<ProbeMetrics> for WasmProbeMetrics {
    fn from(value: ProbeMetrics) -> Self {
        Self {
            sample_count: value.sample_count,
            success_count: value.success_count,
            median_throughput_bytes_per_sec: value.median_throughput_bytes_per_sec.to_string(),
            median_rtt_us: value.median_rtt_us.to_string(),
        }
    }
}

impl From<TransportProviderMetadata> for WasmProviderMetadata {
    fn from(value: TransportProviderMetadata) -> Self {
        Self {
            id: value.id,
            cost: WasmProviderCost {
                model_id: value.cost.model_id,
                units: value.cost.units.to_string(),
            },
            preference_rank: value.preference_rank,
            limits: WasmProviderLimits {
                max_frame_bytes: value.limits.max_frame_bytes.to_string(),
            },
            limitations: value
                .limitations
                .into_iter()
                .map(provider_limitation_name)
                .map(str::to_owned)
                .collect(),
        }
    }
}

fn parse_providers(source: &str) -> Result<Vec<TransportProviderDescriptor>, JsValue> {
    let inputs = serde_json::from_str::<Vec<WasmProviderInput>>(source)
        .map_err(|error| js_error(&error.to_string()))?;
    inputs.into_iter().map(provider_from_input).collect()
}

fn parse_provider(source: &str) -> Result<TransportProviderDescriptor, JsValue> {
    let input = serde_json::from_str::<WasmProviderInput>(source)
        .map_err(|error| js_error(&error.to_string()))?;
    provider_from_input(input)
}

fn provider_from_input(input: WasmProviderInput) -> Result<TransportProviderDescriptor, JsValue> {
    let transport_id = parse_transport_id(input.transport_id)?;
    let kind = parse_provider_kind(input.kind.as_deref().unwrap_or("wasm"))?;
    let metadata = provider_metadata_from_input(input.metadata)?;
    let provider = if input.available.unwrap_or(true) {
        TransportProviderDescriptor::available(input.name, input.version, transport_id, kind)
    } else {
        TransportProviderDescriptor::missing(
            input.name,
            input.version,
            transport_id,
            kind,
            input
                .diagnostic
                .unwrap_or_else(|| "provider is not available".to_string()),
        )
    };
    Ok(provider.with_metadata(metadata))
}

fn provider_metadata_from_input(
    input: WasmProviderMetadata,
) -> Result<TransportProviderMetadata, JsValue> {
    if input.id.is_empty() || !input.id.is_ascii() {
        return Err(js_error("provider metadata id must be non-empty ASCII"));
    }
    let units = parse_canonical_u64(&input.cost.units)?;
    if input.cost.model_id == 0 && units != 0 {
        return Err(js_error(
            "provider cost units must be zero when model_id is zero",
        ));
    }
    let max_frame_bytes = parse_canonical_u64(&input.limits.max_frame_bytes)?;
    if max_frame_bytes == 0 {
        return Err(js_error("provider max_frame_bytes must be positive"));
    }
    let limitations = input
        .limitations
        .iter()
        .map(|value| parse_provider_limitation(value))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TransportProviderMetadata {
        id: input.id,
        cost: ProviderCost {
            model_id: input.cost.model_id,
            units,
        },
        preference_rank: input.preference_rank,
        limits: ProviderLimits { max_frame_bytes },
        limitations,
    })
}

fn parse_probe_samples(source: &str) -> Result<Vec<ProbeSample>, JsValue> {
    let inputs = serde_json::from_str::<Vec<WasmProbeSampleInput>>(source)
        .map_err(|error| js_error(&error.to_string()))?;
    inputs
        .into_iter()
        .map(|sample| {
            Ok(ProbeSample {
                transport_id: parse_transport_id(sample.transport_id)?,
                provider_id: sample.provider_id,
                elapsed_us: sample.elapsed_us,
                rtt_us: sample.rtt_us,
                bytes_sent: sample.bytes_sent,
                bytes_received: sample.bytes_received,
                timed_out: sample.timed_out.unwrap_or(false),
                failed: sample.failed.unwrap_or(false),
            })
        })
        .collect()
}

fn parse_transport_ids(source: &str) -> Result<Vec<TransportId>, JsValue> {
    let ids =
        serde_json::from_str::<Vec<u32>>(source).map_err(|error| js_error(&error.to_string()))?;
    ids.into_iter().map(parse_transport_id).collect()
}

fn parse_transport_id(value: u32) -> Result<TransportId, JsValue> {
    let transport_id = TransportId::try_from_u32(value)
        .map_err(|error| js_error(&format!("invalid transport id: {error}")))?;
    if transport_enabled(transport_id) {
        Ok(transport_id)
    } else {
        Err(js_error(&format!(
            "transport id {value} is not enabled in this wasm artifact"
        )))
    }
}

const fn transport_enabled(transport_id: TransportId) -> bool {
    match transport_id {
        TransportId::Quic => cfg!(feature = "transport-quic"),
        TransportId::Tcp => cfg!(feature = "transport-tcp"),
        TransportId::Ipc => cfg!(feature = "transport-ipc"),
        TransportId::WebSocket => cfg!(feature = "transport-websocket"),
        TransportId::Unspecified => false,
    }
}

fn header_from_input(
    input: WasmFrameHeaderInput,
    metadata_len: usize,
    body_len: usize,
) -> Result<CommonHeader, JsValue> {
    let metadata_len =
        u32::try_from(metadata_len).map_err(|_| js_error("metadata length exceeds u32"))?;
    let body_len = u32::try_from(body_len).map_err(|_| js_error("body length exceeds u32"))?;
    let mut header = CommonHeader::new(
        MessageType::try_from_u8(input.message_type).map_err(js_nnrp_error)?,
        metadata_len,
        body_len,
    );
    header.flags = HeaderFlags(input.flags.unwrap_or(HeaderFlags::NONE.0));
    header.session_id = input.session_id.unwrap_or(0);
    header.frame_id = input.frame_id.unwrap_or(0);
    header.view_id = input.view_id.unwrap_or(0);
    header.route_id = input.route_id.unwrap_or(0);
    header.trace_id = input.trace_id.unwrap_or(0);
    Ok(header)
}

fn parse_policy(value: &str) -> Result<TransportPolicy, JsValue> {
    match value {
        "auto" => Ok(TransportPolicy::Auto),
        "prefer_quic" => Ok(TransportPolicy::PreferQuic),
        "prefer_tcp" => Ok(TransportPolicy::PreferTcp),
        "prefer_ipc" => Ok(TransportPolicy::PreferIpc),
        "prefer_websocket" => Ok(TransportPolicy::PreferWebSocket),
        "force_quic" => Ok(TransportPolicy::ForceQuic),
        "force_tcp" => Ok(TransportPolicy::ForceTcp),
        "force_ipc" => Ok(TransportPolicy::ForceIpc),
        "force_websocket" => Ok(TransportPolicy::ForceWebSocket),
        other => Err(js_error(&format!("unknown transport policy: {other}"))),
    }
}

fn parse_canonical_u64_value(value: &str) -> Result<u64, &'static str> {
    let canonical = value == "0"
        || (!value.is_empty()
            && !value.starts_with('0')
            && value.bytes().all(|byte| byte.is_ascii_digit()));
    if !canonical {
        return Err("value must be a canonical decimal u64 string");
    }
    value
        .parse::<u64>()
        .map_err(|_| "value exceeds the u64 range")
}

fn parse_canonical_u64(value: &str) -> Result<u64, JsValue> {
    parse_canonical_u64_value(value).map_err(js_error)
}

mod canonical_u64 {
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        super::parse_canonical_u64_value(&value).map_err(D::Error::custom)
    }
}

fn parse_provider_limitation(value: &str) -> Result<ProviderLimitation, JsValue> {
    match value {
        "requires-udp" => Ok(ProviderLimitation::RequiresUdp),
        "requires-tcp" => Ok(ProviderLimitation::RequiresTcp),
        "local-host-only" => Ok(ProviderLimitation::LocalHostOnly),
        "native-host-only" => Ok(ProviderLimitation::NativeHostOnly),
        "browser-host-only" => Ok(ProviderLimitation::BrowserHostOnly),
        "unix-domain-socket" => Ok(ProviderLimitation::UnixDomainSocket),
        "windows-named-pipe" => Ok(ProviderLimitation::WindowsNamedPipe),
        other => Err(js_error(&format!("unknown provider limitation: {other}"))),
    }
}

fn provider_limitation_name(value: ProviderLimitation) -> &'static str {
    match value {
        ProviderLimitation::RequiresUdp => "requires-udp",
        ProviderLimitation::RequiresTcp => "requires-tcp",
        ProviderLimitation::LocalHostOnly => "local-host-only",
        ProviderLimitation::NativeHostOnly => "native-host-only",
        ProviderLimitation::BrowserHostOnly => "browser-host-only",
        ProviderLimitation::UnixDomainSocket => "unix-domain-socket",
        ProviderLimitation::WindowsNamedPipe => "windows-named-pipe",
    }
}

fn probe_state_name(value: ProbeState) -> &'static str {
    match value {
        ProbeState::NotRun => "not-run",
        ProbeState::Succeeded => "succeeded",
        ProbeState::Failed => "failed",
        ProbeState::Missing => "missing",
    }
}

fn transport_rejection_reason_name(value: TransportRejectionReason) -> &'static str {
    match value {
        TransportRejectionReason::PolicyDisallowed => "policy-disallowed",
        TransportRejectionReason::LocalUnavailable => "local-unavailable",
        TransportRejectionReason::PeerUnsupported => "peer-unsupported",
        TransportRejectionReason::LimitExceeded => "limit-exceeded",
        TransportRejectionReason::ProbeMissing => "probe-missing",
        TransportRejectionReason::ProbeFailed => "probe-failed",
    }
}

fn parse_provider_kind(value: &str) -> Result<TransportProviderKind, JsValue> {
    match value {
        "pure_rust" => Ok(TransportProviderKind::PureRust),
        "native_dynamic" => Ok(TransportProviderKind::NativeDynamic),
        "wasm" => Ok(TransportProviderKind::Wasm),
        other => Err(js_error(&format!("unknown provider kind: {other}"))),
    }
}

fn provider_kind_name(kind: TransportProviderKind) -> &'static str {
    match kind {
        TransportProviderKind::PureRust => "pure_rust",
        TransportProviderKind::NativeDynamic => "native_dynamic",
        TransportProviderKind::Wasm => "wasm",
    }
}

fn reject_tail_for_fixed_metadata(tail: &[u8]) -> Result<(), JsValue> {
    if tail.is_empty() {
        Ok(())
    } else {
        Err(js_error("metadata type does not carry a tail segment"))
    }
}

fn js_nnrp_error(error: NnrpError) -> JsValue {
    js_error(&error.to_string())
}

fn js_error(message: &str) -> JsValue {
    #[cfg(target_arch = "wasm32")]
    {
        JsValue::from_str(message)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = message;
        JsValue::NULL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_protocol_version_exports_current_values() {
        assert_eq!(nnrp_wasm_protocol_major(), ProtocolVersion::CURRENT.major);
        assert_eq!(
            nnrp_wasm_wire_format(),
            ProtocolVersion::CURRENT.wire_format
        );
    }

    #[test]
    fn wasm_websocket_binary_frame_codec_round_trips_offsets() {
        let header = format!(
            r#"{{"message_type":{},"session_id":7,"frame_id":9,"view_id":2,"route_id":3,"trace_id":123}}"#,
            MessageType::FrameSubmit as u8
        );
        let metadata = [1_u8, 2, 3, 4];
        let body = [5_u8, 6, 7];

        let frame = encode_websocket_binary_frame_json(&header, &metadata, &body)
            .expect("frame should encode");
        let decoded = decode_websocket_binary_frame_json(&frame).expect("frame should decode");
        let decoded = serde_json::from_str::<serde_json::Value>(&decoded).unwrap();

        assert_eq!(
            decoded["header"]["message_type"],
            MessageType::FrameSubmit as u8
        );
        assert_eq!(decoded["header"]["session_id"], 7);
        assert_eq!(decoded["header"]["frame_id"], 9);
        assert_eq!(decoded["header"]["view_id"], 2);
        assert_eq!(decoded["header"]["route_id"], 3);
        assert_eq!(decoded["header"]["trace_id"], 123);
        assert_eq!(decoded["metadata_offset"], COMMON_HEADER_LEN);
        assert_eq!(decoded["metadata_len"], metadata.len());
        assert_eq!(decoded["body_offset"], COMMON_HEADER_LEN + metadata.len());
        assert_eq!(decoded["body_len"], body.len());
        assert_eq!(
            &frame[COMMON_HEADER_LEN..COMMON_HEADER_LEN + metadata.len()],
            metadata
        );
        assert_eq!(&frame[COMMON_HEADER_LEN + metadata.len()..], body);
    }

    #[test]
    fn wasm_websocket_binary_frame_codec_rejects_malformed_frames() {
        let header = format!(r#"{{"message_type":{}}}"#, MessageType::Ping as u8);
        let mut frame = encode_websocket_binary_frame_json(&header, &[1, 2], &[3, 4])
            .expect("frame should encode");
        frame.pop();

        assert!(decode_websocket_binary_frame_json(&frame).is_err());
        assert!(encode_websocket_binary_frame_json(r#"{"message_type":255}"#, &[], &[]).is_err());
    }

    #[test]
    fn wasm_websocket_binary_frame_batch_decodes_offsets_and_limit() {
        let first_header = format!(
            r#"{{"message_type":{},"session_id":1,"frame_id":10}}"#,
            MessageType::Progress as u8
        );
        let second_header = format!(
            r#"{{"message_type":{},"session_id":1,"frame_id":11}}"#,
            MessageType::PartialResult as u8
        );
        let first =
            encode_websocket_binary_frame_json(&first_header, &[1, 2], &[3]).expect("first frame");
        let second = encode_websocket_binary_frame_json(&second_header, &[4], &[5, 6])
            .expect("second frame");
        let mut batch = first.clone();
        batch.extend_from_slice(&second);

        let decoded = decode_websocket_binary_frame_batch_json(&batch, 0)
            .expect("batch should decode without a limit");
        let decoded = serde_json::from_str::<serde_json::Value>(&decoded).unwrap();

        assert_eq!(decoded["frames"].as_array().unwrap().len(), 2);
        assert_eq!(decoded["frames"][0]["frame_offset"], 0);
        assert_eq!(decoded["frames"][0]["frame_len"], first.len());
        assert_eq!(decoded["frames"][0]["metadata_offset"], COMMON_HEADER_LEN);
        assert_eq!(decoded["frames"][0]["metadata_len"], 2);
        assert_eq!(decoded["frames"][0]["body_offset"], COMMON_HEADER_LEN + 2);
        assert_eq!(decoded["frames"][0]["body_len"], 1);
        assert_eq!(decoded["frames"][1]["frame_offset"], first.len());
        assert_eq!(
            decoded["frames"][1]["metadata_offset"],
            first.len() + COMMON_HEADER_LEN
        );
        assert_eq!(decoded["frames"][1]["body_len"], 2);
        assert_eq!(decoded["consumed_len"], batch.len());
        assert_eq!(decoded["remaining_len"], 0);

        let limited = decode_websocket_binary_frame_batch_json(&batch, 1)
            .expect("limited batch should decode");
        let limited = serde_json::from_str::<serde_json::Value>(&limited).unwrap();
        assert_eq!(limited["frames"].as_array().unwrap().len(), 1);
        assert_eq!(limited["consumed_len"], first.len());
        assert_eq!(limited["remaining_len"], second.len());
    }

    #[test]
    fn wasm_websocket_binary_frame_batch_rejects_incomplete_frame() {
        let header = format!(r#"{{"message_type":{}}}"#, MessageType::Progress as u8);
        let mut frame = encode_websocket_binary_frame_json(&header, &[1, 2], &[3])
            .expect("frame should encode");
        frame.pop();

        assert!(decode_websocket_binary_frame_batch_json(&frame, 0).is_err());
    }

    #[test]
    fn wasm_runtime_control_metadata_json_round_trips_progress() {
        let metadata = r#"{"operation_id":"7","progress_sequence":"2","stage_code":3,"percent_x100":4200,"object_id":"11","body_bytes":2}"#;
        let body = [9_u8, 8];

        let encoded =
            encode_runtime_control_metadata_json(MessageType::Progress as u8, metadata, &body)
                .expect("progress metadata should encode");
        let decoded = decode_runtime_control_metadata_json(MessageType::Progress as u8, &encoded)
            .expect("progress metadata should decode");
        let decoded = serde_json::from_str::<serde_json::Value>(&decoded).unwrap();

        assert_eq!(decoded["metadata"]["operation_id"], "7");
        assert_eq!(decoded["metadata"]["progress_sequence"], "2");
        assert_eq!(decoded["metadata"]["percent_x100"], 4200);
        assert_eq!(decoded["tail_len"], body.len());
        assert_eq!(decoded["tail_offset"], encoded.len() - body.len());
        assert_eq!(&encoded[encoded.len() - body.len()..], body);
    }

    #[test]
    fn wasm_runtime_control_metadata_json_rejects_tail_for_fixed_metadata() {
        let scheduling = r#"{"operation_id":"7","control_sequence":"1","priority_class":2,"priority_delta":-1,"deadline_unix_ms":"1000","flags":0}"#;

        assert!(encode_runtime_control_metadata_json(
            MessageType::PriorityUpdate as u8,
            scheduling,
            &[1],
        )
        .is_err());
        assert!(encode_runtime_control_metadata_json(
            MessageType::FrameSubmit as u8,
            scheduling,
            &[],
        )
        .is_err());
    }

    #[test]
    fn wasm_runtime_control_metadata_json_round_trips_preview4_profiles() {
        let cases = [
            (
                MessageType::Cancel,
                r#"{"operation_id":"10","control_sequence":"1","reason_code":2,"source_role":1,"flags":0,"diagnostic_bytes":2}"#,
                &[1_u8, 2][..],
                "operation_id",
                serde_json::json!("10"),
            ),
            (
                MessageType::Abort,
                r#"{"operation_id":"11","control_sequence":"2","reason_code":3,"source_role":2,"flags":0,"diagnostic_bytes":1}"#,
                &[3_u8][..],
                "operation_id",
                serde_json::json!("11"),
            ),
            (
                MessageType::Deadline,
                r#"{"operation_id":"12","control_sequence":"3","priority_class":4,"priority_delta":-2,"deadline_unix_ms":"5000","flags":0}"#,
                &[][..],
                "operation_id",
                serde_json::json!("12"),
            ),
            (
                MessageType::ExpireAt,
                r#"{"operation_id":"13","control_sequence":"4","priority_class":5,"priority_delta":2,"deadline_unix_ms":"6000","flags":0}"#,
                &[][..],
                "operation_id",
                serde_json::json!("13"),
            ),
            (
                MessageType::Supersede,
                r#"{"old_operation_id":"14","new_operation_id":"15","control_sequence":"5","drop_reason_code":6,"flags":0,"diagnostic_bytes":3}"#,
                &[4_u8, 5, 6][..],
                "new_operation_id",
                serde_json::json!("15"),
            ),
            (
                MessageType::BudgetUpdate,
                r#"{"operation_id":"16","compute_budget_units":"17","memory_budget_bytes":"18","bandwidth_budget_bytes":"19","token_budget":20,"flags":0}"#,
                &[][..],
                "operation_id",
                serde_json::json!("16"),
            ),
            (
                MessageType::PartialResult,
                r#"{"operation_id":"21","result_sequence":"22","object_id":"23","delta_sequence":"24","body_bytes":2,"flags":0}"#,
                &[7_u8, 8][..],
                "object_id",
                serde_json::json!("23"),
            ),
            (
                MessageType::Backpressure,
                r#"{"scope_id":"25","credit_window":"26","pressure_level":27,"pressure_reason":28,"retry_after_ms":29,"flags":0}"#,
                &[][..],
                "scope_id",
                serde_json::json!("25"),
            ),
            (
                MessageType::CreditUpdate,
                r#"{"scope_id":"30","credit_window":"31","pressure_level":32,"pressure_reason":33,"retry_after_ms":34,"flags":0}"#,
                &[][..],
                "scope_id",
                serde_json::json!("30"),
            ),
            (
                MessageType::CapabilityNegotiation,
                r#"{"profile_id":35,"capability_count":36,"cost_model_id":37,"preference_rank":38,"limit_bytes":"39","limit_units":"40","body_bytes":2,"flags":0}"#,
                &[9_u8, 10][..],
                "profile_id",
                serde_json::json!(35),
            ),
            (
                MessageType::DegradeProfile,
                r#"{"profile_id":41,"capability_count":42,"cost_model_id":43,"preference_rank":44,"limit_bytes":"45","limit_units":"46","body_bytes":1,"flags":0}"#,
                &[11_u8][..],
                "profile_id",
                serde_json::json!(41),
            ),
            (
                MessageType::RouteHint,
                r#"{"operation_id":"47","route_id":48,"executor_class":49,"affinity_class":50,"deadline_unix_ms":"51","body_bytes":2,"flags":0}"#,
                &[12_u8, 13][..],
                "operation_id",
                serde_json::json!("47"),
            ),
            (
                MessageType::ExecutionHint,
                r#"{"operation_id":"52","route_id":53,"executor_class":54,"affinity_class":55,"deadline_unix_ms":"56","body_bytes":1,"flags":0}"#,
                &[14_u8][..],
                "operation_id",
                serde_json::json!("52"),
            ),
            (
                MessageType::TraceContext,
                r#"{"trace_id":"57","span_id":"58","parent_span_id":"59","stage_code":60,"flags":0,"body_bytes":2}"#,
                &[15_u8, 16][..],
                "trace_id",
                serde_json::json!("57"),
            ),
            (
                MessageType::ResultDropReason,
                r#"{"operation_id":"61","result_sequence":"62","drop_reason_code":63,"source_role":1,"flags":0,"diagnostic_bytes":2}"#,
                &[17_u8, 18][..],
                "operation_id",
                serde_json::json!("61"),
            ),
            (
                MessageType::ErrorRecoverable,
                r#"{"error_code":64,"error_scope":1,"recovery_action":65,"source_role":2,"flags":0,"retry_after_ms":66,"related_session_id":67,"related_frame_id":68,"related_view_id":69,"diagnostic_bytes":1}"#,
                &[19_u8][..],
                "error_code",
                serde_json::json!(64),
            ),
            (
                MessageType::RetryAfter,
                r#"{"scope_id":"70","control_sequence":"71","retry_after_ms":72,"jitter_ms":73,"reason_code":74,"source_role":3,"flags":0,"diagnostic_bytes":2}"#,
                &[20_u8, 21][..],
                "scope_id",
                serde_json::json!("70"),
            ),
        ];

        for (message_type, metadata, tail, key, expected) in cases {
            let encoded = encode_runtime_control_metadata_json(message_type as u8, metadata, tail)
                .unwrap_or_else(|_| panic!("{message_type:?} should encode"));
            let decoded = decode_runtime_control_metadata_json(message_type as u8, &encoded)
                .unwrap_or_else(|_| panic!("{message_type:?} should decode"));
            let decoded = serde_json::from_str::<serde_json::Value>(&decoded).unwrap();

            assert_eq!(decoded["metadata"][key], expected, "{message_type:?}");
            assert_eq!(decoded["tail_len"], tail.len(), "{message_type:?}");
            if !tail.is_empty() {
                assert_eq!(decoded["tail_offset"], encoded.len() - tail.len());
                assert_eq!(&encoded[encoded.len() - tail.len()..], tail);
            }
        }
    }

    #[test]
    fn wasm_runtime_object_metadata_json_round_trips_object_declare() {
        let metadata = r#"{"object_id":"15","object_kind":1,"producer_role":3,"consumer_role":1,"session_id":9,"byte_size":"4096","compute_cost_units":17,"memory_location_hint":2,"ownership_hint":4,"lifetime_hint_ms":250,"metadata_bytes":3}"#;
        let extension = [1_u8, 2, 3];

        let encoded = encode_runtime_object_metadata_json(
            MessageType::ObjectDeclare as u8,
            metadata,
            &extension,
        )
        .expect("object metadata should encode");
        let decoded =
            decode_runtime_object_metadata_json(MessageType::ObjectDeclare as u8, &encoded)
                .expect("object metadata should decode");
        let decoded = serde_json::from_str::<serde_json::Value>(&decoded).unwrap();

        assert_eq!(decoded["metadata"]["object_id"], "15");
        assert_eq!(decoded["metadata"]["object_kind"], 1);
        assert_eq!(decoded["metadata"]["producer_role"], 3);
        assert_eq!(decoded["metadata"]["consumer_role"], 1);
        assert_eq!(decoded["tail_len"], extension.len());
        assert_eq!(decoded["tail_offset"], encoded.len() - extension.len());
        assert_eq!(&encoded[encoded.len() - extension.len()..], extension);
    }

    #[test]
    fn wasm_runtime_object_metadata_json_round_trips_preview4_profiles() {
        let cases = [
            (
                MessageType::ObjectRef,
                r#"{"object_id":"21","operation_id":"22","object_version":"23","offset":"24","length":"25","flags":0,"metadata_bytes":2}"#,
                &[1_u8, 2][..],
                "object_id",
                serde_json::json!("21"),
            ),
            (
                MessageType::ObjectRelease,
                r#"{"object_id":"26","operation_id":"27","release_reason":1,"source_role":1,"flags":0,"diagnostic_bytes":1}"#,
                &[3_u8][..],
                "operation_id",
                serde_json::json!("27"),
            ),
            (
                MessageType::ObjectPatch,
                r#"{"object_id":"28","delta_sequence":"29","region_offset":"30","region_bytes":31,"delta_bytes":32,"flags":0,"metadata_bytes":2}"#,
                &[4_u8, 5][..],
                "object_id",
                serde_json::json!("28"),
            ),
            (
                MessageType::ObjectDelta,
                r#"{"object_id":"33","delta_sequence":"34","region_offset":"35","region_bytes":36,"delta_bytes":37,"flags":0,"metadata_bytes":1}"#,
                &[6_u8][..],
                "delta_sequence",
                serde_json::json!("34"),
            ),
            (
                MessageType::CacheReference,
                r#"{"cache_namespace":37,"cache_key_hi":"18446744073709551615","cache_key_lo":"39","profile_id":40,"reuse_scope":1,"lease_id":"41","producer_trace_id":"42","expiration_hint_ms":43,"metadata_bytes":2,"flags":0}"#,
                &[7_u8, 8][..],
                "cache_key_hi",
                serde_json::json!("18446744073709551615"),
            ),
            (
                MessageType::CacheMiss,
                r#"{"cache_namespace":43,"cache_key_hi":"44","cache_key_lo":"18446744073709551615","miss_reason":1,"profile_id":46,"diagnostic_bytes":1}"#,
                &[9_u8][..],
                "cache_key_lo",
                serde_json::json!("18446744073709551615"),
            ),
        ];

        for (message_type, metadata, tail, key, expected) in cases {
            let encoded = encode_runtime_object_metadata_json(message_type as u8, metadata, tail)
                .unwrap_or_else(|_| panic!("{message_type:?} should encode"));
            let decoded = decode_runtime_object_metadata_json(message_type as u8, &encoded)
                .unwrap_or_else(|_| panic!("{message_type:?} should decode"));
            let decoded = serde_json::from_str::<serde_json::Value>(&decoded).unwrap();

            assert_eq!(decoded["metadata"][key], expected, "{message_type:?}");
            assert_eq!(decoded["tail_len"], tail.len(), "{message_type:?}");
            assert_eq!(decoded["tail_offset"], encoded.len() - tail.len());
            assert_eq!(&encoded[encoded.len() - tail.len()..], tail);
        }
    }

    #[test]
    fn wasm_runtime_metadata_json_rejects_invalid_preview4_enums() {
        assert!(encode_runtime_control_metadata_json(
            MessageType::ErrorRecoverable as u8,
            r#"{"error_code":1,"error_scope":99,"recovery_action":2,"source_role":1,"flags":0,"retry_after_ms":3,"related_session_id":4,"related_frame_id":5,"related_view_id":6,"diagnostic_bytes":0}"#,
            &[],
        )
        .is_err());
        assert!(encode_runtime_object_metadata_json(
            MessageType::ObjectDeclare as u8,
            r#"{"object_id":"1","object_kind":99,"producer_role":3,"consumer_role":1,"session_id":2,"byte_size":"3","compute_cost_units":4,"memory_location_hint":2,"ownership_hint":4,"lifetime_hint_ms":5,"metadata_bytes":0}"#,
            &[],
        )
        .is_err());
        assert!(encode_runtime_object_metadata_json(
            MessageType::ObjectRelease as u8,
            r#"{"object_id":"1","operation_id":"2","release_reason":99,"source_role":1,"flags":0,"diagnostic_bytes":0}"#,
            &[],
        )
        .is_err());
        assert!(encode_runtime_object_metadata_json(
            MessageType::CacheReference as u8,
            r#"{"cache_namespace":1,"cache_key_hi":"1","cache_key_lo":"2","profile_id":3,"reuse_scope":99,"lease_id":"4","producer_trace_id":"5","expiration_hint_ms":6,"metadata_bytes":0,"flags":0}"#,
            &[],
        )
        .is_err());
        assert!(encode_runtime_object_metadata_json(
            MessageType::CacheMiss as u8,
            r#"{"cache_namespace":1,"cache_key_hi":"1","cache_key_lo":"2","miss_reason":99,"profile_id":3,"diagnostic_bytes":0}"#,
            &[],
        )
        .is_err());
        assert!(decode_runtime_control_metadata_json(MessageType::FrameSubmit as u8, &[]).is_err());
        assert!(decode_runtime_object_metadata_json(MessageType::FrameSubmit as u8, &[]).is_err());
    }

    #[test]
    fn wasm_runtime_metadata_json_rejects_noncanonical_u64_values() {
        let numeric = r#"{"operation_id":1,"progress_sequence":"2","stage_code":3,"percent_x100":4200,"object_id":"11","body_bytes":0}"#;
        assert!(
            encode_runtime_control_metadata_json(MessageType::Progress as u8, numeric, &[],)
                .is_err()
        );

        for value in ["-1", "01", "+1", "", "18446744073709551616"] {
            let metadata = format!(
                r#"{{"cache_namespace":1,"cache_key_hi":"{value}","cache_key_lo":"2","miss_reason":1,"profile_id":3,"diagnostic_bytes":0}}"#
            );
            assert!(encode_runtime_object_metadata_json(
                MessageType::CacheMiss as u8,
                &metadata,
                &[],
            )
            .is_err());
        }
    }

    #[cfg(all(feature = "transport-tcp", feature = "transport-quic"))]
    #[test]
    fn wasm_probe_selection_prefers_measured_tcp_over_flaky_quic() {
        let providers = format!(
            "[{},{}]",
            provider_json("tcp", "nnrp.transport.tcp.native", 2, "pure_rust", true),
            provider_json("quic", "nnrp.transport.quic.native", 1, "pure_rust", true)
        );
        let samples = r#"[
            {"transport_id":2,"provider_id":"nnrp.transport.tcp.native","elapsed_us":20000,"rtt_us":5000,"bytes_sent":1024,"bytes_received":1024},
            {"transport_id":2,"provider_id":"nnrp.transport.tcp.native","elapsed_us":20000,"rtt_us":5100,"bytes_sent":1024,"bytes_received":1024},
            {"transport_id":1,"provider_id":"nnrp.transport.quic.native","elapsed_us":20000,"rtt_us":800,"bytes_sent":1024,"bytes_received":1024},
            {"transport_id":1,"provider_id":"nnrp.transport.quic.native","elapsed_us":20000,"rtt_us":null,"bytes_sent":0,"bytes_received":0,"timed_out":true,"failed":true}
        ]"#;

        let output =
            select_transport_with_probe_json(&providers, "[1,2]", "prefer_quic", None, samples)
                .unwrap();
        let output = serde_json::from_str::<serde_json::Value>(&output).unwrap();
        assert_eq!(output["selected"]["transport_id"], 2);
        assert_eq!(output["candidates"].as_array().unwrap().len(), 2);
    }

    #[cfg(all(feature = "transport-tcp", feature = "transport-quic"))]
    #[test]
    fn wasm_probe_selection_reports_rejected_unavailable_provider() {
        let providers = format!(
            "[{},{}]",
            provider_json(
                "tcp-native",
                "nnrp.transport.tcp.native",
                2,
                "native_dynamic",
                true
            ),
            provider_json(
                "quic-native",
                "nnrp.transport.quic.native",
                1,
                "pure_rust",
                false
            )
        );
        let samples = r#"[
            {"transport_id":2,"provider_id":"nnrp.transport.tcp.native","elapsed_us":10000,"rtt_us":2500,"bytes_sent":4096,"bytes_received":4096}
        ]"#;

        let output =
            select_transport_with_probe_json(&providers, "[1,2]", "prefer_tcp", None, samples)
                .unwrap();
        let output = serde_json::from_str::<serde_json::Value>(&output).unwrap();

        assert_eq!(output["selected"]["kind"], "native_dynamic");
        let rejected = output["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|candidate| candidate["transport_id"] == 1)
            .unwrap();
        assert_eq!(rejected["rejection_reason"], "local-unavailable");
    }

    #[cfg(feature = "transport-quic")]
    #[test]
    fn wasm_summarizes_provider_probe_as_structured_metrics() {
        let provider = provider_json("quic", "nnrp.transport.quic.native", 1, "pure_rust", true);
        let samples = r#"[
            {"transport_id":1,"provider_id":"nnrp.transport.quic.native","elapsed_us":8000,"rtt_us":1000,"bytes_sent":2048,"bytes_received":2048},
            {"transport_id":1,"provider_id":"nnrp.transport.quic.native","elapsed_us":9000,"rtt_us":1200,"bytes_sent":2048,"bytes_received":2048}
        ]"#;

        let output = summarize_provider_probe_json(&provider, samples).unwrap();
        let output = serde_json::from_str::<serde_json::Value>(&output).unwrap();

        assert_eq!(output["sample_count"], 2);
        assert_eq!(output["success_count"], 2);
        assert_eq!(output["median_rtt_us"], "1100");
    }

    #[cfg(feature = "transport-tcp")]
    #[test]
    fn wasm_probe_summary_reports_missing_samples() {
        let provider = provider_json("tcp", "nnrp.transport.tcp.native", 2, "pure_rust", true);
        assert!(summarize_provider_probe_json(&provider, "[]").is_err());
    }

    #[cfg(feature = "transport-tcp")]
    #[test]
    fn wasm_provider_metadata_validation_covers_the_frozen_registry() {
        let provider = serde_json::json!({
            "name": "tcp",
            "version": "0.0.0",
            "transport_id": 2,
            "kind": "pure_rust",
            "available": true,
            "metadata": {
                "id": "nnrp.transport.tcp.native",
                "cost": { "model_id": 1, "units": "7" },
                "preference_rank": 3,
                "limits": { "max_frame_bytes": "67108864" },
                "limitations": [
                    "requires-udp",
                    "requires-tcp",
                    "local-host-only",
                    "native-host-only",
                    "browser-host-only",
                    "unix-domain-socket",
                    "windows-named-pipe"
                ]
            }
        })
        .to_string();
        let samples = r#"[{
            "transport_id":2,
            "provider_id":"nnrp.transport.tcp.native",
            "elapsed_us":10,
            "rtt_us":5,
            "bytes_sent":10,
            "bytes_received":10
        }]"#;

        let selection =
            select_transport_with_probe_json(&format!("[{provider}]"), "[2]", "auto", None, "[]")
                .unwrap();
        let selection = serde_json::from_str::<serde_json::Value>(&selection).unwrap();
        assert_eq!(
            selection["candidates"][0]["provider"]["limitations"]
                .as_array()
                .unwrap()
                .len(),
            7
        );

        let output = summarize_provider_probe_json(&provider, samples).unwrap();
        let output = serde_json::from_str::<serde_json::Value>(&output).unwrap();
        assert_eq!(output["success_count"], 1);

        for invalid_metadata in [
            serde_json::json!({
                "id": "",
                "cost": { "model_id": 0, "units": "0" },
                "preference_rank": 3,
                "limits": { "max_frame_bytes": "1" },
                "limitations": []
            }),
            serde_json::json!({
                "id": "nnrp.transport.websocket.browser-wasm",
                "cost": { "model_id": 0, "units": "1" },
                "preference_rank": 3,
                "limits": { "max_frame_bytes": "1" },
                "limitations": []
            }),
            serde_json::json!({
                "id": "nnrp.transport.websocket.browser-wasm",
                "cost": { "model_id": 0, "units": "0" },
                "preference_rank": 3,
                "limits": { "max_frame_bytes": "0" },
                "limitations": []
            }),
            serde_json::json!({
                "id": "nnrp.transport.websocket.browser-wasm",
                "cost": { "model_id": 0, "units": "00" },
                "preference_rank": 3,
                "limits": { "max_frame_bytes": "1" },
                "limitations": []
            }),
            serde_json::json!({
                "id": "nnrp.transport.websocket.browser-wasm",
                "cost": { "model_id": 0, "units": "0" },
                "preference_rank": 3,
                "limits": { "max_frame_bytes": "18446744073709551616" },
                "limitations": []
            }),
            serde_json::json!({
                "id": "nnrp.transport.websocket.browser-wasm",
                "cost": { "model_id": 0, "units": "0" },
                "preference_rank": 3,
                "limits": { "max_frame_bytes": "1" },
                "limitations": ["unknown"]
            }),
        ] {
            let mut invalid = serde_json::from_str::<serde_json::Value>(&provider).unwrap();
            invalid["metadata"] = invalid_metadata;
            assert!(summarize_provider_probe_json(&invalid.to_string(), samples).is_err());
        }
    }

    #[cfg(feature = "transport-tcp")]
    #[test]
    fn wasm_rejects_invalid_policy_kind_and_transport_id() {
        let tcp = provider_json("tcp", "nnrp.transport.tcp.native", 2, "pure_rust", true);
        let unspecified = provider_json(
            "unspecified",
            "nnrp.transport.unspecified.native",
            0,
            "pure_rust",
            true,
        );
        let bad_kind = provider_json("tcp", "nnrp.transport.tcp.native", 2, "plugin", true);
        let bad_transport =
            provider_json("tcp", "nnrp.transport.tcp.native", 99, "pure_rust", true);

        assert!(
            select_transport_with_probe_json(&format!("[{tcp}]"), "[2]", "sticky", None, "[]")
                .is_err()
        );
        assert!(summarize_provider_probe_json(&unspecified, "[]").is_err());
        assert!(summarize_provider_probe_json(&bad_kind, "[]").is_err());
        assert!(summarize_provider_probe_json(&bad_transport, "[]").is_err());
    }

    #[test]
    fn wasm_accepts_new_transport_policy_names() {
        assert_eq!(
            parse_policy("prefer_ipc").unwrap(),
            TransportPolicy::PreferIpc
        );
        assert_eq!(
            parse_policy("prefer_websocket").unwrap(),
            TransportPolicy::PreferWebSocket
        );
        assert_eq!(
            parse_policy("force_ipc").unwrap(),
            TransportPolicy::ForceIpc
        );
        assert_eq!(
            parse_policy("force_websocket").unwrap(),
            TransportPolicy::ForceWebSocket
        );
    }

    #[cfg(all(feature = "transport-tcp", not(feature = "transport-quic")))]
    #[test]
    fn wasm_tcp_scoped_artifact_rejects_quic_provider() {
        let quic = provider_json("quic", "nnrp.transport.quic.native", 1, "pure_rust", true);
        assert!(summarize_provider_probe_json(&quic, "[]").is_err());
    }

    #[cfg(all(feature = "transport-quic", not(feature = "transport-tcp")))]
    #[test]
    fn wasm_quic_scoped_artifact_rejects_tcp_provider() {
        let tcp = provider_json("tcp", "nnrp.transport.tcp.native", 2, "pure_rust", true);
        assert!(summarize_provider_probe_json(&tcp, "[]").is_err());
    }

    fn provider_json(
        name: &str,
        provider_id: &str,
        transport_id: u32,
        kind: &str,
        available: bool,
    ) -> String {
        serde_json::json!({
            "name": name,
            "version": "0.0.0",
            "transport_id": transport_id,
            "kind": kind,
            "available": available,
            "metadata": {
                "id": provider_id,
                "cost": { "model_id": 0, "units": "0" },
                "preference_rank": transport_id,
                "limits": { "max_frame_bytes": "67108864" },
                "limitations": []
            }
        })
        .to_string()
    }
}
