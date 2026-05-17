# Rust Preview3 Implementation Surface

## Scope

1. `04` owns how frozen preview3 semantics are materialized across `nnrp-core`, `nnrp-ffi`, and `nnrp-conformance`.
2. `04` must not be used to finalize protocol behavior that is still unfrozen in `nnrp-doc`; crate and FFI work here is implementation of frozen semantics, not a substitute for protocol design.
3. `04` is the upstream dependency surface for C#/Python SDK wiring and therefore must keep handle, ABI, and export contracts narrower than host-specific convenience APIs.

## Sub-Shards

1. `04a-core-surface.md`: `nnrp-core` wire primitives, descriptors, and validation core.
2. `04b-ffi-surface.md`: `nnrp-ffi` handle/ABI/event-delivery/buffer-ownership surface.
3. `04c-conformance-and-binding-rollout.md`: `nnrp-conformance` exports and downstream binding-consumption contract.

## Dependency Gates

1. `04a` depends on `01/02/03` semantics already being frozen enough to implement in `nnrp-core`; it must not invent descriptor or lifecycle rules locally.
2. `04b` depends on `04a` plus frozen `02` semantics; it exposes handles and delivery primitives, not host-language orchestration policy.
3. `04c` depends on `04a/04b` artifacts and owns canonical vectors, fixture manifests, and binding-consumption notes.
4. Downstream SDK `04` shards may depend on `04b/04c`, but they must not back-port host policy into Rust ABI shape without updating `nnrp-rs` explicitly.