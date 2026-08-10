# SDK Contract v12 Client Events

This corrective workstream closes the gap between the frozen Preview4 SDK API contract and the
Rust client role consumed by native and browser bindings. It does not add compatibility aliases for
older Preview APIs.

- [x] Expose the client role event pump as the closed runtime-or-lifecycle union.
- [x] Preserve the union without fabricated wire headers in native FFI event projection.
- [x] Preserve the union without fabricated wire headers in the browser WASM event projection.
- [x] Make the SDK contract checker verify the Rust client method surface, not only the JSON shape.
- [x] Cover native and browser projections with executable lifecycle and runtime cases.
- [x] Run the complete Rust CI workflow and downstream conformance gates before publishing artifacts.
