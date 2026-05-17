# NNRP/1-preview3 Rust Implementation Todo

## 0. Scope

1. This directory tracks the Rust canonical SDK work required to implement `NNRP/1-preview3`.
2. `nnrp-rs` is the single implementation source for preview3 wire codecs, state machines, cache/schema semantics, conformance fixtures, and stable FFI contracts.
3. Python, C#, and future language SDKs must consume preview3 semantics from this repository rather than re-implementing the hot path locally.

## 1. Shard Map

1. `01-foundation-and-contract.md`: land the frozen preview3 contract as code, ABI, and downstream-consumption baseline.
2. `02-connection-session-flow-control.md`: ownership and dependency map for the `02a/02b/02c` connection/session shards.
3. `02a-connection-session-lifecycle.md`: common-header, connection/session lifecycle metadata, and host-neutral multi-session state machines.
4. `02b-scheduling-and-operation-model.md`: priority, operation lifecycle, cancel scope, and `FLOW_UPDATE` scheduling semantics.
5. `02c-recovery-and-binding-consumption.md`: recovery validation and export rules consumed by Python/C#.
6. `03-cache-schema-profile-registry.md`: cache lease contract, schema/profile registry, standard profiles, typed payload descriptors.
7. `04-implementation-surface.md`: ownership and dependency map for the `04a/04b/04c` implementation-surface shards.
8. `04a-core-surface.md`: `nnrp-core` wire primitives, descriptors, and validation core.
9. `04b-ffi-surface.md`: `nnrp-ffi` handle/ABI/event-delivery/buffer-ownership surface.
10. `04c-conformance-and-binding-rollout.md`: `nnrp-conformance` exports and downstream binding-consumption contract.
11. `05-validation-and-docs.md`: workspace validation, conformance exports, and rollout documentation.

## 2. PR Rules

1. One shard per PR by default; foundation/contract work should land before binding-facing implementation PRs depend on it.
2. `main` should accept reviewed PRs only after GitHub publication.
3. If a preview3 item changes wire shape, lifecycle semantics, error behavior, or descriptor layout, it must land here before Python/C# consume it.
4. `02` and `03` may not invent protocol semantics that are not already frozen in `nnrp-doc`; if the contract is still open, update `nnrp-doc` first.
5. `04` may wire frozen semantics into Rust crates and exports, but it must not use FFI or conformance work as a backdoor to freeze new protocol behavior.

## 3. Protocol Coverage Check

1. FFI handle families, callback/polling model, thread affinity, and error families are tracked in `01` and `04`.
2. `SESSION_OPEN` / `SESSION_OPEN_ACK`, explicit session close, multi-session routing, and recovery object semantics are tracked in `01` and `02`.
3. Priority classes, operation states, cancel scope, and `FLOW_UPDATE` 32B semantics are tracked in `01` and `02`.
4. Cache lease/version/dependency rules, schema descriptor 32B, typed payload descriptor 24B, and `descriptor_flags` are tracked in `01` and `03`.
5. `tensor` / `token` first-round standard profiles plus `structured_event` / `tool_delta` ownership boundaries are tracked in `01` and `03`.
6. Rust conformance-first enum/message/error baselines and downstream binding-consumption rules are tracked in `01`, `04`, and `05`.