# 06 - Release Validation And Docs

## Version And Release Flow

- [ ] Move Rust workspace version to the preview4 release line in the release-preparation commit.
- [ ] Keep preview3 release artifacts available for downstream SDKs until they move to preview4.
- [x] Add release notes that explain preview4 runtime control, runtime object, IPC, and WebSocket work.
- [x] Add downstream SDK maintainer checklist for required artifact and API updates.

## Validation Gates

- [ ] Run workspace formatting.
- [ ] Run workspace clippy with warnings denied.
- [ ] Run workspace tests.
- [ ] Run conformance preview2, preview3, and preview4 baseline validation.
- [ ] Run wire conformance dry-run.
- [ ] Run transport loopback tests.
  - [ ] TCP.
  - [ ] QUIC.
  - [ ] IPC.
  - [ ] WebSocket.
- [ ] Run native artifact inspection.
- [ ] Run browser WASM artifact inspection.

## Benchmark Gates

- [ ] Preserve preview3 coarse FFI benchmark baselines.
- [x] Add preview4 control-frame hot path benchmarks.
- [x] Add runtime object declare/ref/release benchmarks.
- [x] Add IPC loopback benchmark.
- [x] Add WebSocket loopback benchmark.
- [ ] Compare Python cffi API hot path with the preview4 artifact set.
- [ ] Compare JavaScript native transport and browser WASM hot paths with the preview4 artifact set.

## Documentation

- [x] Update README with preview4 transport and runtime-object scope.
- [x] Update native link library documentation for transport-scoped artifacts.
- [x] Update browser WASM documentation for WebSocket substrate.
- [x] Update conformance usage examples for wire target manifests.
- [x] Keep generated headers and manifests in sync with release artifacts.
