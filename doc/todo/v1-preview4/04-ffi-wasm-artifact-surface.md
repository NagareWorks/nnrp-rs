# 04 - FFI And WASM Artifact Surface

## FFI Control Surface

- [ ] Add C ABI structs for preview4 control requests.
  - [ ] Cancel/abort.
  - [ ] Priority/deadline.
  - [ ] Progress/partial result.
  - [ ] Backpressure/credit.
  - [ ] Capability negotiation.
  - [ ] Route/execution hint.
  - [ ] Trace context.
  - [ ] Result drop reason.
- [ ] Add coarse C ABI calls for hot paths.
  - [ ] Submit with object references.
  - [ ] Submit with control metadata.
  - [ ] Poll batch events.
  - [ ] Complete partial result.
  - [ ] Drop stale result.
- [ ] Add C ABI error mapping for preview4 families.
- [ ] Add ABI version feature flags for runtime control and runtime object support.

## FFI Object Surface

- [ ] Add object descriptor handle types.
- [ ] Add cache reference descriptor handle types.
- [ ] Add native-owned metadata buffer release functions.
- [ ] Add borrowed view rules for object descriptors.
- [ ] Add copied snapshot fallback rules for SDKs that cannot preserve borrow lifetimes.
- [ ] Add tests for handle ownership and release order.

## WASM Surface

- [ ] Add TypeScript-visible runtime control structures.
- [ ] Add TypeScript-visible runtime object structures.
- [ ] Add WASM event polling batch calls.
- [ ] Add WASM helpers for WebSocket binary frame mapping.
- [ ] Keep browser APIs aligned with native role package semantics.
- [ ] Add wasm-bindgen tests for encode/decode and event batching.

## Artifact Packaging

- [ ] Package transport-scoped native libraries.
  - [ ] TCP.
  - [ ] QUIC.
  - [ ] IPC.
  - [ ] WebSocket.
- [ ] Package transport-scoped WASM outputs.
  - [ ] TCP frame/runtime primitives where applicable.
  - [ ] QUIC frame/runtime primitives where applicable.
  - [ ] IPC frame/runtime primitives for non-browser hosts where applicable.
  - [ ] WebSocket browser substrate helpers.
- [ ] Write manifest fields for transport name, protocol version, ABI version, and enabled features.
- [ ] Reject release artifacts that collapse all transport behavior into one hidden package.
