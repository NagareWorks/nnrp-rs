# NNRP/1-preview3 Rust Implementation Todo

## 0. Scope

1. This file tracks the Rust canonical SDK work required to implement `NNRP/1-preview3`.
2. `nnrp-rs` is the single implementation source for preview3 wire codecs, state machines, cache/schema semantics, conformance fixtures, and stable FFI contracts.
3. Python, C#, and future language SDKs must consume preview3 semantics from this repository rather than re-implementing the hot path locally.
4. Any preview3 item that affects cross-language wire shape, lifecycle semantics, or error behavior belongs here first unless it is purely binding-specific.

## 1. Current Baseline

- [x] Create the initial Cargo workspace for `nnrp-core`, `nnrp-ffi`, and `nnrp-conformance`.
- [x] Freeze the repository role: Rust is the canonical preview3 implementation source; other SDKs are binding/integration layers.
- [ ] Freeze the final crate ownership boundaries before substantial implementation begins.
	- [ ] Keep `nnrp-core` responsible for wire primitives, protocol validation, state-machine core types, cache/schema semantics, and reusable host-neutral logic.
	- [ ] Keep `nnrp-ffi` responsible for stable ABI, handle lifecycle, callback/polling surfaces, and cross-language buffer ownership rules.
	- [ ] Keep `nnrp-conformance` responsible for golden vectors, fixture manifests, cross-language conformance exports, and protocol regression baselines.

## 2. First-Round Freeze Gate

- [ ] Freeze the preview3 connection/session lifecycle before binding work starts.
	- [ ] Confirm the connection-level vs session-level message set (`CLIENT_HELLO`, `SERVER_HELLO_ACK`, `SESSION_OPEN`, `SESSION_OPEN_ACK`, `SESSION_PATCH`, session-close semantics, connection `CLOSE`).
	- [ ] Freeze `SESSION_OPEN` and `SESSION_OPEN_ACK` fixed metadata tables, including final 48B / 56B layouts.
	- [ ] Freeze `session_flags`, `session_status`, `session_flags_ack`, and `session_error_code` values.
	- [ ] Freeze whether preview3 introduces explicit session-close metadata or keeps session close as a specialized control frame.
	- [ ] Freeze the minimal multi-session routing fields required on control/data messages.
- [ ] Freeze the FFI contract before Python/C# bindings start implementation.
	- [ ] Freeze canonical handle families: connection, session, operation, schema, and buffer view.
	- [ ] Freeze callback vs polling entry points and thread-affinity expectations.
	- [ ] Freeze stable error-code families and binding-facing error mapping rules.
- [ ] Freeze the first-round scheduling semantics.
	- [ ] Freeze session priority classes and numeric values.
	- [ ] Freeze operation lifecycle states and cancellation scopes with numeric values.
	- [ ] Freeze the final 32B `FLOW_UPDATE` metadata table.
	- [ ] Freeze `scope_kind`, `update_reason`, `backpressure_level`, and `flow_flags` values.
	- [ ] Freeze which flow-control signals are connection-scoped vs session-scoped vs operation-scoped.
- [ ] Freeze the advanced cache contract.
	- [ ] Freeze `object_id`, `object_version`, lease identity, expiry, renew, and dependency invalidation semantics.
	- [ ] Freeze the minimum stable error reasons for cache miss, lease expiry, version mismatch, and dependency invalidation.
- [ ] Freeze the schema/profile registry contract.
	- [ ] Freeze the first-round standard profile set and keep the preview3 public layer profile-neutral rather than tensor-privileged.
	- [ ] Treat `tensor` and `token` as peer first-round standard profiles unless the protocol doc freezes a broader set.
	- [ ] Freeze the minimum standard semantics for the token profile before any binding invents token-chunk-specific lifecycle rules.
	- [ ] Freeze the standard schema descriptor header fields, including the final fixed 32B common header.
	- [ ] Freeze the minimum typed-payload descriptor fields shared across profiles, including the final fixed 24B common layout.
	- [ ] Freeze `descriptor_flags` bit definitions.
	- [ ] Freeze the profile-specific minimum interpretation entry points required for tensor vs token payloads.
	- [ ] Freeze schema install/update/invalidate/version-conflict handling.
	- [ ] Freeze how typed payload descriptors bind to schema/profile identifiers.
- [ ] Freeze the payload-family vs lifecycle boundary.
	- [ ] Keep `structured_event` and `tool_delta` as payload families by default.
	- [ ] Freeze which fields must enter the public operation lifecycle model because they affect cross-language routing, cancellation, or state transitions.
	- [ ] Keep tool/event payload bodies in schema/profile space unless the protocol doc explicitly promotes them.
- [ ] Freeze conformance ownership.
	- [ ] Rust-generated golden vectors are the only canonical preview3 fixtures.
	- [ ] Python/C# test suites may import/export these fixtures but must not define competing preview3 canonical vectors.

## 3. nnrp-core

- [ ] Add preview3 version/stage primitives beyond the initial placeholder.
- [ ] Implement fixed-width common-header codecs and strict preview3 stage handling.
- [ ] Implement preview3 fixed metadata models for connection/session lifecycle messages.
- [ ] Implement preview3 typed payload descriptors, extension descriptors, and schema/profile binding rules.
- [ ] Implement preview3 cache lease, object version, dependency tracking, and invalidation primitives.
- [ ] Implement preview3 operation/workflow identifiers, lifecycle states, and cancellation semantics.
- [ ] Implement host-neutral connection/session state machines for multi-session orchestration.
- [ ] Implement strict validation for illegal lifecycle, cache, and schema combinations.

## 4. nnrp-ffi

- [ ] Define stable ABI-safe handle layouts and ownership rules.
- [ ] Expose connection bootstrap, session open/patch/close, submit, result/event pump, and control operations through FFI.
- [ ] Expose zero-copy or bounded-copy buffer-view APIs suitable for Python and C# bindings.
- [ ] Expose callback-driven and polling-driven event delivery surfaces.
- [ ] Expose stable preview3 error codes and diagnostics to binding layers.
- [ ] Add ABI compatibility tests and fixture-driven smoke coverage.

## 5. nnrp-conformance

- [ ] Export preview3 canonical golden vectors from Rust.
- [ ] Export cross-language fixture manifests for Python/C# binding tests.
- [ ] Add conformance fixtures for multi-session routing, cache lease events, schema conflicts, and operation lifecycle transitions.
- [ ] Add regression fixtures for degraded/partial/stale/drop outcomes under preview3 routing semantics.

## 6. Binding Rollout Support

- [ ] Publish a binding-consumption contract for Python and C#.
	- [ ] Library naming/loading expectations.
	- [ ] Versioning and compatibility window policy.
	- [ ] Feature negotiation rules for preview2 compatibility vs preview3 native paths.
- [ ] Document which preview2 semantics remain compatibility-only and which must move fully into preview3 Rust handles.

## 7. Validation

- [ ] Keep `cargo test` green across the workspace.
- [ ] Add fixture-driven tests for all frozen preview3 enum/message/error values.
- [ ] Add integration tests for multi-session orchestration on one connection.
- [ ] Add validation for cache lease expiry, schema mismatch, cancellation, and priority-aware flow updates.
- [ ] Add FFI smoke tests that prove Python/C# can bind without redefining protocol semantics.

## 8. Documentation

- [ ] Document the workspace layout and crate ownership boundaries.
- [ ] Document the frozen preview3 connection/session, cache, schema, and operation lifecycle semantics.
- [ ] Document the FFI contract and binding responsibilities.
- [ ] Document the conformance workflow for Python, C#, and future JS/Java/Go bindings.