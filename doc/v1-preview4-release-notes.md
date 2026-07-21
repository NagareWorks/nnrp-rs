# NNRP/1 Preview4 Release Notes

Preview4 moves the Rust workspace beyond token-stream transport substitution and into runtime orchestration features that help SDKs model cancellation, priority, progress, partial results, cache references, route hints, trace context, result drop reasons, IPC, and WebSocket endpoints directly.

## 1.0.0-preview.4.9

Transport-scoped native artifact manifests now declare both role connection-close entry points exported by the ABI 3
library. Downstream SDKs can validate and bind complete client and server connection lifecycles without rejecting the
official TCP, QUIC, IPC, or WebSocket artifact as incomplete.

Native runtime capability records now derive their SDK components from the Cargo package version at compile time, so
artifact probes, public headers, and crate metadata report the same Preview4 revision.

## 1.0.0-preview.4.8

Closing a native client or server session now releases every operation and cache lease owned by that session before the
session handle is removed. Applications can therefore reopen the same protocol session identity and reuse operation
identifiers without colliding with stale FFI resources left by an earlier session lifetime.

## 1.0.0-preview.4.7

TCP and QUIC now retain partially received packet bytes when a bounded role-event poll times out. Repeated short polls
therefore resume the same frame instead of restarting at the next byte and corrupting stream alignment. The public FFI
also projects `PARTIAL_RESULT` through the session-scoped runtime-frame event surface in both directions, matching the
frozen Preview4 SDK contract.

## 1.0.0-preview.4.6

The `nnrp-conformance` crate now drives declared external wire targets through real TCP, IPC, QUIC, WebSocket, and
secure WebSocket endpoints. Its typed client, server, and proxy roles exercise the frozen Preview4 runtime-control
flows without fabricating observed frames inside the suite process.

## 1.0.0-preview.4.5

The native FFI ABI is `3.0.0`. Cache identities now use the frozen `(cache_namespace: u32, cache_key_hi: u64,
cache_key_lo: u64)` representation across protocol metadata, native FFI, WASM, conformance vectors, and downstream SDK
bindings. `ObjectReferenceBlock`, `CACHE_PUT`, `CACHE_ACK`, `CACHE_INVALIDATE`, `CACHE_REFERENCE`, and `CACHE_MISS`
therefore have one canonical little-endian wire layout, including the frozen invalidation-scope zeroing rules.

WASM JSON surfaces encode every runtime-control and runtime-object `u64` as an unsigned decimal JSON string and reject
JSON numbers, preserving all 64 bits when JavaScript maps the values to `bigint`.

Native FFI resource handles now use monotonic per-kind identities. Releasing a resource cannot make a stale handle
valid again when another thread allocates a resource with the same generation.

Client sessions reject operation identifier reuse for their full lifetime. Server runtime-event correlation maintains
both frame-to-operation and operation-to-frame indexes, avoiding linear scans on progress and partial-result hot paths.
Both peers reject operation-scoped events whose header frame identity does not match the operation identity in fixed
metadata; client control and scheduling senders populate the correlated frame identity on the wire.

Production transport artifacts no longer export client completion, drop, or submit/result helpers that synthesized
terminal events without reading a peer result from the selected carrier. The retired feature-flag bits remain reserved
and are not reused.

Raw `nnrp_control`, `nnrp_client_submit_control`, and `nnrp_client_send_result_hint` event-injection exports are also
removed. Inherited `RESULT_HINT` is sent by a server and decoded by a client through the same carrier-backed
`nnrp_runtime_frame_send` and role event path as typed Preview4 control frames.

Synthetic FFI loops are available only from an explicit `benchmark-ffi` build under the
`nnrp_benchmark_*` namespace. Their declarations live in `benchmarks/include/nnrp/nnrp_ffi_benchmark.h`; the
production package builder rejects both the retired symbols and benchmark-only symbols.

```bash
cargo build -p nnrp-ffi --release --no-default-features --features transport-tcp,benchmark-ffi
```

## 1.0.0-preview.4.4

This revision makes every transport-scoped native artifact reachable through the frozen coarse transport FFI. TCP,
QUIC, IPC, and WebSocket libraries now own their actual connect, listen, accept, probe, and packet-batch paths instead
of merely advertising provider metadata. The ABI adds typed connection, listener, and security handles, endpoint
snapshots, native-owned read batches, and deterministic close semantics.

Native artifact validation now loads each produced dynamic library and completes a real two-frame NNRP packet
loopback through its exported ABI. This is in addition to Rust-level TCP, QUIC, IPC, WS, and WSS tests, so symbol-only
or configuration-only transport artifacts cannot pass the release gate.

## 1.0.0-preview.4.3

This revision freezes provider metadata and deterministic selection as a first-class Rust SDK API. Native and browser
WASM manifests expose the same cost, preference, frame-limit, and limitation fields consumed by downstream SDKs.
Provider selection returns ordered candidate diagnostics and structured probe metrics without weighted scores.

## 1.0.0-preview.4.2

This revision fixes native FFI validation for `OBJECT_PATCH` and `OBJECT_DELTA` frames that carry both extension metadata and delta payload bytes. The coarse `nnrp_runtime_frame_send` ABI remains at `1.12.0`; only validation of the already-frozen payload layout changes.

## Runtime Control

Preview4 adds protocol-level control frames for:

- cancellation and abort flows
- priority and deadline updates
- progress and partial-result reporting
- backpressure and credit updates
- capability negotiation with cost, preference, and limit metadata
- route and execution hints
- cache references, misses, and invalidation
- trace context and result drop reasons

These control frames are Rust-owned protocol structures in `nnrp-core` and are exposed through native FFI and WASM surfaces with coarse calls for downstream SDK hot paths.

## Runtime Objects And Cache References

Runtime objects make large or reusable payloads first-class protocol values instead of forcing every higher-level SDK to encode everything as JSON tokens. Cache references carry explicit object identity, object kind, producer, lifetime, and release behavior so clients and servers can reason about reuse without inventing SDK-specific side channels.

## IPC And WebSocket

Preview4 adds:

- `nnrp-transport-ipc` for same-node schedulers, local agents, and runtime sidecars
- `nnrp-transport-websocket` for browser-compatible edge paths and WebSocket service endpoints

TCP, QUIC, IPC, and WebSocket stay in separate transport crates with owned connection behavior and owned tests.

## Native And WASM Artifacts

Native artifacts are transport-scoped. Release packages emit one native package per transport scope: `tcp`, `quic`, `ipc`, and `websocket`. Browser WASM emits `nnrp-wasm-browser` and only declares the browser WebSocket substrate.

Release CI inspects native and WASM manifests and rejects artifacts that collapse transport ownership boundaries.
Host-native CI also loads each transport library and verifies a real packet-batch loopback through the exported C ABI.

Provider selection exposes frozen cost, preference, limit, limitation, probe, rank, and rejection diagnostics. Probe
samples bind to stable provider ids and use deterministic per-sample throughput and median aggregation. Native and
browser WASM manifests carry the same provider metadata, and Rust/WASM public APIs do not expose weighted scores.

## Wire Conformance

`nnrp-conformance-wire` consumes preview4 suite and target manifests to produce direct wire-level dry-run results. The suite can target TCP, QUIC, IPC, and WebSocket endpoints without routing through an SDK adapter.

## Benchmarks

Rust owns a repeatable Preview4 benchmark entrypoint:

```bash
cargo run -p nnrp-conformance --bin nnrp-preview4-benchmarks -- --iterations 100000 --transport-iterations 1000
```

The JSON report includes control-frame hot-path encode/decode, runtime object declare/ref/release metadata, IPC loopback, and WebSocket loopback cases. Downstream SDK benchmark comparisons should use this Rust report as the artifact-set baseline for Python cffi and JavaScript native/WASM hot paths. Synthetic host-FFI measurements must load an explicit `benchmark-ffi` build and must not be reported as live carrier results.
