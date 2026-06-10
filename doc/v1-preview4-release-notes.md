# NNRP/1 Preview4 Release Notes

Preview4 moves the Rust workspace beyond token-stream transport substitution and into runtime orchestration features that help SDKs model cancellation, priority, progress, partial results, cache references, route hints, trace context, result drop reasons, IPC, and WebSocket endpoints directly.

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

## Wire Conformance

`nnrp-conformance-wire` consumes preview4 suite and target manifests to produce direct wire-level dry-run results. The suite can target TCP, QUIC, IPC, and WebSocket endpoints without routing through an SDK adapter.
