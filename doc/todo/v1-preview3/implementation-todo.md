# NNRP/1-preview3 Rust Implementation Todo

## 0. Scope

1. This directory tracks the Rust canonical SDK work required to implement `NNRP/1-preview3`.
2. `nnrp-rs` is the single implementation source for preview3 wire codecs, state machines, cache/schema semantics, conformance fixtures, and stable FFI contracts.
3. Python, C#, and future language SDKs must consume preview3 semantics from this repository rather than re-implementing the hot path locally.

## 1. Shard Map

1. `00-inherited-nnrp1-baseline.md`: land preview1/preview2/current NNRP/1 contracts that preview3 inherits and Rust must implement from zero.
2. `01-foundation-and-contract.md`: land inherited NNRP/1 contracts plus preview3 extensions as code, ABI, and downstream-consumption baseline.
3. `02-connection-session-flow-control.md`: ownership and dependency map for the `02a/02b/02c` connection/session shards.
4. `02a-connection-session-lifecycle.md`: common-header, connection/session lifecycle metadata, and host-neutral multi-session state machines.
5. `02b-scheduling-and-operation-model.md`: inherited `FLOW_UPDATE` semantics plus preview3 priority, operation lifecycle, and cancel scope.
6. `02c-recovery-and-binding-consumption.md`: recovery validation and export rules consumed by Python/C#.
7. `03-cache-schema-profile-registry.md`: cache lease contract, schema/profile registry, standard profiles, typed payload descriptors.
8. `04-implementation-surface.md`: ownership and dependency map for the `04a/04b/04c` implementation-surface shards.
9. `04a-core-surface.md`: `nnrp-core` wire primitives, descriptors, and validation core.
10. `04b-ffi-surface.md`: `nnrp-ffi` handle/ABI/event-delivery/buffer-ownership surface.
11. `04c-conformance-and-binding-rollout.md`: `nnrp-conformance` exports and downstream binding-consumption contract.
12. `05-validation-and-docs.md`: workspace validation, conformance exports, and rollout documentation.
13. `06-client-server-runtime.md`: usable Rust client/server SDK surface, transport pump, and runtime-backed FFI entrypoints.

## 2. PR Rules

1. One shard per PR by default; foundation/contract work should land before binding-facing implementation PRs depend on it.
2. `main` should accept reviewed PRs only after GitHub publication.
3. If an inherited NNRP/1 item or preview3 extension changes wire shape, lifecycle semantics, error behavior, or descriptor layout, it must land here before Python/C# consume it.
4. `02` and `03` may not invent protocol semantics that are not already inherited from preview1/preview2/current NNRP/1 docs or newly frozen in `nnrp-doc`; if the contract is still open, update `nnrp-doc` first.
5. `04` may wire frozen semantics into Rust crates and exports, but it must not use FFI or conformance work as a backdoor to freeze new protocol behavior.

## 3. Protocol Coverage Check

1. Inherited preview1/preview2/current NNRP/1 wire, control-plane, data-plane, cache, transport-probe, and migration baselines are tracked in `00`.
2. FFI handle families, callback/polling model, thread affinity, and error families are tracked in `01` and `04`.
3. `SESSION_OPEN` / `SESSION_OPEN_ACK`, explicit session close, multi-session routing, and recovery object semantics are tracked in `01` and `02`.
4. Inherited `FLOW_UPDATE` 32B semantics plus preview3 priority classes, operation states, and cancel scope are tracked in `00`, `01`, and `02`.
5. Inherited cache/typed-payload pieces plus preview3 cache lease/version/dependency rules, schema descriptor 32B, typed payload descriptor 24B, and `descriptor_flags` are tracked in `00`, `01`, and `03`.
6. `tensor` / `token` first-round standard profiles plus `structured_event` / `tool_delta` ownership boundaries are tracked in `01` and `03`.
7. Rust conformance-first enum/message/error baselines and downstream binding-consumption rules are tracked in `00`, `01`, `04`, and `05`.
8. Usable Rust client/server APIs, transport runtime, loopback integration, and runtime-backed FFI entrypoints are tracked in `06`.
