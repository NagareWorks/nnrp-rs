# Rust Preview3 Connection, Session, And Flow Control

## Host-Neutral State Machines

- [ ] Add preview3 version/stage primitives beyond the initial placeholder.
- [ ] Implement fixed-width common-header codecs and strict preview3 stage handling.
- [ ] Implement fixed metadata models for connection/session lifecycle messages.
- [ ] Implement host-neutral connection/session state machines for multi-session orchestration.
- [ ] Implement explicit session-close and resume concepts once frozen.

## Scheduling And Operation Model

- [ ] Implement session priority classes and operation lifecycle/cancel-scope enums.
- [ ] Implement operation/workflow identifiers, parent/group relationships, and lifecycle transitions.
- [ ] Implement the 32B `FLOW_UPDATE` metadata model and three-scope credit semantics.
- [ ] Implement strict validation for illegal lifecycle and scheduling combinations.

## Recovery

- [ ] Implement resume-token, resume-window, and recovery-object validation once the upstream concept freezes.
- [ ] Export recovery semantics in a way that Python/C# can consume without inventing their own retry state machines.