# SDK Contract v14 Role Events

This corrective workstream closes the gap between the frozen Preview4 SDK API contract and the
Rust client and server roles consumed by native and browser bindings. It does not add compatibility
aliases for older Preview APIs.

- [x] Expose the client role event pump as the closed runtime-or-lifecycle union.
- [x] Preserve the union without fabricated wire headers in native FFI event projection.
- [x] Preserve the union without fabricated wire headers in the browser WASM event projection.
- [x] Make the SDK contract checker verify the Rust client method surface, not only the JSON shape.
- [x] Cover native and browser projections with executable lifecycle and runtime cases.
- [x] Expose the server role event pump as the closed submit-or-runtime-or-lifecycle union.
- [x] Keep `receive_submit` selective while retaining skipped events for the canonical event pump.
- [x] Make `NnrpServerOperation` own progress, partial-result, result, and drop methods.
- [x] Remove parallel application-facing operation reply methods from `NnrpServerSession`.
- [x] Reject cross-session operation use and operation-ID mismatches before writing a frame.
- [x] Freeze the complete Rust role-method projection and operation signatures in the machine contract.
- [x] Make the SDK contract checker verify operation signatures and ownership, not only type names.
- [x] Run the complete Rust CI workflow and downstream conformance gates before publishing artifacts.
