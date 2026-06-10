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

export interface WebSocketFrameBatchEntry {
  frame_offset: number;
  frame_len: number;
  header: WebSocketFrameHeaderOutput;
  metadata_offset: number;
  metadata_len: number;
  body_offset: number;
  body_len: number;
}

export interface WebSocketFrameBatchOutput {
  frames: WebSocketFrameBatchEntry[];
  consumed_len: number;
  remaining_len: number;
}

export interface DecodedMetadata<T> {
  metadata: T;
  tail_offset: number;
  tail_len: number;
}

export interface ControlRequestMetadata {
  operation_id: number;
  control_sequence: number;
  reason_code: number;
  source_role: number;
  flags: number;
  diagnostic_bytes: number;
}

export interface SchedulingMetadata {
  operation_id: number;
  control_sequence: number;
  priority_class: number;
  priority_delta: number;
  deadline_unix_ms: number;
  flags: number;
}

export interface SupersedeMetadata {
  old_operation_id: number;
  new_operation_id: number;
  control_sequence: number;
  drop_reason_code: number;
  flags: number;
  diagnostic_bytes: number;
}

export interface BudgetMetadata {
  operation_id: number;
  compute_budget_units: number;
  memory_budget_bytes: number;
  bandwidth_budget_bytes: number;
  token_budget: number;
  flags: number;
}

export interface ProgressMetadata {
  operation_id: number;
  progress_sequence: number;
  stage_code: number;
  percent_x100: number;
  object_id: number;
  body_bytes: number;
}

export interface PartialResultMetadata {
  operation_id: number;
  result_sequence: number;
  object_id: number;
  delta_sequence: number;
  body_bytes: number;
  flags: number;
}

export interface PressureMetadata {
  scope_id: number;
  credit_window: number;
  pressure_level: number;
  pressure_reason: number;
  retry_after_ms: number;
  flags: number;
}

export interface CapabilityMetadata {
  profile_id: number;
  capability_count: number;
  cost_model_id: number;
  preference_rank: number;
  limit_bytes: number;
  limit_units: number;
  body_bytes: number;
  flags: number;
}

export interface RouteHintMetadata {
  operation_id: number;
  route_id: number;
  executor_class: number;
  affinity_class: number;
  deadline_unix_ms: number;
  body_bytes: number;
  flags: number;
}

export interface TraceContextMetadata {
  trace_id: number;
  span_id: number;
  parent_span_id: number;
  stage_code: number;
  flags: number;
  body_bytes: number;
}

export interface ResultDropReasonMetadata {
  operation_id: number;
  result_sequence: number;
  drop_reason_code: number;
  source_role: number;
  flags: number;
  diagnostic_bytes: number;
}

export interface RecoverableErrorMetadata {
  error_code: number;
  error_scope: number;
  recovery_action: number;
  source_role: number;
  flags: number;
  retry_after_ms: number;
  related_session_id: number;
  related_frame_id: number;
  related_view_id: number;
  diagnostic_bytes: number;
}

export interface RetryAfterMetadata {
  scope_id: number;
  control_sequence: number;
  retry_after_ms: number;
  jitter_ms: number;
  reason_code: number;
  source_role: number;
  flags: number;
  diagnostic_bytes: number;
}

export interface ObjectDescriptorMetadata {
  object_id: number;
  object_kind: number;
  producer_role: number;
  consumer_role: number;
  session_id: number;
  byte_size: number;
  compute_cost_units: number;
  memory_location_hint: number;
  ownership_hint: number;
  lifetime_hint_ms: number;
  metadata_bytes: number;
}

export interface ObjectReferenceMetadata {
  object_id: number;
  operation_id: number;
  object_version: number;
  offset: number;
  length: number;
  flags: number;
  metadata_bytes: number;
}

export interface ObjectReleaseMetadata {
  object_id: number;
  operation_id: number;
  release_reason: number;
  source_role: number;
  flags: number;
  diagnostic_bytes: number;
}

export interface ObjectDeltaMetadata {
  object_id: number;
  delta_sequence: number;
  region_offset: number;
  region_bytes: number;
  delta_bytes: number;
  flags: number;
  metadata_bytes: number;
}

export interface CacheReferenceMetadata {
  cache_key_hi: number;
  cache_key_lo: number;
  profile_id: number;
  reuse_scope: number;
  lease_id: number;
  producer_trace_id: number;
  expiration_hint_ms: number;
  metadata_bytes: number;
  flags: number;
}

export interface CacheMissMetadata {
  cache_key_hi: number;
  cache_key_lo: number;
  miss_reason: number;
  profile_id: number;
  diagnostic_bytes: number;
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

export function decodeWebSocketBinaryFrameBatchJson(
  frames: Uint8Array,
  maxFrames: number,
): string;

export function encodeRuntimeControlMetadataJson(
  messageType: number,
  metadataJson: string,
  tail: Uint8Array,
): Uint8Array;

export function decodeRuntimeControlMetadataJson(
  messageType: number,
  metadata: Uint8Array,
): string;

export function encodeRuntimeObjectMetadataJson(
  messageType: number,
  metadataJson: string,
  tail: Uint8Array,
): Uint8Array;

export function decodeRuntimeObjectMetadataJson(
  messageType: number,
  metadata: Uint8Array,
): string;
