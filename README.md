# nnrp-rs

Rust canonical SDK workspace for NNRP preview3.

This repository is intended to become the single implementation source for:

1. Wire codecs and strict protocol validation.
2. Connection/session state machines.
3. Cache/schema lifecycle semantics.
4. Stable FFI for Python, C#, and future language bindings.
5. Golden vectors and conformance fixtures.

## Workspace Layout

- `crates/nnrp-core`: wire primitives, protocol models, validation, and state-machine-facing core types.
- `crates/nnrp-ffi`: stable FFI facade over the Rust core for host language bindings.
- `crates/nnrp-conformance`: shared golden vectors and conformance fixture export surface.
- `doc/todo/`: implementation planning and rollout checklists.

## Current Status

This repository currently contains the initial workspace skeleton only. The preview3 protocol design now lives in `nnrp-doc/docs/developers/design/v1-preview3.md` while Rust becomes the canonical implementation source.
