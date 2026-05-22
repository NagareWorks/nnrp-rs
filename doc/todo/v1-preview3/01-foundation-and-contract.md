# Rust Preview3 Foundation And Contract

## Repository Boundary

- [x] Create the initial Cargo workspace for `nnrp-core`, `nnrp-ffi`, and `nnrp-conformance`.
- [x] Lock the repository role: Rust is the canonical preview3 implementation source; other SDKs are binding/integration layers.
- [x] Finalize crate ownership boundaries as the frozen preview3 contract turns into code.

## Protocol Contract Landing

- [ ] Land the frozen connection/session lifecycle, explicit session-close, and recovery contract in `nnrp-core`.
- [x] Land the frozen priority, operation-state, cancel-scope, and `FLOW_UPDATE` metadata/enum contract in `nnrp-core`.
- [ ] Land the frozen cache lease, schema registry, and typed payload descriptor contract in `nnrp-core`.
- [ ] Land the frozen payload-family and public lifecycle boundary in `nnrp-core`.

## FFI And Downstream Consumption

- [ ] Land the frozen handle families, lifecycle rules, callback/polling model, and buffer-view contract in `nnrp-ffi`.
- [ ] Land stable preview3 error families and downstream mapping guidance in `nnrp-ffi`.
- [ ] Export Rust-generated conformance fixtures as the only canonical preview3 baseline for downstream SDKs.
