export type TransportPolicy =
  | "auto"
  | "prefer_quic"
  | "prefer_tcp"
  | "force_quic"
  | "force_tcp";

export type ProviderKind = "pure_rust" | "native_dynamic" | "wasm";

export interface TransportProviderInput {
  name: string;
  version: string;
  transport_id: 1 | 2;
  kind?: ProviderKind;
  available?: boolean;
  diagnostic?: string;
}

export interface ProbeSampleInput {
  transport_id: 1 | 2;
  provider_name: string;
  elapsed_us: number;
  rtt_us: number | null;
  bytes_sent: number;
  bytes_received: number;
  timed_out?: boolean;
  failed?: boolean;
}

export interface ProbeScore {
  sample_count: number;
  failure_count: number;
  failure_rate: number;
  median_rtt_us: number;
  throughput_bytes_per_sec: number;
  score: number;
}

export interface TransportSelection {
  selected: TransportProviderInput;
  selected_score: ProbeScore;
  candidates: Array<{ provider: TransportProviderInput; probe_score: ProbeScore }>;
  rejected: Array<{ transport_id: 1 | 2; provider_name?: string; reason: string }>;
}

export function nnrp_wasm_protocol_major(): number;
export function nnrp_wasm_wire_format(): number;

export function selectTransportWithProbeJson(
  providersJson: string,
  remoteTransportsJson: string,
  policy: TransportPolicy,
  samplesJson: string,
): string;

export function scoreProviderProbeJson(
  providerJson: string,
  policy: TransportPolicy,
  samplesJson: string,
): string;
