# Rust Preview3 Conformance And Binding Rollout

- [x] Keep `nnrp-conformance` responsible for golden vectors, fixture manifests, cross-language conformance exports, and protocol regression baselines.
- [ ] Export preview3 canonical golden vectors from Rust.
- [ ] Export cross-language fixture manifests for Python/C# binding tests.
- [x] Publish the binding-consumption contract for Python and C#: Rust reserves `cargo run -p nnrp-conformance --bin nnrp-conformance-adapter -- --plan <path> --output <path>` as its own adapter wrapper, while downstream SDKs keep ownership of their repo-local wrapper names and bootstrap over the shared plan/result JSON.
- [x] Add the initial `nnrp-conformance-adapter` wrapper so it can read the suite-owned execution plan and emit a schema-valid explicit `not_implemented` case-result report.
- [ ] Implement real preview3 adapter case execution inside `nnrp-conformance-adapter` once the shared adapter-execution path is enabled.
- [ ] Document feature negotiation rules for the current `NNRP/1` path versus legacy planning assumptions, without restoring preview compatibility shims.