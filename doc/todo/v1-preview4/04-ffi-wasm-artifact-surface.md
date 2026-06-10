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
- [x] Add coarse C ABI calls for hot paths.
  - [x] Submit with object references.
  - [x] Submit with control metadata.
  - [x] Poll batch events.
  - [x] Complete partial result.
  - [x] Drop stale result.
- [x] Add C ABI error mapping for preview4 families.
- [x] Add ABI version feature flags for runtime control and runtime object support.

## FFI Object Surface

- [x] Add object descriptor handle types.
- [x] Add cache reference descriptor handle types.
- [x] Add native-owned metadata buffer release functions.
- [x] Add borrowed view rules for object descriptors.
- [x] Add copied snapshot rules for SDKs that cannot preserve borrow lifetimes.
- [x] Add tests for handle ownership and release order.

## WASM Surface

- [x] Add TypeScript-visible runtime control structures.
- [x] Add TypeScript-visible runtime object structures.
- [x] Add WASM event polling batch calls.
- [x] Add WASM helpers for browser WebSocket binary frame mapping.
- [x] Keep browser APIs aligned with native role package semantics.
- [x] Keep TCP, QUIC, and IPC transport implementations out of browser WASM packages.
- [x] Keep browser WASM output focused on Rust-owned framing, control, runtime-object, and WebSocket substrate helpers.
- [x] Add wasm-bindgen tests for encode/decode and event batching.

## Artifact Packaging

- [x] Package transport-scoped native libraries.
  - [x] TCP.
  - [x] QUIC.
  - [x] IPC.
  - [x] WebSocket.
- [x] Package browser-scoped WASM outputs.
  - [x] Runtime control frame codecs.
  - [x] Runtime object reference codecs.
  - [x] Browser WebSocket substrate helpers.
- [x] Write manifest fields for transport name, protocol version, ABI version, and enabled features.
- [x] Reject release artifacts that collapse all transport behavior into one hidden package.
