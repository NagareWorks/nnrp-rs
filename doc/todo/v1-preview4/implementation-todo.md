# Rust Preview4 Implementation Todo

Preview4 moves NNRP from token-stream transport substitution toward runtime object efficiency, explicit control frames, and wire-level interoperability. Rust remains the canonical protocol, transport, FFI, and WASM source for downstream SDKs.

## Workstreams

- [ ] [00 - Scope and ownership](00-scope-and-ownership.md)
- [ ] [01 - Runtime control protocol](01-runtime-control-protocol.md)
- [ ] [02 - Runtime object and cache reference](02-runtime-object-cache-reference.md)
- [ ] [03 - IPC and WebSocket transports](03-ipc-websocket-transports.md)
- [ ] [04 - FFI and WASM artifact surface](04-ffi-wasm-artifact-surface.md)
- [ ] [05 - Wire conformance runner](05-wire-conformance-runner.md)
- [ ] [06 - Release validation and docs](06-release-validation-and-docs.md)

## Coordination Rules

- [ ] Keep protocol object definitions in `nnrp-core` owned by the control/object workstreams.
- [ ] Keep transport behavior inside transport crates; do not turn transport crates into feature flags over hidden implementation elsewhere.
- [ ] Keep FFI and WASM calls coarse enough for SDK hot paths.
- [ ] Keep conformance runner work independent from SDK adapters until live endpoints are available.
- [ ] Update this index whenever a workstream is split or completed.
