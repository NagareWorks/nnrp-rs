# 05 - Wire Conformance Runner

## Plan Consumption

- [x] Consume `wire-conformance/nnrp-1-preview4/manifest.json`.
- [x] Consume target manifests with TCP, QUIC, IPC, and WebSocket endpoints.
- [x] Generate execution plans through the shared conformance runner.
- [x] Expose a CLI dry-run entrypoint for suite and target manifest inputs.
- [x] Preserve selected scenario IDs exactly in result reports.
- [x] Write evidence paths for frame logs and timing traces.

## External Target Endpoint Driver

- [x] Implement suite-as-client mode against an externally declared target endpoint.
  - [x] TCP endpoint.
  - [x] QUIC endpoint.
  - [x] IPC endpoint.
  - [x] WebSocket endpoint.
- [x] Implement suite-as-server mode by binding the endpoint declared for an external target client.
  - [x] TCP listener.
  - [x] QUIC listener.
  - [x] IPC listener.
  - [x] WebSocket listener.
- [x] Implement suite-as-proxy mode with an external target server as the upstream endpoint.
  - [x] Bind an ephemeral suite-owned QUIC front endpoint.
  - [x] Terminate and forward typed NNRP operations in both directions.
  - [x] Inject `PRIORITY_UPDATE` and an already-expired non-zero `EXPIRE_AT` timestamp.
  - [x] Preserve the target's typed `RESULT_DROP_REASON` for the suite-owned probe client.
- [x] Load owned TLS material for QUIC and secure WebSocket endpoint roles.
- [x] Reject TLS material on TCP, IPC, and plain WebSocket endpoints.

## Scenario Execution

- [x] Execute cancel/abort scenarios with suite-to-target control direction.
- [x] Execute priority/deadline scenarios through the typed QUIC proxy path.
- [x] Execute progress/backpressure scenarios with target-client-to-suite-server direction.
- [x] Execute capability/route/cache scenarios with suite-to-target hints and target-to-suite cache miss.
- [x] Execute IPC-specific cancel scenarios.
- [x] Execute WebSocket-specific progress/backpressure scenarios.

## Result Validation

- [x] Record observed frames with direction and timestamp.
- [x] Validate expected terminal state.
- [x] Validate required frame presence.
- [x] Validate result drop reason when expected.
- [x] Validate trace context propagation when expected.
- [x] Preserve skipped outcomes when a target does not claim the required transport or capability.

## CI Integration

- [x] Keep dry-run plan generation in conformance CI.
- [x] Add external target role tests for each concrete transport crate.
- [x] Add negative target tests for unsupported modes and transports.
- [x] Add matrix coverage for TCP, QUIC, IPC, plain WebSocket, and secure WebSocket endpoints.
