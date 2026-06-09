use nnrp_core::{
    CommonHeader, HeaderFlags, MessageType, NnrpError, ProtocolVersion, TransportId,
    COMMON_HEADER_LEN,
};
use nnrp_transport_provider::{
    select_transport_with_probe, ProbeSample, RemoteTransportSupport, TransportPolicy,
    TransportProviderDescriptor, TransportProviderKind,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

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
    samples_json: &str,
) -> Result<String, JsValue> {
    let providers = parse_providers(providers_json)?;
    let remote = RemoteTransportSupport::new(parse_transport_ids(remote_transports_json)?);
    let policy = parse_policy(policy)?;
    let samples = parse_probe_samples(samples_json)?;
    let selection = select_transport_with_probe(&providers, &remote, policy, &samples)
        .map_err(|error| js_error(&error.to_string()))?;

    serde_json::to_string(&WasmTransportSelection::from(selection))
        .map_err(|error| js_error(&error.to_string()))
}

#[wasm_bindgen(js_name = scoreProviderProbeJson)]
pub fn score_provider_probe_json(
    provider_json: &str,
    policy: &str,
    samples_json: &str,
) -> Result<String, JsValue> {
    let provider = parse_provider(provider_json)?;
    let policy = parse_policy(policy)?;
    let samples = parse_probe_samples(samples_json)?;
    let score = nnrp_transport_provider::score_provider_probe(&provider, &samples, policy)
        .ok_or_else(|| js_error("no matching probe samples for provider"))?;

    serde_json::to_string(&WasmProbeScore::from(score))
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

#[derive(Debug, Deserialize)]
struct WasmProviderInput {
    name: String,
    version: String,
    transport_id: u32,
    kind: Option<String>,
    available: Option<bool>,
    diagnostic: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WasmProbeSampleInput {
    transport_id: u32,
    provider_name: String,
    elapsed_us: u64,
    rtt_us: Option<u64>,
    bytes_sent: u64,
    bytes_received: u64,
    timed_out: Option<bool>,
    failed: Option<bool>,
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
struct WasmTransportSelection {
    selected: WasmProviderOutput,
    selected_score: WasmProbeScore,
    candidates: Vec<WasmCandidateScore>,
    rejected: Vec<WasmRejectedCandidate>,
}

impl From<nnrp_transport_provider::ProbeSelection> for WasmTransportSelection {
    fn from(value: nnrp_transport_provider::ProbeSelection) -> Self {
        Self {
            selected: value.selected.into(),
            selected_score: value.selected_score.into(),
            candidates: value.candidates.into_iter().map(Into::into).collect(),
            rejected: value.rejected.into_iter().map(Into::into).collect(),
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
            diagnostic: value.diagnostic,
        }
    }
}

#[derive(Debug, Serialize)]
struct WasmCandidateScore {
    provider: WasmProviderOutput,
    probe_score: WasmProbeScore,
}

impl From<nnrp_transport_provider::ProbeCandidateScore> for WasmCandidateScore {
    fn from(value: nnrp_transport_provider::ProbeCandidateScore) -> Self {
        Self {
            provider: value.provider.into(),
            probe_score: value.probe_score.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct WasmProbeScore {
    sample_count: usize,
    failure_count: usize,
    failure_rate: f64,
    median_rtt_us: u64,
    throughput_bytes_per_sec: u64,
    score: f64,
}

impl From<nnrp_transport_provider::ProbeScore> for WasmProbeScore {
    fn from(value: nnrp_transport_provider::ProbeScore) -> Self {
        Self {
            sample_count: value.sample_count,
            failure_count: value.failure_count,
            failure_rate: value.failure_rate,
            median_rtt_us: value.median_rtt_us,
            throughput_bytes_per_sec: value.throughput_bytes_per_sec,
            score: value.score,
        }
    }
}

#[derive(Debug, Serialize)]
struct WasmRejectedCandidate {
    transport_id: u32,
    provider_name: Option<String>,
    reason: String,
}

impl From<nnrp_transport_provider::RejectedTransportCandidate> for WasmRejectedCandidate {
    fn from(value: nnrp_transport_provider::RejectedTransportCandidate) -> Self {
        Self {
            transport_id: value.transport_id as u32,
            provider_name: value.provider_name,
            reason: format!("{:?}", value.reason),
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
    if input.available.unwrap_or(true) {
        Ok(TransportProviderDescriptor::available(
            input.name,
            input.version,
            transport_id,
            kind,
        ))
    } else {
        Ok(TransportProviderDescriptor::missing(
            input.name,
            input.version,
            transport_id,
            kind,
            input
                .diagnostic
                .unwrap_or_else(|| "provider is not available".to_string()),
        ))
    }
}

fn parse_probe_samples(source: &str) -> Result<Vec<ProbeSample>, JsValue> {
    let inputs = serde_json::from_str::<Vec<WasmProbeSampleInput>>(source)
        .map_err(|error| js_error(&error.to_string()))?;
    inputs
        .into_iter()
        .map(|sample| {
            Ok(ProbeSample {
                transport_id: parse_transport_id(sample.transport_id)?,
                provider_name: sample.provider_name,
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

    #[cfg(all(feature = "transport-tcp", feature = "transport-quic"))]
    #[test]
    fn wasm_probe_selection_prefers_measured_tcp_over_flaky_quic() {
        let providers = r#"[
            {"name":"tcp","version":"0.0.0","transport_id":2,"kind":"wasm","available":true},
            {"name":"quic","version":"0.0.0","transport_id":1,"kind":"wasm","available":true}
        ]"#;
        let samples = r#"[
            {"transport_id":2,"provider_name":"tcp","elapsed_us":20000,"rtt_us":5000,"bytes_sent":1024,"bytes_received":1024},
            {"transport_id":2,"provider_name":"tcp","elapsed_us":20000,"rtt_us":5100,"bytes_sent":1024,"bytes_received":1024},
            {"transport_id":1,"provider_name":"quic","elapsed_us":20000,"rtt_us":800,"bytes_sent":1024,"bytes_received":1024},
            {"transport_id":1,"provider_name":"quic","elapsed_us":20000,"rtt_us":null,"bytes_sent":0,"bytes_received":0,"timed_out":true,"failed":true}
        ]"#;

        let output =
            select_transport_with_probe_json(providers, "[1,2]", "prefer_quic", samples).unwrap();
        let output = serde_json::from_str::<serde_json::Value>(&output).unwrap();
        assert_eq!(output["selected"]["transport_id"], 2);
        assert_eq!(output["candidates"].as_array().unwrap().len(), 2);
    }

    #[cfg(all(feature = "transport-tcp", feature = "transport-quic"))]
    #[test]
    fn wasm_probe_selection_reports_rejected_unavailable_provider() {
        let providers = r#"[
            {"name":"tcp-native","version":"0.0.0","transport_id":2,"kind":"native_dynamic","available":true},
            {"name":"quic-native","version":"0.0.0","transport_id":1,"kind":"pure_rust","available":false,"diagnostic":"backend missing"}
        ]"#;
        let samples = r#"[
            {"transport_id":2,"provider_name":"tcp-native","elapsed_us":10000,"rtt_us":2500,"bytes_sent":4096,"bytes_received":4096}
        ]"#;

        let output =
            select_transport_with_probe_json(providers, "[1,2]", "prefer_tcp", samples).unwrap();
        let output = serde_json::from_str::<serde_json::Value>(&output).unwrap();

        assert_eq!(output["selected"]["kind"], "native_dynamic");
        assert_eq!(output["rejected"][0]["transport_id"], 1);
        assert_eq!(output["rejected"][0]["provider_name"], "quic-native");
        assert!(output["rejected"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("LocalProviderUnavailable"));
    }

    #[cfg(feature = "transport-quic")]
    #[test]
    fn wasm_score_provider_probe_returns_json_score() {
        let provider = r#"{"name":"quic","version":"0.0.0","transport_id":1,"kind":"pure_rust","available":true}"#;
        let samples = r#"[
            {"transport_id":1,"provider_name":"quic","elapsed_us":8000,"rtt_us":1000,"bytes_sent":2048,"bytes_received":2048},
            {"transport_id":1,"provider_name":"quic","elapsed_us":9000,"rtt_us":1200,"bytes_sent":2048,"bytes_received":2048}
        ]"#;

        let output = score_provider_probe_json(provider, "force_quic", samples).unwrap();
        let output = serde_json::from_str::<serde_json::Value>(&output).unwrap();

        assert_eq!(output["sample_count"], 2);
        assert_eq!(output["failure_count"], 0);
        assert_eq!(output["median_rtt_us"], 1200);
    }

    #[cfg(feature = "transport-tcp")]
    #[test]
    fn wasm_score_provider_probe_reports_missing_samples() {
        let provider =
            r#"{"name":"tcp","version":"0.0.0","transport_id":2,"kind":"wasm","available":true}"#;
        assert!(score_provider_probe_json(provider, "auto", "[]").is_err());
    }

    #[cfg(feature = "transport-tcp")]
    #[test]
    fn wasm_rejects_invalid_policy_kind_and_transport_id() {
        let tcp =
            r#"{"name":"tcp","version":"0.0.0","transport_id":2,"kind":"wasm","available":true}"#;
        let unspecified = r#"{"name":"unspecified","version":"0.0.0","transport_id":0,"kind":"wasm","available":true}"#;
        let bad_kind =
            r#"{"name":"tcp","version":"0.0.0","transport_id":2,"kind":"plugin","available":true}"#;
        let bad_transport =
            r#"{"name":"tcp","version":"0.0.0","transport_id":99,"kind":"wasm","available":true}"#;
        let ipc =
            r#"{"name":"ipc","version":"0.0.0","transport_id":3,"kind":"wasm","available":true}"#;
        let websocket = r#"{"name":"websocket","version":"0.0.0","transport_id":4,"kind":"wasm","available":true}"#;

        assert!(score_provider_probe_json(tcp, "sticky", "[]").is_err());
        assert!(score_provider_probe_json(unspecified, "auto", "[]").is_err());
        assert!(score_provider_probe_json(bad_kind, "force_tcp", "[]").is_err());
        assert!(score_provider_probe_json(bad_transport, "force_tcp", "[]").is_err());
        assert!(score_provider_probe_json(ipc, "force_ipc", "[]").is_err());
        assert!(score_provider_probe_json(websocket, "force_websocket", "[]").is_err());
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
        let quic =
            r#"{"name":"quic","version":"0.0.0","transport_id":1,"kind":"wasm","available":true}"#;
        assert!(score_provider_probe_json(quic, "force_quic", "[]").is_err());
    }

    #[cfg(all(feature = "transport-quic", not(feature = "transport-tcp")))]
    #[test]
    fn wasm_quic_scoped_artifact_rejects_tcp_provider() {
        let tcp =
            r#"{"name":"tcp","version":"0.0.0","transport_id":2,"kind":"wasm","available":true}"#;
        assert!(score_provider_probe_json(tcp, "force_tcp", "[]").is_err());
    }
}
