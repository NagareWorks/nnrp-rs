# Rust Preview3 Validation And Docs

## Validation

- [ ] Keep `cargo test` green across the workspace.
- [ ] Add fixture-driven tests for all frozen preview3 enum/message/error values.
- [ ] Add integration tests for multi-session orchestration on one connection.
- [ ] Add validation for cache lease expiry, schema mismatch, cancellation, priority-aware flow updates, and resume behavior.
- [ ] Add FFI smoke tests that prove Python/C# can bind without redefining protocol semantics.
- [ ] Add ABI compatibility tests and fixture-driven smoke coverage.

## Documentation

- [ ] Document the workspace layout and crate ownership boundaries.
- [ ] Document the frozen preview3 connection/session, cache, schema, and operation lifecycle semantics.
- [ ] Document the FFI contract and binding responsibilities.
- [ ] Document the conformance workflow for Python, C#, and future bindings.
- [ ] Document which preview2 semantics remain compatibility-only and which must move fully into preview3 Rust handles.