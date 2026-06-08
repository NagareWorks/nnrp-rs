# 00 - Scope And Ownership

## Repository Boundary

- [ ] Keep `nnrp-core` as the canonical Rust model for preview4 runtime control frames, runtime objects, cache references, trace context, and result drop reasons.
- [ ] Keep `nnrp-runtime` as the host-facing client/server orchestration layer over transport providers.
- [ ] Keep `nnrp-transport-provider` as the shared provider contract for TCP, QUIC, IPC, and WebSocket transports.
- [ ] Keep each concrete transport in its own crate with real connection behavior and owned tests.
- [ ] Keep `nnrp-ffi` and `nnrp-wasm` as downstream integration surfaces, not as protocol owners.
- [ ] Keep `nnrp-conformance` consumption as the release gate for preview4 protocol behavior.

## Preview4 Baseline Inputs

- [ ] Align Rust capability tokens with the preview4 control and object catalogs.
  - [ ] Import the control capability list used by conformance.
  - [ ] Import the object capability list used by conformance.
  - [ ] Keep capability names stable across Rust, FFI, WASM, Python, C#, and JavaScript.
- [ ] Align transport names with conformance target manifests.
  - [ ] Use `tcp`.
  - [ ] Use `quic`.
  - [ ] Use `ipc`.
  - [ ] Use `websocket`.
- [ ] Align result terminal states with wire result reports.
  - [ ] Support `success`.
  - [ ] Support `cancelled`.
  - [ ] Support `dropped`.
  - [ ] Support `error`.

## Parallel Work Ownership

- [ ] Core protocol structs can proceed independently from transport crates.
- [ ] IPC transport can proceed independently from WebSocket transport.
- [ ] FFI bindings consume the Rust core structs directly and remain independent from unfinished transport crates.
- [ ] WASM bindings consume the shared frame codecs and object descriptor types from `nnrp-core`.
- [ ] Wire conformance runner owns direct endpoint scenarios and can use reference endpoints without SDK adapters.
