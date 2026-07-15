# 06 - Release Validation And Docs

## Version And Release Flow

- [x] Move Rust workspace version to the preview4 release line in the release-preparation commit.
- [x] Keep preview3 release artifacts available for downstream SDKs until they move to preview4.
- [x] Add release notes that explain preview4 runtime control, runtime object, IPC, and WebSocket work.
- [x] Add downstream SDK maintainer checklist for required artifact and API updates.

## Validation Gates

- [x] Run workspace formatting.
- [x] Run workspace clippy with warnings denied.
- [x] Run workspace tests.
- [x] Run conformance preview2, preview3, and preview4 baseline validation.
- [x] Run wire conformance dry-run.
- [x] Run transport loopback tests.
  - [x] TCP.
  - [x] QUIC.
  - [x] IPC.
  - [x] WebSocket.
- [x] Run native artifact inspection.
  - [x] Verify transport-scoped export sets.
  - [x] Load host DLL, SO, and dylib outputs in their platform CI jobs.
  - [x] Run real packet-batch loopbacks through each loaded transport artifact.
- [x] Run browser WASM artifact inspection.

## Benchmark Gates

- [x] Preserve preview3 coarse FFI benchmark baselines.
- [x] Add preview4 control-frame hot path benchmarks.
- [x] Add runtime object declare/ref/release benchmarks.
- [x] Add IPC loopback benchmark.
- [x] Add WebSocket loopback benchmark.

## Documentation

- [x] Update README with preview4 transport and runtime-object scope.
- [x] Update native link library documentation for transport-scoped artifacts.
- [x] Update browser WASM documentation for WebSocket substrate.
- [x] Update conformance usage examples for wire target manifests.
- [x] Keep generated headers and manifests in sync with release artifacts.
