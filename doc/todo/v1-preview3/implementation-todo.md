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
6. `02c-recovery-and-binding-consumption.md`: recovery validation and binding-consumption rules used by Python/C#.
7. `03-cache-schema-profile-registry.md`: cache lease contract, schema/profile registry, standard profiles, typed payload descriptors.
8. `04-implementation-surface.md`: ownership and dependency map for the `04a/04b/04c` implementation-surface shards.
9. `04a-core-surface.md`: `nnrp-core` wire primitives, descriptors, and validation core.
10. `04b-ffi-surface.md`: `nnrp-ffi` handle/ABI/event-delivery/buffer-ownership surface.
11. `04c-conformance-and-binding-rollout.md`: `nnrp-conformance` adapter integration and downstream binding-consumption contract.
12. `05-validation-and-docs.md`: workspace validation, conformance adapter flow, and rollout documentation.
13. `06-client-server-runtime.md`: usable Rust client/server SDK surface, transport pump, and runtime-backed FFI entrypoints.
14. `06a-transport-provider-packaging.md`: split transport providers, native link library packaging, and JS/TS WASM packaging.

## 2. PR Rules

1. One shard per PR by default; foundation/contract work should land before binding-facing implementation PRs depend on it.
2. `main` should accept reviewed PRs only after GitHub publication.
3. If an inherited NNRP/1 item or preview3 extension changes wire shape, lifecycle semantics, error behavior, or descriptor layout, it must land here before Python/C# consume it.
4. `02` and `03` may not invent protocol semantics beyond the inherited preview1/preview2/current NNRP/1 docs and preview3 protocol contract.
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
9. Transport provider packaging, provider discovery, native link libraries, and JS/TS WASM outputs are tracked in `06a`.

## 4. Preview3 Field-Level Traceability

1. `SESSION_OPEN` 48B metadata is tracked by `01` and `02a` and implemented in `nnrp-core::SessionOpenMetadata`: `requested_session_id`, `profile_id`, `priority_class`, `session_flags`, `schema_id`, `schema_version`, inflight limits, `lease_ttl_hint_ms`, `resume_token_bytes`, `auth_bytes`, `session_extension_bytes`, and `client_session_tag`.
2. `SESSION_OPEN_ACK` 56B metadata is tracked by `01` and `02a` and implemented in `nnrp-core::SessionOpenAckMetadata`: `session_id`, accepted profile/priority/status, schema binding, inflight limits, `lease_ttl_ms`, `resume_window_ms`, `resume_token_bytes`, `session_extension_bytes`, `server_session_tag`, `session_error_code`, and `session_flags_ack`.
3. `SESSION_CLOSE` and `SESSION_CLOSE_ACK` metadata plus close status/reason/in-flight policy are tracked by `02a` and implemented in `nnrp-core::SessionCloseMetadata`, `SessionCloseAckMetadata`, and lifecycle validation.
4. Priority, operation lifecycle, cancel scope, and three-scope `FLOW_UPDATE` are tracked by `02b` and implemented in `nnrp-core::SessionPriorityClass`, `OperationState`, `CancelScope`, `FlowUpdateMetadata`, `FlowScopeKind`, `FlowUpdateReason`, `BackpressureLevel`, and lifecycle validation.
5. `RESULT_HINT`, `TRANSPORT_PROBE`, `TRANSPORT_PROBE_ACK`, `SESSION_MIGRATE`, and `SESSION_MIGRATE_ACK` are tracked by `00` and consumed by `02c`/`06a`; their fixed metadata is implemented in `nnrp-core::control` and exported through `nnrp-ffi`.
6. Cache lease identity, owner scope, object kind, object version, dependency validation, invalidation, and cache error behavior are tracked by `03` and implemented in `nnrp-core::cache`, `nnrp-core::codes`, and `nnrp-ffi` cache lease handles.
7. Schema/profile registry identity, `SchemaDescriptorHeader` 32B layout, schema flags, dependency count, stream semantics, schema hash, install/update/invalidate/version-conflict behavior, and schema error behavior are tracked by `03` and implemented in `nnrp-core::schema` plus `nnrp-ffi` schema registry APIs.
8. `TypedPayloadDescriptor` 24B layout, `descriptor_flags`, profile/schema binding, token delta schema anchor, and `profile_id = 0` semantics are tracked by `03` and implemented in `nnrp-core::schema`, `nnrp-core::data`, and descriptor helpers exported through `nnrp-ffi`.
9. Client/server runtime consumption of session defaults, schema/profile registry inputs, resume windows, cache hints, flow updates, cancellation, and transport selection is tracked by `06` and implemented in `nnrp-runtime` plus runtime-backed `nnrp-ffi` entrypoints.
