# 04 - FFI And WASM Artifact Surface

## FFI Control Surface

- [x] Add C ABI structs for preview4 control requests.
  - [x] Cancel/abort.
  - [x] Priority/deadline.
  - [x] Progress/partial result.
  - [x] Backpressure/credit.
  - [x] Capability negotiation.
  - [x] Route/execution hint.
  - [x] Trace context.
  - [x] Result drop reason.
- [ ] Add coarse C ABI calls for hot paths.
  - [ ] Submit with object references.
  - [ ] Submit with control metadata.
  - [ ] Poll batch events.
  - [ ] Complete partial result.
  - [ ] Drop stale result.
- [ ] Add C ABI error mapping for preview4 families.
- [x] Add ABI version feature flags for runtime control and runtime object support.

## FFI Object Surface

- [ ] Add object descriptor handle types.
- [ ] Add cache reference descriptor handle types.
- [x] Add native-owned metadata buffer release functions.
- [ ] Add borrowed view rules for object descriptors.
- [ ] Add copied snapshot rules for SDKs that cannot preserve borrow lifetimes.
- [ ] Add tests for handle ownership and release order.

## WASM Surface

- [ ] Add TypeScript-visible runtime control structures.
- [ ] Add TypeScript-visible runtime object structures.
- [ ] Add WASM event polling batch calls.
- [x] Add WASM helpers for browser WebSocket binary frame mapping.
- [x] Keep browser APIs aligned with native role package semantics.
- [x] Keep TCP, QUIC, and IPC transport implementations out of browser WASM packages.
- [ ] Keep browser WASM output focused on Rust-owned framing, control, runtime-object, and WebSocket substrate helpers.
- [ ] Add wasm-bindgen tests for encode/decode and event batching.

## Artifact Packaging

- [x] Package transport-scoped native libraries.
  - [x] TCP.
  - [x] QUIC.
  - [x] IPC.
  - [x] WebSocket.
- [ ] Package browser-scoped WASM outputs.
  - [ ] Runtime control frame codecs.
  - [ ] Runtime object reference codecs.
  - [x] Browser WebSocket substrate helpers.
- [x] Write manifest fields for transport name, protocol version, ABI version, and enabled features.
- [ ] Reject release artifacts that collapse all transport behavior into one hidden package.
