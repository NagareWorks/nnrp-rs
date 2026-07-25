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
  - [x] Match probe samples by stable provider id instead of package display name.
  - [x] Preserve provider cost, preference, limits, and limitations in candidate diagnostics.
  - [x] Apply the frozen success-count, throughput, RTT, comparable-cost, policy, and identity comparator.
  - [x] Expose ordered candidates and registered rejection reasons without an opaque score.
- [x] Ensure every provider owns real connect/listen/send/receive behavior.
- [x] Keep provider packages from becoming configuration-only switches.

## Host Route Model

- [x] Add the frozen application and provider endpoint values.
  - [x] Add `NnrpEndpoint` for `nnrp://` and `nnrps://` only.
  - [x] Add `ProviderEndpoint` for carrier-local locator overrides.
  - [x] Preserve application authority, path, query, and security intent.
  - [x] Reject provider-local schemes in `NnrpEndpoint`.
- [ ] Add role-specific route values.
  - [x] Add `ClientProviderRoute` and `ClientProviderRoutes`.
  - [x] Add `ServerProviderRoute` and `ServerProviderRoutes`.
  - [x] Add the exact owned fields for `ClientTransportSecurity` and `ServerTransportSecurity`.
  - [x] Keep locator and security values isolated per transport ID.
  - [ ] Reject duplicate keys, provider-kind mismatches, and role-mismatched security.
  - [ ] Report a configured known-but-uninstalled route as `local-unavailable`.
  - [ ] Apply the exact rejection precedence when multiple checks fail.
- [ ] Add host-level client orchestration.
  - [ ] Resolve every installed provider route before selection.
  - [ ] Keep unresolved and security-incompatible candidates in diagnostics.
  - [ ] Probe all eligible Auto/Prefer candidates.
  - [ ] Adopt only the selected carrier into `NnrpClient`.
  - [ ] Make Force fail without fallback.
- [ ] Add host-level server orchestration.
  - [ ] Resolve every policy-allowed installed provider route.
  - [ ] Bind every eligible Auto/Prefer listener into one logical `NnrpServer`.
  - [ ] Restrict Force to the named listener.
  - [ ] Roll back every listener opened by a failed logical listen operation.
  - [ ] Accept across the listener set while each session adopts one carrier.
  - [ ] Expose the actual `active_transport_id` on every accepted session.
  - [ ] Expose actual bound provider endpoints, including assigned ports.
  - [ ] Break simultaneous accept readiness with stable provider order.
  - [ ] Fail and close the complete logical set after a terminal provider-listener failure.

## Application Security Intent

- [ ] Enforce `nnrps://` before probing or binding.
  - [ ] Add TCP TLS client and server paths with route-local credentials.
  - [ ] Keep plain TCP visible as `security-unsatisfied` for `nnrps://`.
  - [ ] Keep QUIC TLS credentials route-local.
  - [ ] Reject IPC for `nnrps://` in Preview4.
  - [ ] Require WSS and route-local credentials for native WebSocket.
  - [ ] Preserve browser-owned TLS verification for browser WSS.
- [x] Add `route-unresolved` and `security-unsatisfied` to the exact rejection registry.
- [ ] Keep `nnrp://` compatible with both plain and secure eligible routes.

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

- [x] Expose shared WebSocket frame codec hooks for `nnrp-wasm`.
- [x] Keep browser WebSocket API as an I/O substrate.
- [x] Keep NNRP framing, control semantics, and diagnostics in Rust/WASM-owned logic.
- [x] Add WASM tests or generated fixtures for browser WebSocket frame mapping.

## Packaging

- [x] Add IPC native artifacts to release packaging.
- [x] Add WebSocket native artifacts to release packaging.
- [x] Ensure transport-specific artifacts remain scoped to transport packages.
- [x] Ensure downstream SDK manifests can distinguish TCP, QUIC, IPC, and WebSocket artifacts.
  - [x] Write the frozen provider id, cost, preference rank, frame limit, and limitations.
  - [x] Reject aggregate `all` native artifacts from the release surface.
