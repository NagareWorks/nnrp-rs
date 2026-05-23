# Rust Preview3 Foundation And Contract

## Repository Boundary

- [x] Create the initial Cargo workspace for `nnrp-core`, `nnrp-ffi`, and `nnrp-conformance`.
- [x] Lock the repository role: Rust is the canonical preview3 implementation source; other SDKs are binding/integration layers.
- [x] Finalize crate ownership boundaries as the inherited NNRP/1 contract and preview3 extensions turn into code.

## Protocol Contract Landing

- [x] Land the frozen connection/session lifecycle and explicit session-close contract in `nnrp-core`; recovery contract remains tracked in `02c`.
- [x] Land inherited `FLOW_UPDATE` metadata plus preview3 priority, operation-state, cancel-scope, and multi-scope scheduling contract in `nnrp-core`.
- [x] Land inherited cache/typed-payload pieces plus preview3 cache lease, schema registry, and descriptor-binding contract in `nnrp-core`.
- [ ] Land the frozen payload-family and public lifecycle boundary in `nnrp-core`.

## FFI And Downstream Consumption

- [ ] Land the frozen handle families, lifecycle rules, callback/polling model, and buffer-view contract in `nnrp-ffi`.
- [ ] Land stable preview3 error families and downstream mapping guidance in `nnrp-ffi`.
- [ ] Export Rust-generated conformance fixtures as the only canonical preview3 baseline for downstream SDKs.
