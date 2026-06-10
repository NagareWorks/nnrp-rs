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
- Keep native calls coarse around session, control, object, progress, result, and release hot paths.
- Keep SDKs pinned to Preview3 release assets until their Preview4 transport package split and benchmark gates are complete.

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
- Treat skipped wire cases as explicit capability or transport gaps, not as passes.
