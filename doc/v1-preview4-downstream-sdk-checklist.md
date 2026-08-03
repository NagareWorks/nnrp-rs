# Preview4 Downstream SDK Checklist

Downstream SDKs should consume Preview4 as a protocol and artifact boundary update, not as a configuration-only feature toggle.

## Package Boundaries

- Keep client and server packages focused on their public API roles.
- Keep transport packages responsible for their own transport behavior or native/WASM artifacts.
- Do not make transport packages only flip configuration over a hidden all-transports runtime.
- Use `tcp`, `quic`, `ipc`, and `websocket` as the transport names in package manifests and probe metadata.

## Native Artifacts

- Download or bundle the transport-scoped native package matching the SDK transport package.
- Validate `manifest.json` before loading a native library.
- Reject artifacts whose `transport_scope` does not match the SDK package.
- Reject artifacts whose `transport_slots` contains transports outside the package boundary.
- Require FFI ABI `4.4.0` and Rust artifact revision `1.0.0-preview.4.22` for provider-route orchestration, exact transport/provider identity selection, route-local security, asynchronous server admission, negotiated session identity, persistent server accept tickets, complete cache-lease and schema-registry state, cancellation-safe bounded event polling, complete wire-header preservation, the closed runtime-event metadata/tail envelope, reachable transport handles, packet batches, the full-duplex browser client role runtime, cancellation-safe browser ingress, explicit native transport runtime shutdown, and the release-valid explicit benchmark ABI used by downstream performance gates.
- Bind transport connect, listen, accept, endpoint, probe, batch read/write, security-config, and close exports directly.
- Keep native calls coarse around session, control, object, progress, result, and release hot paths.
- Keep complete NNRP packets as the transport FFI unit; do not introduce per-socket-chunk cross-language calls.
- Project `NnrpRuntimeEvent` losslessly from native event batches; do not reconstruct missing header fields or expose private role-decoder enums.
- Keep native events with `header.present == 0` in a separate lifecycle-event type; never manufacture a wire header for them.
- Reject duplicate transport IDs and duplicate provider IDs during provider registration.
- Supply exactly one route/security readiness record for every registered provider before selection.
- Match readiness and aggregate probe observations by the `(transport_id, provider_id)` identity pair; reject duplicate, unmatched, incomplete, or structurally invalid evidence before producing candidate diagnostics.
- Keep raw probe samples below the SDK selection boundary and pass only succeeded/failed aggregate observations into deterministic selection.

## Browser WASM

- Use `nnrp-wasm-browser` only for browser-safe primitives.
- Treat browser transport as WebSocket-substrate unless a later browser transport artifact declares otherwise.
- Do not bundle native TCP, QUIC, or IPC libraries into browser packages.

## Protocol Surface

- Expose Preview4 control frames with stable names matching the Rust/conformance capability catalog.
- Expose runtime object descriptors and cache references without forcing token-profile JSON parsing in hot paths.
- Preserve result terminal states: `success`, `cancelled`, `dropped`, and `error`.

## Conformance

- Keep SDK adapter conformance for SDK public API behavior.
- Add wire conformance target manifests for direct endpoint behavior.
- Run the typed external client, server, and proxy roles from `nnrp-conformance`; do not synthesize target observations in the suite process.
- Treat skipped wire cases as explicit capability or transport gaps, not as passes.
- Load the packaged dynamic library in SDK CI and run at least one real loopback through the SDK transport package.
