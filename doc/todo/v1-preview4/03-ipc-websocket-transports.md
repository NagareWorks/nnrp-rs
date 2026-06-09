# 03 - IPC And WebSocket Transports

## Transport Provider Contract

- [ ] Extend the provider registry to expose four named transport providers.
  - [ ] TCP provider.
  - [ ] QUIC provider.
  - [x] IPC provider.
  - [ ] WebSocket provider.
- [ ] Keep provider probing behavior stable.
  - [ ] If one transport package is present, select that transport directly.
  - [ ] If multiple transport packages are present, probe candidates by policy.
  - [ ] Preserve capability, cost, and preference metadata in probe results.
- [ ] Ensure every provider owns real connect/listen/send/receive behavior.
- [ ] Keep provider packages from becoming configuration-only switches.

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
- [ ] Implement cancellation-safe read/write tasks.
- [ ] Add loopback tests.
  - [x] Client to server handshake.
  - [x] Submit/result.
  - [ ] Cancel/drop reason.
  - [ ] Backpressure credit update.

## WebSocket Transport

- [ ] Add `nnrp-transport-websocket` crate.
  - [ ] Add Cargo package metadata.
  - [ ] Add provider registration.
  - [ ] Add endpoint parser for `ws://`.
  - [ ] Add endpoint parser for `wss://`.
  - [ ] Add endpoint parser tests.
- [ ] Implement native WebSocket client connect.
- [ ] Implement native WebSocket server accept.
- [ ] Map binary WebSocket messages to NNRP frames.
- [ ] Reject text-message protocol paths for NNRP data frames.
- [ ] Implement close frame mapping to NNRP transport close diagnostics.
- [ ] Add loopback tests.
  - [ ] Client to server handshake.
  - [ ] Submit/result.
  - [ ] Progress/partial result.
  - [ ] Backpressure credit update.

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
