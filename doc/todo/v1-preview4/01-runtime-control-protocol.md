# 01 - Runtime Control Protocol

## Control Frame Model

- [ ] Add Rust enums and payload structs for cancellation and abort.
  - [ ] `CANCEL`.
  - [ ] `ABORT`.
  - [ ] Cancellation source.
  - [ ] Cancellation reason.
  - [ ] Operation identifier.
- [ ] Add Rust enums and payload structs for scheduling.
  - [ ] `PRIORITY_UPDATE`.
  - [ ] `DEADLINE`.
  - [ ] `EXPIRE_AT`.
  - [ ] `SUPERSEDE`.
  - [ ] `BUDGET_UPDATE`.
- [ ] Add Rust enums and payload structs for streaming progress.
  - [ ] `PROGRESS`.
  - [ ] `PARTIAL_RESULT`.
  - [ ] Progress stage.
  - [ ] Optional percentage.
  - [ ] Optional object reference.
- [ ] Add Rust enums and payload structs for pressure management.
  - [ ] `BACKPRESSURE`.
  - [ ] `CREDIT_UPDATE`.
  - [ ] Window size.
  - [ ] Pressure reason.
- [ ] Add Rust enums and payload structs for negotiation and routing.
  - [ ] `CAPABILITY_NEGOTIATION`.
  - [ ] `DEGRADE_PROFILE`.
  - [ ] `ROUTE_HINT`.
  - [ ] `EXECUTION_HINT`.
  - [ ] Cost and preference metadata.
- [ ] Add Rust enums and payload structs for diagnostics.
  - [ ] `TRACE_CONTEXT`.
  - [ ] `RESULT_DROP_REASON`.
  - [ ] `ERROR_RECOVERABLE`.
  - [ ] `RETRY_AFTER`.

## Encoding And Validation

- [ ] Add binary encoding helpers for every control frame family.
  - [ ] Validate fixed fields.
  - [ ] Validate variable metadata lengths.
  - [ ] Reject unknown required fields.
  - [ ] Preserve unknown optional extension fields for diagnostics.
- [ ] Add decoding helpers for every control frame family.
  - [ ] Decode without heap allocation where fixed layout is enough.
  - [ ] Surface typed error families instead of generic decode failures.
  - [ ] Preserve trace identifiers through decode errors when present.
- [ ] Add roundtrip tests for every frame family.
- [ ] Add negative tests for malformed operation IDs, deadlines, credits, and trace metadata.

## Runtime Semantics

- [ ] Route cancellation into operation lifecycle state.
  - [ ] Cooperative cancellation.
  - [ ] Hard abort.
  - [ ] Late result suppression.
- [ ] Route priority and deadline changes into scheduler metadata.
  - [ ] Update priority without reopening the session.
  - [ ] Expire stale work before final result delivery.
  - [ ] Emit `RESULT_DROP_REASON` when stale work is discarded.
- [ ] Route progress and partial results through event queues.
  - [ ] Preserve order within one operation.
  - [ ] Allow interleaving across operations.
  - [ ] Support bounded event polling.
- [ ] Route backpressure and credit changes through transport/provider state.
  - [ ] Apply send window changes.
  - [ ] Report pressure to host code.
  - [ ] Avoid unbounded buffering in runtime queues.

## Conformance Hooks

- [ ] Add control-frame capability declarations to Rust capability reports.
- [ ] Add conformance fixture coverage for representative control frames.
- [ ] Add runtime tests that mirror wire scenarios for cancel, priority/deadline, progress/backpressure, and route/cache behavior.
