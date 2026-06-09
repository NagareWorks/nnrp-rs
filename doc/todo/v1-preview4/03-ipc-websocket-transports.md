# 03 - IPC And WebSocket Transports

## Transport Provider Contract

- [x] Extend the provider registry to expose four named transport providers.
  - [x] TCP provider.
  - [x] QUIC provider.
  - [x] IPC provider.
  - [x] WebSocket provider.
- [x] Keep provider probing behavior stable.
  - [x] If one transport package is present, select that transport directly.
  - [x] If multiple transport packages are present, probe candidates by policy.
  - [x] Preserve capability, cost, and preference metadata in probe results.
- [x] Ensure every provider owns real connect/listen/send/receive behavior.
- [x] Keep provider packages from becoming configuration-only switches.

## IPC Transport

- [x] Add `nnrp-transport-ipc` crate.
  - [x] Add Cargo package metadata.
  - [x] Add provider registration.
  - [x] Add endpoint parser for `unix://` paths.
  - [x] Add endpoint parser for `npipe://` paths.
  - [x] Add endpoint parser tests.
- [x] Implement local client connect.
- [x] Implement local server listen.
- [x] Implement framed read/write over IPC streams.
- [x] Implement graceful close.
- [x] Implement cancellation-safe read/write tasks.
- [x] Add loopback tests.
  - [x] Client to server handshake.
  - [x] Submit/result.
  - [x] Cancel/drop reason.
  - [x] Backpressure credit update.

## WebSocket Transport

- [x] Add `nnrp-transport-websocket` crate.
  - [x] Add Cargo package metadata.
  - [x] Add provider registration.
  - [x] Add endpoint parser for `ws://`.
  - [x] Add endpoint parser for `wss://`.
  - [x] Add endpoint parser tests.
- [x] Implement native WebSocket client connect.
- [x] Implement native WebSocket server accept.
- [x] Map binary WebSocket messages to NNRP frames.
- [x] Reject text-message protocol paths for NNRP data frames.
- [x] Implement close frame mapping to NNRP transport close diagnostics.
- [x] Add loopback tests.
  - [x] Client to server handshake.
  - [x] Submit/result.
  - [x] Progress/partial result.
    - [x] Progress.
    - [x] Partial result.
  - [x] Backpressure credit update.

## WASM And Browser Boundary

- [ ] Expose shared WebSocket frame codec hooks for `nnrp-wasm`.
- [ ] Keep browser WebSocket API as an I/O substrate.
- [ ] Keep NNRP framing, control semantics, and diagnostics in Rust/WASM-owned logic.
- [ ] Add WASM tests or generated fixtures for browser WebSocket frame mapping.

## Packaging

- [ ] Add IPC native artifacts to release packaging.
- [ ] Add WebSocket native artifacts to release packaging.
- [ ] Ensure transport-specific artifacts remain scoped to transport packages.
- [ ] Ensure downstream SDK manifests can distinguish TCP, QUIC, IPC, and WebSocket artifacts.
