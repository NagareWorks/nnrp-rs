export type TransportPolicy =
  | "auto"
  | "prefer_quic"
  | "prefer_tcp"
  | "prefer_ipc"
  | "prefer_websocket"
  | "force_quic"
  | "force_tcp"
  | "force_ipc"
  | "force_websocket";

export type TransportId = 1 | 2 | 3 | 4;

export type ProviderKind = "pure_rust" | "native_dynamic" | "wasm";

export interface TransportProviderInput {
  name: string;
  version: string;
  transport_id: TransportId;
  kind?: ProviderKind;
  available?: boolean;
  diagnostic?: string;
}

export interface ProbeSampleInput {
  transport_id: TransportId;
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
  rejected: Array<{ transport_id: TransportId; provider_name?: string; reason: string }>;
}

export interface WebSocketFrameHeaderInput {
  message_type: number;
  flags?: number;
  session_id?: number;
  frame_id?: number;
  view_id?: number;
  route_id?: number;
  trace_id?: number;
}

export interface WebSocketFrameHeaderOutput {
  version_major: number;
  wire_format: number;
  message_type: number;
  header_len: number;
  flags: number;
  meta_len: number;
  body_len: number;
  session_id: number;
  frame_id: number;
  view_id: number;
  route_id: number;
  trace_id: number;
}

export interface WebSocketFrameOutput {
  header: WebSocketFrameHeaderOutput;
  metadata_offset: number;
  metadata_len: number;
  body_offset: number;
  body_len: number;
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

export function encodeWebSocketBinaryFrameJson(
  headerJson: string,
  metadata: Uint8Array,
  body: Uint8Array,
): Uint8Array;

export function decodeWebSocketBinaryFrameJson(frame: Uint8Array): string;
