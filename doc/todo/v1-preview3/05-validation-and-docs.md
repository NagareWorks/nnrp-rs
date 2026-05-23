# Rust Preview3 Validation And Docs

## Validation

- [x] Keep `cargo test` green across the workspace.
- [x] Add fixture-driven tests for all frozen preview3 enum/message/error values.
- [x] Add integration tests for multi-session orchestration on one connection.
- [x] Add validation for cache lease expiry, schema mismatch, cancellation, priority-aware flow updates, and resume behavior.
- [x] Add FFI smoke tests that prove Python/C# can bind without redefining protocol semantics.
- [x] Add ABI compatibility tests and fixture-driven smoke coverage.

## Documentation

- [x] Document the workspace layout and crate ownership boundaries.
- [x] Document the frozen preview3 connection/session, cache, schema, and operation lifecycle semantics.
- [x] Document the FFI contract and binding responsibilities.
- [x] Document the conformance workflow for Python, C#, and future bindings.
- [x] Document which historical preview-era semantics are retired and which must move fully into Rust-owned handles.
