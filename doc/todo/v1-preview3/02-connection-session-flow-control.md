# Rust Preview3 Connection, Session, And Flow Control

## Scope

1. `02` owns the canonical Rust implementation of preview3 connection/session state machines, scheduling enums, credit semantics, and recovery validation.
2. `02` implements the lifecycle, credit, recovery, and scheduling rules already defined by the preview3 protocol contract.
3. `02` defines the host-neutral source of truth that `nnrp-cs` and `nnrp-py` consume; downstream SDKs must not fork these semantics.

## Sub-Shards

1. `02a-connection-session-lifecycle.md`: header/version primitives, lifecycle metadata, and host-neutral multi-session state machines.
2. `02b-scheduling-and-operation-model.md`: priority classes, operation lifecycle, cancel scope, and `FLOW_UPDATE` scheduling semantics.
3. `02c-recovery-and-binding-consumption.md`: recovery validation plus the consumption rules exported to downstream SDKs.

## Integration Gates

1. `02a` implements connection/session lifecycle metadata and common-header rules without waiting on FFI packaging.
2. `02b` implements priority, lifecycle, cancel-scope, and `FLOW_UPDATE` semantics without letting crate-local convenience leak into protocol behavior.
3. `02c` implements recovery-object and resume-window semantics and exports opaque recovery consumption rules rather than SDK-specific retry policies.
4. `04b` and downstream SDK shards may consume `02`, but must not redefine `02` semantics through FFI helper behavior.
