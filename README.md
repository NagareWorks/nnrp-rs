# nnrp-rs

Rust canonical SDK workspace for NNRP preview3.

This repository is intended to become the single implementation source for:

1. Wire codecs and strict protocol validation.
2. Connection/session state machines.
3. Cache/schema lifecycle semantics.
4. Stable FFI for Python, C#, and future language bindings.
5. Golden vectors and conformance fixtures.

## Contributors

<a href="https://github.com/NagareWorks/nnrp-rs/graphs/contributors" title="Open the contributors graph for individual GitHub profiles and IDs.">
	<img src="https://contrib.rocks/image?repo=NagareWorks/nnrp-rs" alt="Contributors" />
</a>

The avatar wall above updates automatically from the repository contributor list once this repository is published at the matching GitHub location.

GitHub README rendering does not support per-avatar dynamic tooltips for an auto-generated contributor wall, so use the linked contributors graph if you want individual profile pages and account IDs.

## Workspace Layout

- `crates/nnrp-core`: canonical NNRP/1 wire primitives, preview3 extension models, strict validation, state-machine-facing core types, and host-neutral cache/schema semantics.
- `crates/nnrp-runtime`: transport-neutral client/server session runtime over framed async transport slots.
- `crates/nnrp-transport-provider`: provider registry, local transport discovery helpers, and transport policy selection.
- `crates/nnrp-transport-tcp`: TCP provider package for the runtime transport/listener slots.
- `crates/nnrp-ffi`: stable ABI facade over `nnrp-core`, including handle ownership, buffer views, callbacks, polling, downstream error mapping, and native `cdylib` packaging.
- `crates/nnrp-wasm`: low-level WASM primitives and TypeScript declarations consumed by the future `nnrp-js` wrapper.
- `crates/nnrp-conformance`: Rust-owned golden vectors, fixture manifests, adapter wrappers, and cross-language conformance export surface.
- `include/nnrp/`: C ABI headers and native link-library packaging surface for downstream loaders.
- `scripts/package_native_artifacts.py`: builds, verifies, and packages `nnrp-ffi` native artifacts for Windows, Linux, and macOS hosts.
- `scripts/package_wasm_primitives.py`: builds and packages `nnrp-wasm` plus the minimal `.d.ts` surface for downstream JS/TS wrappers.
- `doc/todo/`: implementation planning and rollout checklists.

## Current Status

The workspace has moved past the initial skeleton. `nnrp-core` now owns the inherited NNRP/1 common header, inherited `FLOW_UPDATE` metadata contract, preview3 session lifecycle metadata, host-neutral connection/session lifecycle state, schema/payload descriptors, protocol enums, and cache/schema error code constants. The preview3 protocol design remains in `nnrp-doc/docs/developers/design/v1-preview3.md`, while this repository is the canonical implementation source consumed by downstream SDKs.
