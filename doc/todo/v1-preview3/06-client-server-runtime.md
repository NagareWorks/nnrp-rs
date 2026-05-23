# Rust Preview3 Client And Server Runtime

## Scope

- [x] Keep this shard responsible for the usable Rust client/server SDK surface, transport runtime, and runtime-backed FFI entrypoints.
- [x] Treat `nnrp-core` as the protocol and state-machine source of truth; runtime APIs must consume core semantics rather than redefining protocol behavior.
- [ ] Treat `nnrp-ffi` as the downstream ABI boundary; FFI functions may expose runtime handles only after the Rust runtime surface exists.

## Transport Abstraction

- [x] Define a host-neutral transport trait for framed async read/write, connection close, and backpressure signaling.
- [x] Implement TCP transport support for local integration tests.
- [x] Add QUIC transport hooks without freezing TLS/provider policy in `nnrp-core`.
- [x] Implement the connection pump that reads/writes `CommonHeader` packets and dispatches metadata/body regions.

## Client API

- [x] Implement `NnrpClientConfig` with protocol/schema defaults and flow-control defaults.
- [x] Add client cache hints and explicit runtime transport selection.
- [x] Implement `NnrpClient::connect_tcp` and `NnrpClient::connect_quic` over the transport abstraction.
- [x] Implement `NnrpClient::open_session` and `NnrpClientSession` lifecycle ownership.
- [x] Implement submit, submit-nowait, await-result, and session close APIs.
- [x] Implement cancel, session patch, and result/drop/flow event stream APIs.
- [x] Implement client-side resume consumption using `nnrp-core` recovery semantics.
- [x] Implement transport migration consumption using `nnrp-core` recovery semantics.

## Server API

- [x] Implement `NnrpServerConfig` with flow-control defaults and session lease windows.
- [x] Add server capability advertisement, cache limits, and schema/profile registry inputs.
- [x] Implement TCP bind/listen/accept and QUIC bind hooks.
- [x] Implement `NnrpServerSession` with receive-submit, send-result, and close ack APIs.
- [x] Implement `NnrpServerSession` send-result-drop, send-flow-update, and patch ack APIs.
- [x] Implement server-side operation registry and cache/schema validation.
- [x] Implement server-side session registry and recovery token handling.
- [x] Keep authentication and application policy pluggable rather than built into the protocol layer.

## FFI Runtime Binding

- [x] Replace preview3 FFI placeholder/bootstrap entrypoints with runtime-backed handles.
- [ ] Expose client connect/open/submit/await/cancel/close through stable C ABI.
- [ ] Expose server bind/accept/receive-submit/send-result/send-flow-update/close through stable C ABI.
- [ ] Preserve existing value-handle, buffer-view, callback, polling, and error-family rules.

## Conformance And Validation

- [x] Add loopback client/server integration tests over TCP.
- [x] Add loopback tests for submit/result and session close.
- [ ] Add fixture-driven tests for flow update, cancellation, cache miss, schema mismatch, and resume.
- [ ] Add FFI smoke tests that drive the real runtime rather than only validating ABI shape.
- [ ] Export runtime-backed conformance cases from `nnrp-conformance`.
