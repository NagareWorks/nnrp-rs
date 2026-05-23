use nnrp_core::{ProtocolVersion, TransportId};
use nnrp_transport_provider::{
    select_transport_with_probe, ProbeSample, RemoteTransportSupport, TransportPolicy,
    TransportProviderDescriptor, TransportProviderKind,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

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
    TransportId::try_from_u32(value)
        .map_err(|error| js_error(&format!("invalid transport id: {error}")))
}

fn parse_policy(value: &str) -> Result<TransportPolicy, JsValue> {
    match value {
        "auto" => Ok(TransportPolicy::Auto),
        "prefer_quic" => Ok(TransportPolicy::PreferQuic),
        "prefer_tcp" => Ok(TransportPolicy::PreferTcp),
        "force_quic" => Ok(TransportPolicy::ForceQuic),
        "force_tcp" => Ok(TransportPolicy::ForceTcp),
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
    use serde_json::Value;

    #[test]
    fn wasm_protocol_version_exports_current_values() {
        assert_eq!(nnrp_wasm_protocol_major(), ProtocolVersion::CURRENT.major);
        assert_eq!(
            nnrp_wasm_wire_format(),
            ProtocolVersion::CURRENT.wire_format
        );
    }

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
        let output = serde_json::from_str::<Value>(&output).unwrap();
        assert_eq!(output["selected"]["transport_id"], 2);
        assert_eq!(output["candidates"].as_array().unwrap().len(), 2);
    }

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
        let output = serde_json::from_str::<Value>(&output).unwrap();

        assert_eq!(output["selected"]["kind"], "native_dynamic");
        assert_eq!(output["rejected"][0]["transport_id"], 1);
        assert_eq!(output["rejected"][0]["provider_name"], "quic-native");
        assert!(output["rejected"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("LocalProviderUnavailable"));
    }

    #[test]
    fn wasm_score_provider_probe_returns_json_score() {
        let provider = r#"{"name":"quic","version":"0.0.0","transport_id":1,"kind":"pure_rust","available":true}"#;
        let samples = r#"[
            {"transport_id":1,"provider_name":"quic","elapsed_us":8000,"rtt_us":1000,"bytes_sent":2048,"bytes_received":2048},
            {"transport_id":1,"provider_name":"quic","elapsed_us":9000,"rtt_us":1200,"bytes_sent":2048,"bytes_received":2048}
        ]"#;

        let output = score_provider_probe_json(provider, "force_quic", samples).unwrap();
        let output = serde_json::from_str::<Value>(&output).unwrap();

        assert_eq!(output["sample_count"], 2);
        assert_eq!(output["failure_count"], 0);
        assert_eq!(output["median_rtt_us"], 1200);
    }

    #[test]
    fn wasm_score_provider_probe_reports_missing_samples() {
        let provider =
            r#"{"name":"tcp","version":"0.0.0","transport_id":2,"kind":"wasm","available":true}"#;
        assert!(score_provider_probe_json(provider, "auto", "[]").is_err());
    }

    #[test]
    fn wasm_rejects_invalid_policy_kind_and_transport_id() {
        let tcp =
            r#"{"name":"tcp","version":"0.0.0","transport_id":2,"kind":"wasm","available":true}"#;
        let bad_kind =
            r#"{"name":"tcp","version":"0.0.0","transport_id":2,"kind":"plugin","available":true}"#;
        let bad_transport =
            r#"{"name":"tcp","version":"0.0.0","transport_id":99,"kind":"wasm","available":true}"#;

        assert!(score_provider_probe_json(tcp, "sticky", "[]").is_err());
        assert!(score_provider_probe_json(bad_kind, "force_tcp", "[]").is_err());
        assert!(score_provider_probe_json(bad_transport, "force_tcp", "[]").is_err());
    }
}
