# Rust Preview3 Implementation Surface

## Crate Ownership

- [ ] Keep `nnrp-core` responsible for wire primitives, protocol validation, state-machine core types, cache/schema semantics, and host-neutral logic.
- [ ] Keep `nnrp-ffi` responsible for stable ABI, handle lifecycle, callback/polling surfaces, and cross-language buffer ownership rules.
- [ ] Keep `nnrp-conformance` responsible for golden vectors, fixture manifests, cross-language conformance exports, and protocol regression baselines.

## nnrp-core

- [ ] Implement preview3 typed payload descriptors, extension descriptors, and schema/profile binding rules.
- [ ] Implement strict validation for illegal lifecycle, cache, and schema combinations.

## nnrp-ffi

- [ ] Define stable ABI-safe handle layouts and ownership rules.
- [ ] Expose connection bootstrap, session open/patch/close, submit, result/event pump, and control operations through FFI.
- [ ] Expose zero-copy or bounded-copy buffer-view APIs suitable for Python and C# bindings.
- [ ] Expose callback-driven and polling-driven event delivery surfaces.
- [ ] Expose stable preview3 error codes and diagnostics to binding layers.

## nnrp-conformance And Binding Rollout Support

- [ ] Export preview3 canonical golden vectors from Rust.
- [ ] Export cross-language fixture manifests for Python/C# binding tests.
- [ ] Publish a binding-consumption contract for Python and C#.
- [ ] Document feature negotiation rules for preview2 compatibility versus preview3 native paths.