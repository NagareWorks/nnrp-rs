# Rust Preview3 Transport Providers And Packaging

## Scope

- [x] Keep `nnrp-runtime` as the transport-neutral session runtime and keep concrete TCP/QUIC providers outside the core runtime crate.
- [x] Split provider crates so downstream users can opt into TCP, QUIC, native dynamic loading, or WASM without pulling unused dependencies.
- [x] Treat JavaScript/TypeScript as a first-class downstream target without implementing the full JS SDK in this repository: `nnrp-rs` emits native/WASM primitives, while `nnrp-js` owns npm packaging, runtime selection, and browser/Node adapters.

## Provider Crates

- [x] Add `nnrp-transport-tcp` as the built-in TCP provider package over `FramedTransport` / `FramedListener`.
- [x] Add `nnrp-transport-quic` as the default Quinn/Rustls QUIC provider package while keeping TLS/QUIC backend selection outside `nnrp-core`.
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
- [x] Feed the selected transport into `SESSION_OPEN` and preserve migration/fallback through `SESSION_MIGRATE`.

## Native Link Library Packaging

- [x] Configure `nnrp-ffi` to build `rlib`, `cdylib`, and `staticlib` outputs.
- [x] Add release packaging scripts for Windows DLL, Linux SO, macOS dylib, Android SO, and iOS static library artifacts.
- [x] Package the C ABI umbrella/header set (`include/nnrp/nnrp.h`, runtime, error, version, and FFI headers) into every native release artifact.
- [x] Add CI jobs that verify host native library names/exported symbols and release jobs that build cross-platform artifact matrices.
- [x] Add Node.js native-loading guidance for backend JS/TS users.
- [x] Add transport-scoped native artifact build modes for `all`, `tcp`, and `quic` so downstream SDK transport packages consume real scoped link libraries instead of config flags over one hidden artifact.
- [x] Keep the scoped native artifacts on the existing coarse FFI surface so transport packaging does not introduce extra cross-language boundary calls or regress hot-path performance.
- [x] Stamp native artifact manifests with `transport_scope` and `transport_slots`, and make scoped builds reject disabled transport ids at connect/bind time.

## WASM And JS/TS Packaging Boundary

- [x] Add a dedicated WASM-facing crate or feature surface instead of exposing the raw C ABI as the browser SDK.
- [x] Add minimal `wasm-bindgen` bindings for low-level session/config/probe primitives that `nnrp-js` can wrap.
- [x] Emit WASM package primitives and generated `.d.ts` files for `nnrp-js`; do not keep npm package layout, bundler adapters, or browser app examples in this repository.
- [x] Document the downstream split: Node may prefer native link libraries and fall back to WASM, while browsers consume WASM plus WebSocket/WebTransport adapters implemented in `nnrp-js`.
- [x] Keep browser documentation clear that native link libraries are not usable there; browsers consume WASM plus web transports.
- [x] Add transport-scoped WASM primitive artifacts for `all`, `tcp`, and `quic`, with scoped manifests and runtime rejection for disabled transport ids.
- [x] Keep scoped WASM artifacts as provider/probe primitives owned by `nnrp-rs`, while package naming and browser/client/server composition remain downstream SDK responsibilities.

## Validation

- [x] Add provider registry unit tests for present/missing native library cases.
- [x] Add transport policy resolver tests for every local/remote capability combination.
- [x] Add probe selection tests that cover success, timeout, downgrade, and force-policy failure.
- [x] Add native artifact build checks in release CI.
- [x] Add WASM build checks and a minimal JS/TS smoke test.
- [x] Add single-transport FFI checks so scoped builds cannot silently advertise or accept transports outside their artifact boundary.
