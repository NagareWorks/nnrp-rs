# Native Link Library Packaging

`nnrp-ffi` builds an `rlib` for Rust consumers and a native link library for host runtimes. The native artifacts are the intended integration point for Python, C#, Unity, Node.js native addons, Electron, and local agent services that want lower overhead than WASM.

## Build And Package

Use the packaging script after building the workspace, or let the script build `nnrp-ffi` itself:

```bash
python scripts/package_native_artifacts.py --out artifacts/native
```

The default script writes transport-scoped platform directories for `tcp`, `quic`, `ipc`, and `websocket`. Each directory contains:

- `nnrp_ffi.dll` on Windows, `libnnrp_ffi.so` on Linux, or `libnnrp_ffi.dylib` on macOS.
- `include/nnrp/nnrp.h`, the umbrella C/C++ ABI header consumed by native loaders and generated bindings.
- `manifest.json`, including the package name, transport scope, transport slots, protocol version, ABI version, enabled Rust features, profile, OS, architecture, library file, header files, and required exported symbols.

The same script verifies exported `nnrp_*` symbols before packaging. CI runs it on Windows, Linux, and macOS so platform-specific library names stay pinned.

## Transport Scope

Release artifacts are scoped by transport:

| Transport scope | Manifest package | Transport slots | Rust feature |
|---|---|---|---|
| `tcp` | `nnrp-ffi-transport-tcp` | `["tcp"]` | `transport-tcp` |
| `quic` | `nnrp-ffi-transport-quic` | `["quic"]` | `transport-quic` |
| `ipc` | `nnrp-ffi-transport-ipc` | `["ipc"]` | `transport-ipc` |
| `websocket` | `nnrp-ffi-transport-websocket` | `["websocket"]` | `transport-websocket` |

Use `--transport-scope <name>` to build one scope during local debugging. Release CI uses the default scoped matrix and then runs:

```bash
python scripts/inspect_release_artifacts.py --native-dir artifacts/native
```

The inspection step rejects release artifacts that advertise every transport from one hidden package.

## Node.js Loading Guidance

`nnrp-js` backend packages should treat Node.js differently from browsers:

1. Prefer the native link library when the platform package is available.
2. Validate `manifest.json` before loading so the Node wrapper can reject mismatched native artifacts early.
3. Load the transport-scoped library through a native addon or FFI loader owned by `nnrp-js`.
4. Fall back only when native loading is unavailable or explicitly disabled.
5. Keep browser builds on `nnrp-wasm-browser`; browsers cannot load `.dll`, `.so`, `.dylib`, or native static-library artifacts.

This repository does not own npm package layout or bundler adapters. It owns the Rust-generated native/WASM primitives and the ABI contract that downstream SDKs consume.
