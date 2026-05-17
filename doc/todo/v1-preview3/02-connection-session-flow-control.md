# Rust Preview3 Connection, Session, And Flow Control

## Scope

1. `02` owns the canonical Rust implementation of preview3 connection/session state machines, scheduling enums, credit semantics, and recovery validation.
2. `02` may only implement semantics that are already frozen in `nnrp-doc`; if a lifecycle, credit, or recovery rule is still under design, it must not be silently finalized here.
3. `02` defines the host-neutral source of truth that `nnrp-cs` and `nnrp-py` consume; downstream SDKs must not fork these semantics.

## Sub-Shards

1. `02a-connection-session-lifecycle.md`: header/version primitives, lifecycle metadata, and host-neutral multi-session state machines.
2. `02b-scheduling-and-operation-model.md`: priority classes, operation lifecycle, cancel scope, and `FLOW_UPDATE` scheduling semantics.
3. `02c-recovery-and-binding-consumption.md`: recovery validation plus the consumption rules exported to downstream SDKs.

## Dependency Gates

1. `02a` depends on `nnrp-doc` freezing connection/session lifecycle metadata and common-header rules; it should not wait on FFI packaging.
2. `02b` depends on `nnrp-doc` freezing priority, lifecycle, cancel-scope, and `FLOW_UPDATE` semantics; it must not let crate-local convenience leak into protocol behavior.
3. `02c` depends on `nnrp-doc` freezing recovery-object and resume-window semantics; it should export opaque recovery consumption rules rather than SDK-specific retry policies.
4. `04b` and downstream SDK shards may consume `02`, but must not redefine `02` semantics through FFI helper behavior.