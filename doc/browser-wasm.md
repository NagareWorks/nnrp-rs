# Browser WASM Packaging

`nnrp-wasm` packages browser-facing primitives for JavaScript and TypeScript SDKs. Browser artifacts are not native transport bundles: they do not contain TCP, QUIC, IPC, `.dll`, `.so`, `.dylib`, or static-library outputs.

## Package Shape

Build the browser artifact with:

```bash
rustup target add wasm32-unknown-unknown
python scripts/package_wasm_primitives.py --out artifacts/wasm
```

The script writes `artifacts/wasm/nnrp-wasm-browser` with:

- `nnrp_wasm.wasm`
- `nnrp_wasm.d.ts`
- `manifest.json`

The manifest declares:

- `package`: `nnrp-wasm`
- `artifact`: `nnrp-wasm-browser`
- `transport_scope`: `browser`
- `transport_slots`: `["websocket"]`
- `enabled_features`: `["transport-websocket", "wasm-provider"]`

## Runtime Boundary

The browser package owns browser-safe NNRP primitives and WebSocket-substrate frame helpers. It does not smuggle native TCP, QUIC, or IPC implementations into browser builds. Backend JavaScript packages should use transport-scoped native artifacts when native loading is available.

## Release Checks

Release and CI validation run:

```bash
python scripts/inspect_release_artifacts.py --wasm-dir artifacts/wasm
```

The inspection step rejects WASM artifacts whose package, artifact name, transport scope, transport slots, or enabled feature list no longer match the browser boundary.
