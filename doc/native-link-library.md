# Native Link Library Packaging

`nnrp-ffi` builds an `rlib` for Rust consumers and a `cdylib` for native host runtimes. The native artifact is the intended integration point for Python, C#, Unity, Node.js native addons, Electron, and local agent services that want lower overhead than WASM.

## Build And Package

Use the packaging script after building the workspace, or let the script build `nnrp-ffi` itself:

```bash
python scripts/package_native_artifacts.py --out artifacts/native
```

The script writes one platform directory containing:

- `nnrp_ffi.dll` on Windows, `libnnrp_ffi.so` on Linux, or `libnnrp_ffi.dylib` on macOS.
- `nnrp_ffi.h`, the C ABI header consumed by native loaders and generated bindings.
- `manifest.json`, including the package name, profile, OS, architecture, library file, header file, and required exported symbols.

The same script verifies exported `nnrp_*` symbols before packaging. CI runs it on Windows, Linux, and macOS so platform-specific library names stay pinned.

## Node.js Loading Guidance

The future `nnrp-js` package should treat Node.js differently from browsers:

1. Prefer the native link library when the platform package is available.
2. Validate `manifest.json` before loading so the Node wrapper can reject mismatched native artifacts early.
3. Load the library through a native addon or FFI loader owned by `nnrp-js`.
4. Fall back to WASM when native loading is unavailable or explicitly disabled.
5. Keep browser builds on WASM plus web transports only; browsers cannot load `.dll`, `.so`, or `.dylib` artifacts.

This repository does not own npm package layout, bundler adapters, or WebSocket/WebTransport adapters. It owns the Rust-generated native/WASM primitives and the ABI contract that `nnrp-js` consumes.
