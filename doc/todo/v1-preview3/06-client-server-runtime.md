# Rust Preview3 Client And Server Runtime

## Scope

- [ ] Keep this shard responsible for the usable Rust client/server SDK surface, transport runtime, and runtime-backed FFI entrypoints.
- [ ] Treat `nnrp-core` as the protocol and state-machine source of truth; runtime APIs must consume core semantics rather than redefining protocol behavior.
- [ ] Treat `nnrp-ffi` as the downstream ABI boundary; FFI functions may expose runtime handles only after the Rust runtime surface exists.

## Transport Abstraction

- [ ] Define a host-neutral transport trait for framed async read/write, connection close, and backpressure signaling.
- [ ] Implement TCP transport support for local integration tests.
- [ ] Add QUIC transport hooks without freezing TLS/provider policy in `nnrp-core`.
- [ ] Implement the connection pump that reads/writes `CommonHeader` packets and dispatches metadata/body regions.

## Client API

- [ ] Implement `NnrpClientConfig` with protocol capability defaults, cache hints, flow-control defaults, and transport selection.
- [ ] Implement `NnrpClient::connect_tcp` and `NnrpClient::connect_quic` over the transport abstraction.
- [ ] Implement `NnrpClient::open_session` and `NnrpClientSession` lifecycle ownership.
- [ ] Implement submit, submit-nowait, await-result, cancel, session patch, session close, and result/event stream APIs.
- [ ] Implement client-side resume and transport migration consumption using `nnrp-core` recovery semantics.

## Server API

- [ ] Implement `NnrpServerConfig` with capability advertisement, cache limits, schema/profile registry, and flow-control defaults.
- [ ] Implement TCP bind/listen/accept and QUIC bind hooks.
- [ ] Implement `NnrpServerSession` with receive-submit, send-result, send-result-drop, send-flow-update, patch ack, and close APIs.
- [ ] Implement server-side session registry, operation registry, cache/schema validation, and recovery token handling.
- [ ] Keep authentication and application policy pluggable rather than built into the protocol layer.

## FFI Runtime Binding

- [ ] Replace preview3 FFI placeholder/bootstrap entrypoints with runtime-backed handles.
- [ ] Expose client connect/open/submit/await/cancel/close through stable C ABI.
- [ ] Expose server bind/accept/receive-submit/send-result/send-flow-update/close through stable C ABI.
- [ ] Preserve existing value-handle, buffer-view, callback, polling, and error-family rules.

## Conformance And Validation

- [ ] Add loopback client/server integration tests over TCP.
- [ ] Add fixture-driven tests for submit/result, flow update, cancellation, session close, cache miss, schema mismatch, and resume.
- [ ] Add FFI smoke tests that drive the real runtime rather than only validating ABI shape.
- [ ] Export runtime-backed conformance cases from `nnrp-conformance`.
