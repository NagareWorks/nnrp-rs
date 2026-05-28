# Rust Preview3 FFI Surface

- [x] Keep `nnrp-ffi` responsible for stable ABI, handle lifecycle, callback/polling surfaces, and cross-language buffer ownership rules.
- [x] Define stable ABI-safe handle layouts and ownership rules.
- [x] Expose connection bootstrap, session open/patch/close, submit, result/event pump, and control operations through FFI.
- [x] Expose zero-copy or bounded-copy buffer-view APIs suitable for Python and C# bindings.
- [x] Expose callback-driven and polling-driven event delivery surfaces.
  - [x] Expose bounded batch polling for downstream SDK hot paths.
- [x] Expose stable preview3 error codes and diagnostics to binding layers.
- [x] Expose a stable runtime capability probe for ABI version, protocol version, transport slots, and feature flags.
- [x] Expose schema descriptor and typed payload descriptor helpers for downstream SDK bindings.
- [x] Expose recovery and migration validation helpers so downstream SDKs can reuse Rust's canonical resume/replay rules.
- [x] Expose client-side completion/drop/control aliases so downstream SDKs can benchmark and validate glued runtime paths without binding server-named helpers.
- [x] Expose coarse client submit/result helper so downstream SDKs can collapse hot submit-complete-poll loops into one ABI call.
- [x] Wire FFI entrypoints to the real client/server runtime once `06` lands.
