# Rust Preview4 Implementation Todo

Preview4 moves NNRP from token-stream transport substitution toward runtime object efficiency, explicit control frames, and wire-level interoperability. Rust remains the canonical protocol, transport, FFI, and WASM source for downstream SDKs.

## Workstreams

- [ ] [00 - Scope and ownership](00-scope-and-ownership.md)
- [x] [01 - Runtime control protocol](01-runtime-control-protocol.md)
- [x] [02 - Runtime object and cache reference](02-runtime-object-cache-reference.md)
- [x] [03 - IPC and WebSocket transports](03-ipc-websocket-transports.md)
- [x] [04 - FFI and WASM artifact surface](04-ffi-wasm-artifact-surface.md)
- [x] [05 - Wire conformance runner](05-wire-conformance-runner.md)
- [ ] [06 - Release validation and docs](06-release-validation-and-docs.md)

## Coordination Rules

- [x] Keep protocol object definitions in `nnrp-core` owned by the control/object workstreams.
- [x] Keep transport behavior inside transport crates; do not turn transport crates into feature flags over hidden implementation elsewhere.
- [x] Keep FFI and WASM calls coarse enough for SDK hot paths.
- [x] Keep conformance runner work independent from SDK adapters until live endpoints are available.
- [x] Update this index whenever a workstream is split or completed.
