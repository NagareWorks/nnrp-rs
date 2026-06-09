# 01 - Runtime Control Protocol

## Control Frame Model

- [x] Add Rust enums and payload structs for cancellation and abort.
  - [x] `CANCEL`.
  - [x] `ABORT`.
  - [x] Cancellation source.
  - [x] Cancellation reason.
  - [x] Operation identifier.
- [x] Add Rust enums and payload structs for scheduling.
  - [x] `PRIORITY_UPDATE`.
  - [x] `DEADLINE`.
  - [x] `EXPIRE_AT`.
  - [x] `SUPERSEDE`.
  - [x] `BUDGET_UPDATE`.
- [x] Add Rust enums and payload structs for streaming progress.
  - [x] `PROGRESS`.
  - [x] `PARTIAL_RESULT`.
  - [x] Progress stage.
  - [x] Optional percentage.
  - [x] Optional object reference.
- [x] Add Rust enums and payload structs for pressure management.
  - [x] `BACKPRESSURE`.
  - [x] `CREDIT_UPDATE`.
  - [x] Window size.
  - [x] Pressure reason.
- [x] Add Rust enums and payload structs for negotiation and routing.
  - [x] `CAPABILITY_NEGOTIATION`.
  - [x] `DEGRADE_PROFILE`.
  - [x] `ROUTE_HINT`.
  - [x] `EXECUTION_HINT`.
  - [x] Cost and preference metadata.
- [x] Add Rust enums and payload structs for diagnostics.
  - [x] `TRACE_CONTEXT`.
  - [x] `RESULT_DROP_REASON`.
  - [x] `ERROR_RECOVERABLE`.
  - [x] `RETRY_AFTER`.

## Encoding And Validation

- [x] Add binary encoding helpers for every control frame family.
  - [x] Validate fixed fields.
  - [x] Validate variable metadata lengths.
  - [x] Reject unknown required fields.
  - [x] Preserve declared optional extension fields for diagnostics.
- [x] Add decoding helpers for every control frame family.
  - [x] Decode without heap allocation where fixed layout is enough.
  - [x] Surface typed declared-length errors instead of generic decode failures.
  - [x] Preserve trace identifiers through decode errors when present.
- [x] Add roundtrip tests for every frame family.
- [x] Add negative tests for malformed operation IDs, deadlines, credits, and trace metadata.

## Runtime Semantics

- [ ] Route cancellation into operation lifecycle state.
  - [x] Cooperative cancellation.
  - [ ] Hard abort.
  - [x] Late result suppression.
- [ ] Route priority and deadline changes into scheduler metadata.
  - [x] Update priority without reopening the session.
  - [x] Persist priority, deadline, and expire-at metadata in the operation registry.
  - [x] Expire stale work before final result delivery.
  - [ ] Emit `RESULT_DROP_REASON` when stale work is discarded.
  - [x] Expose typed `RESULT_DROP_REASON` send/read APIs for host-controlled discard paths.
- [ ] Route progress and partial results through event queues.
  - [ ] Preserve order within one operation.
  - [ ] Allow interleaving across operations.
  - [x] Support host polling for `PARTIAL_RESULT`.
- [ ] Route backpressure and credit changes through transport/provider state.
  - [ ] Apply send window changes.
  - [x] Report pressure to host code.
  - [ ] Avoid unbounded buffering in runtime queues.

## Conformance Hooks

- [x] Add control-frame capability declarations to Rust capability reports.
- [ ] Add conformance fixture coverage for representative control frames.
- [ ] Add runtime tests that mirror wire scenarios for cancel, priority/deadline, progress/backpressure, and route/cache behavior.
  - [x] Runtime loopback coverage for cancel, priority/deadline, partial result, drop reason, and backpressure.
  - [ ] Runtime loopback coverage for route/cache behavior.
