# NNRP/1 Preview3 Rust Implementation Notes

This repository treats `nnrp-core` as the canonical source for preview3 wire
codecs, lifecycle validation, cache/schema semantics, operation state, and
host-neutral recovery rules. Python, C#, and future SDKs should consume these
contracts rather than re-declaring protocol behavior.

## Frozen Runtime Semantics

- Connection lifecycle is owned by `ConnectionLifecycle`; a connection may host
  multiple independent sessions, and closing one session must not close sibling
  sessions.
- Session open/close and resumed-session outcomes are validated in core. Resume
  tokens are session-bound, and a resumed ack must carry a non-zero resume window
  and token length.
- Operation state is tracked by `OperationRegistry`; cancellation scopes are
  operation, subtree, group, and session.
- Cache behavior exposes object identity, lease expiry, version mismatch, and
  dependency invalidation as protocol-level primitives without exposing model
  private cache layouts.
- Schema/profile binding is registry-driven. `profile_id = 0` means
  `unspecified`; it is not a tensor default. `tensor` and `token` are peer
  first-round standard profiles.
- `structured_event` and `tool_delta` remain payload families interpreted through
  the registry. They are not promoted into standalone public profiles.

## FFI Boundary

`nnrp-ffi` exposes a stable C-compatible surface for downstream bindings:

- Handles are value handles with `kind`, `id`, and `generation`; host SDKs own
  language-specific object wrappers.
- Buffer views are borrowed views. Non-empty buffers must provide non-null
  pointers, and the callee does not retain them after the call returns.
- Event delivery supports single-event polling, bounded batch polling, and
  callback shapes. Callback receivers must not retain event pointers after the
  callback returns.
- Status values carry an FFI status code, a protocol error family, and an
  optional protocol error code so Python/C# can map errors without inventing new
  protocol categories.

The current FFI surface is backed by the Rust runtime handles for connection,
session, submit/result, control, polling, and close paths. Transport-provider
packaging, custom provider injection, and downstream host integration continue
to be tracked in `doc/todo/v1-preview3/06-client-server-runtime.md` and
`doc/todo/v1-preview3/06a-transport-provider-packaging.md`.

## Conformance Workflow

The suite-owned `nnrp-conformance` repository owns preview3 golden vectors,
fixture manifests, and adapter execution plans. Downstream SDKs should consume
that baseline and run their own adapters against the shared plan/result JSON
shape.

Rust reserves this adapter wrapper:

```text
cargo run -p nnrp-conformance --bin nnrp-conformance-adapter -- --plan <path> --output <path>
```

Python, C#, and future bindings should keep repo-local wrapper names but consume
the same case IDs and result schema.

## Retired Preview-Era Assumptions

- Preview compatibility shims must not be restored in `nnrp-rs`; current work is
  the `NNRP/1` path.
- Historical preview1/preview2 behavior that is still part of preview3 is
  inherited explicitly through Rust-owned core modules and conformance fixtures.
- SDK-specific retry loops, cache semantics, schema binding, and recovery state
  machines should move behind Rust-owned handles or fixture-validated bindings.
- Feature negotiation uses the current `NNRP/1` version and wire-format window.
  Legacy planning assumptions are documentation context, not runtime fallbacks.
