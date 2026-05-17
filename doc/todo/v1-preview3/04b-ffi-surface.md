# Rust Preview3 FFI Surface

- [ ] Keep `nnrp-ffi` responsible for stable ABI, handle lifecycle, callback/polling surfaces, and cross-language buffer ownership rules.
- [ ] Define stable ABI-safe handle layouts and ownership rules.
- [ ] Expose connection bootstrap, session open/patch/close, submit, result/event pump, and control operations through FFI.
- [ ] Expose zero-copy or bounded-copy buffer-view APIs suitable for Python and C# bindings.
- [ ] Expose callback-driven and polling-driven event delivery surfaces.
- [ ] Expose stable preview3 error codes and diagnostics to binding layers.