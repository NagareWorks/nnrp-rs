# 05 - Wire Conformance Runner

## Plan Consumption

- [x] Consume `wire-conformance/nnrp-1-preview4/manifest.json`.
- [x] Consume target manifests with TCP, QUIC, IPC, and WebSocket endpoints.
- [x] Generate execution plans through the shared conformance runner.
- [x] Expose a CLI dry-run entrypoint for suite and target manifest inputs.
- [x] Preserve selected scenario IDs exactly in result reports.
- [x] Write evidence paths for frame logs and timing traces.

## Direct Endpoint Driver

- [ ] Implement suite-as-client mode.
  - [x] TCP endpoint.
  - [ ] QUIC endpoint.
  - [ ] IPC endpoint.
  - [x] WebSocket endpoint.
- [ ] Implement suite-as-server mode.
  - [ ] TCP listener.
  - [ ] QUIC listener.
  - [ ] IPC listener.
  - [ ] WebSocket listener.
- [ ] Implement suite-as-proxy mode.
  - [ ] Bidirectional frame forwarding.
  - [ ] Frame injection.
  - [ ] Timeout injection.
  - [ ] Close injection.
  - [ ] Backpressure injection.
  - [ ] Frame-order perturbation.

## Scenario Execution

- [ ] Execute cancel/abort scenarios.
- [ ] Execute priority/deadline scenarios.
- [ ] Execute progress/backpressure scenarios.
- [ ] Execute capability/route/cache scenarios.
- [ ] Execute IPC-specific cancel scenarios.
- [ ] Execute WebSocket-specific progress/backpressure scenarios.

## Result Validation

- [ ] Record observed frames with direction and timestamp.
- [ ] Validate expected terminal state.
- [ ] Validate required frame presence.
- [ ] Validate result drop reason when expected.
- [ ] Validate trace context propagation when expected.
- [x] Preserve skipped outcomes when a target does not claim the required transport or capability.

## CI Integration

- [x] Keep dry-run plan generation in conformance CI.
- [ ] Add local reference endpoint tests for each concrete transport crate.
- [x] Add negative target tests for unsupported modes and transports.
- [ ] Add matrix coverage for TCP, QUIC, IPC, and WebSocket reference endpoints.
