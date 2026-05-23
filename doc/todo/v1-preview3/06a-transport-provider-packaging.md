# Rust Preview3 Transport Providers And Packaging

## Scope

- [x] Keep `nnrp-runtime` as the transport-neutral session runtime and keep concrete TCP/QUIC providers outside the core runtime crate.
- [x] Split provider crates so downstream users can opt into TCP, QUIC, native dynamic loading, or WASM without pulling unused dependencies.
- [ ] Treat JavaScript/TypeScript as a first-class downstream target: Node may use native libraries or WASM, while browsers must use WASM plus WebSocket/WebTransport bindings.

## Provider Crates

- [x] Add `nnrp-transport-tcp` as the built-in TCP provider package over `FramedTransport` / `FramedListener`.
- [x] Add `nnrp-transport-quic` as the QUIC provider package without freezing a single TLS/QUIC backend into `nnrp-core`.
- [x] Add provider feature flags so applications can choose `tcp`, `quic`, `native-loader`, and future provider families explicitly.
- [x] Keep provider crate public APIs aligned with the runtime slot contract rather than duplicating session semantics.

## Local Provider Discovery

- [x] Define a provider registry that reports installed transports, provider version, transport id, and whether the provider is native, pure Rust, or WASM-facing.
- [x] Detect native dynamic libraries (`.dll`, `.so`, `.dylib`) by explicit path, environment variable, and conventional package layout.
- [x] Report missing provider dependencies as structured diagnostics instead of collapsing them into generic connection failure.
- [x] Keep `force_*` policies fail-fast when the requested local provider is absent.

## Remote Capability And Probe Selection

- [x] Resolve local provider availability against user policy: `auto`, `prefer_quic`, `prefer_tcp`, `force_quic`, `force_tcp`.
- [x] Intersect local provider availability with remote transport support learned from manifest, endpoint metadata, or probe/hello ack.
- [x] Probe all viable candidate bindings when both TCP and QUIC are available and policy allows both.
- [x] Score probe results using latency, timeout/failure rate, and effective throughput rather than choosing the first successful path.
- [x] Expose the selected transport and rejected candidates for debug/telemetry.
- [ ] Feed the selected transport into `SESSION_OPEN` and preserve migration/fallback through `SESSION_MIGRATE`.

## Native Link Library Packaging

- [x] Configure `nnrp-ffi` to build `rlib` and `cdylib` outputs.
- [ ] Add release packaging scripts for Windows DLL, Linux SO, and macOS dylib artifacts.
- [ ] Add header generation or cbindgen output for C ABI consumers.
- [ ] Add CI jobs that verify platform-specific native library names and exported symbols.
- [ ] Add Node.js native-loading guidance for backend JS/TS users.

## WASM And JS/TS Packaging

- [ ] Add a dedicated WASM-facing crate or feature surface instead of exposing the raw C ABI as the browser SDK.
- [ ] Add `wasm-bindgen` bindings for JS/TS session APIs.
- [ ] Add browser transport adapters for WebSocket and WebTransport.
- [ ] Add Node WASM packaging for environments that do not want native addons.
- [ ] Generate `.d.ts` TypeScript declarations as part of the WASM package.
- [ ] Add npm package layout and examples for both browser and Node consumers.
- [ ] Keep browser documentation clear that native link libraries are not usable there; browsers consume WASM plus web transports.

## Validation

- [x] Add provider registry unit tests for present/missing native library cases.
- [x] Add transport policy resolver tests for every local/remote capability combination.
- [x] Add probe selection tests that cover success, timeout, downgrade, and force-policy failure.
- [ ] Add native artifact build checks in release CI.
- [ ] Add WASM build checks and a minimal JS/TS smoke test.
