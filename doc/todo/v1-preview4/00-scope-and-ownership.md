# 00 - Scope And Ownership

## Repository Boundary

- [x] Keep `nnrp-core` as the canonical Rust model for preview4 runtime control frames, runtime objects, cache references, trace context, and result drop reasons.
- [x] Keep `nnrp-runtime` as the host-facing client/server orchestration layer over transport providers.
- [x] Keep `nnrp-transport-provider` as the shared provider contract for TCP, QUIC, IPC, and WebSocket transports.
- [x] Keep each concrete transport in its own crate with real connection behavior and owned tests.
- [ ] Keep `nnrp-ffi` and `nnrp-wasm` as downstream integration surfaces, not as protocol owners.
- [ ] Keep `nnrp-conformance` consumption as the release gate for preview4 protocol behavior.

## Preview4 Baseline Inputs

- [x] Align Rust capability tokens with the preview4 control and object catalogs.
  - [x] Import the control capability list used by conformance.
  - [x] Import the object capability list used by conformance.
  - [ ] Keep capability names stable across Rust, FFI, WASM, Python, C#, and JavaScript.
- [x] Align transport names with conformance target manifests.
  - [x] Use `tcp`.
  - [x] Use `quic`.
  - [x] Use `ipc`.
  - [x] Use `websocket`.
- [x] Align result terminal states with wire result reports.
  - [x] Support `success`.
  - [x] Support `cancelled`.
  - [x] Support `dropped`.
  - [x] Support `error`.

## Parallel Work Ownership

- [x] Core protocol structs can proceed independently from transport crates.
- [x] IPC transport can proceed independently from WebSocket transport.
- [ ] FFI bindings consume the Rust core structs directly and remain independent from unfinished transport crates.
- [x] WASM bindings consume the shared frame codecs and object descriptor types from `nnrp-core`.
- [x] Wire conformance runner owns direct endpoint scenarios and can use reference endpoints without SDK adapters.
