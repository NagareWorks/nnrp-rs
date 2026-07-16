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

## FFI Transport Reachability

- [x] Add transport connection, listener, and security-config handle kinds.
- [x] Add endpoint-bearing connect and listen requests for TCP, QUIC, IPC, and WebSocket artifacts.
- [x] Add listener endpoint snapshots for ephemeral ports and generated IPC endpoints.
- [x] Add coarse complete-packet batch read and write calls.
  - [x] Preserve the first failed write index in status detail.
  - [x] Preserve an unread packet when a read batch reaches its byte limit.
- [x] Add transport probe calls that require peer acknowledgements.
- [x] Add deterministic idempotent transport resource close behavior.
- [x] Run real Rust FFI loopbacks for TCP, QUIC, IPC, WS, and WSS.
- [x] Load each packaged host dynamic library and run a real packet-batch loopback through exported symbols.

## FFI Role Runtime Carrier Ownership

- [x] Replace logical-only role connections with carrier-backed runtime resources.
  - [x] Transfer a transport connection into `nnrp_client_connect` without exposing it after success.
  - [x] Transfer a transport listener into `nnrp_server_bind` without exposing it after success.
  - [x] Keep failed transfers caller-owned and close successful transfers through the role owner.
  - [x] Reject handles from another artifact or duplicate library instance.
- [x] Drive the canonical `nnrp-runtime` state machines from FFI role handles.
  - [x] Perform the client `SESSION_OPEN` / `SESSION_OPEN_ACK` exchange in `nnrp_client_open_session`.
  - [x] Accept a carrier connection and perform the server handshake in `nnrp_server_accept`.
  - [x] Remove caller-injected server session/profile/schema state.
- [ ] Route every role operation over the adopted carrier.
  - [x] Validate, split, and send `FRAME_SUBMIT` metadata/body with independent wire operation and frame identities in one coarse call.
  - [x] Decode inbound submit packets and bind both wire identities to opaque server operation handles.
  - [x] Validate, split, and send partial/terminal/drop/trace result packets.
  - [x] Validate, split, and send control/object/cache packets.
  - [x] Read and decode bounded client and server event batches.
  - [ ] Preserve operation ordering, pressure state, object/cache state, and owned payload release.
- [ ] Remove production use of local completion and event-injection helpers.
  - [x] Keep any synthetic loop helper explicitly benchmark-only.
  - [ ] Reject SDK or conformance paths that never read or write the selected carrier.
- [ ] Add same-library role/carrier E2E coverage.
  - [ ] Cover TCP, QUIC, IPC, WebSocket, and secure variants supported by each platform.
  - [x] Cover handshake, submit, partial result, terminal result, control, object/cache, and close.
  - [x] Assert successful adoption invalidates the packet-level transport handle.
  - [ ] Run the E2E through every packaged host dynamic library before release.

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
- [x] Write provider metadata required for deterministic downstream selection.
  - [x] Native TCP, QUIC, IPC, and WebSocket provider identities and platform limitations.
  - [x] Browser WASM WebSocket provider identity and browser limitation.
  - [x] Canonical decimal cost and frame-limit values.
- [x] Export structured WASM probe metrics and candidate diagnostics without weighted scores.
- [x] Reject release artifacts that collapse all transport behavior into one hidden package.
- [x] Reject transport artifacts whose exported ABI cannot establish and use their declared transport.
