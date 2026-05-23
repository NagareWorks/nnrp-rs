# Rust Preview3 FFI Surface

- [x] Keep `nnrp-ffi` responsible for stable ABI, handle lifecycle, callback/polling surfaces, and cross-language buffer ownership rules.
- [x] Define stable ABI-safe handle layouts and ownership rules.
- [x] Expose connection bootstrap, session open/patch/close, submit, result/event pump, and control operations through FFI.
- [x] Expose zero-copy or bounded-copy buffer-view APIs suitable for Python and C# bindings.
- [x] Expose callback-driven and polling-driven event delivery surfaces.
- [x] Expose stable preview3 error codes and diagnostics to binding layers.
