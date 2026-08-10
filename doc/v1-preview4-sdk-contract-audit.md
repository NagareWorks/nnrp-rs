# Preview4 SDK Contract Audit

This audit binds the Preview4 machine contract to the Rust runtime and downstream SDK release
surface. The canonical contract is
`nnrp-doc/docs/public/contracts/nnrp-1-preview4-sdk-api.json` at contract version 12.

## Runtime Event Envelope

| Contract item | Rust implementation | Executable evidence |
| --- | --- | --- |
| Complete non-derived common header | `nnrp_runtime::RuntimeFrameHeader` | runtime packet and batch header-preservation tests |
| Closed metadata union | `nnrp_runtime::NnrpRuntimeEventMetadata` | client/server loopbacks and external wire conformance |
| Closed owned tail union | `nnrp_runtime::NnrpRuntimeEventTail` | object delta, diagnostic, partial, result, and cache cases |
| One role-neutral wire event | `nnrp_runtime::NnrpRuntimeEvent` | client, transport, FFI, and WASM tests |
| Submit operation ownership | `nnrp_runtime::NnrpServerOperation` | server event-pump and selective-receive tests |
| Closed server event union | `nnrp_runtime::NnrpServerEvent` | submit, runtime, and lifecycle variant tests |
| Headerless lifecycle event | `nnrp_runtime::OperationLifecycleEvent` | runtime event-pump and native FFI role-carrier E2E tests |
| Closed terminal evidence union | `nnrp_runtime::NnrpTerminalEvent` | runtime and lifecycle variant tests |
| Correlated terminal result | `nnrp_runtime::NnrpResult` | runtime decoder and public API tests |

Private role decoders remain implementation details. The public server event pump returns
`NnrpServerEvent`: `FRAME_SUBMIT` becomes `NnrpServerOperation`, other wire messages retain the
role-neutral runtime envelope, and headerless local state uses the lifecycle variant. The FFI
transports one complete encoded metadata-plus-tail payload and the full header in one coarse poll
result; it does not add per-field or per-frame boundary calls.

`NnrpServerSession::await_event` is the canonical ordered receive operation.
`NnrpServerSession::receive_submit` is selective convenience only: it retains skipped events in the
session queue and cannot decode-and-forget control, object, cache, or lifecycle evidence. A server
operation owns the complete submit event and both wire identities until the application sends one
terminal outcome or closes the session.

`NnrpResult.event` is a closed `Runtime | Lifecycle` union. Wire results retain the complete
`NnrpRuntimeEvent`; local completion, cancellation, supersession, and failure retain the exact
`OperationLifecycleEvent`. A non-terminal lifecycle state cannot construct a result, and no local
state is converted into a zero-filled `RuntimeFrameHeader`.

## Coordinated Baseline

The coordinated baseline is Rust `1.0.0-preview.4.22` with native FFI ABI `4.4.0`. Python,
JavaScript, and C# releases must each prove all of the following against that exact baseline:

1. Public names and field ownership match the frozen language projection.
2. Every runtime-event message maps to the frozen metadata and tail variants for the receiving role.
3. Wire headers are preserved without inferred defaults.
4. Local lifecycle events cannot enter the public wire-event type.
5. Native provider packages load their own transport-scoped artifact and pass role E2E.
6. Browser packages contain only browser-safe WASM and WebSocket substrate behavior.
7. Public API parity and wire conformance both pass; neither substitutes for the other.
8. Canonical server event pumps preserve event order and selective submit receives retain every
   skipped event.

Benchmark results are recorded for regression analysis but do not replace correctness, API parity,
wire conformance, or artifact-boundary gates.
