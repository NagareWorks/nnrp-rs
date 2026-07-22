<p align="center">
  <img src="https://raw.githubusercontent.com/NagareWorks/nnrp-rs/main/assets/nnrp-readme-banner.svg" alt="NNRP - Neural Network Runtime Protocol" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/NagareWorks/nnrp-rs/actions"><img alt="CI" src="https://img.shields.io/badge/CI-preview4-22c55e"></a>
  <a href="https://www.rust-lang.org"><img alt="Rust 1.82+" src="https://img.shields.io/badge/Rust-1.82%2B-f97316?logo=rust&logoColor=white"></a>
  <a href="https://nagareworks.github.io/nnrp-doc/"><img alt="Docs" src="https://img.shields.io/badge/docs-nnrp--doc-38bdf8"></a>
  <a href="https://github.com/NagareWorks/nnrp-rs/blob/main/LICENSE"><img alt="Apache-2.0" src="https://img.shields.io/badge/license-Apache--2.0-64748b"></a>
  <img alt="Native FFI" src="https://img.shields.io/badge/native-FFI-0f766e">
  <img alt="WASM primitives" src="https://img.shields.io/badge/WASM-primitives-b45309">
</p>

# nnrp-rs

`nnrp-rs` is the Rust canonical SDK workspace for NNRP/1 Preview4. NNRP is a domain-level application-layer protocol for long-lived, real-time AI model runtime communication: session lifecycle, flow control, runtime control frames, runtime object references, cache/schema negotiation, recovery, transport selection, and typed payload exchange live above TCP, QUIC, IPC, and WebSocket transports.

This repository is intended to be the implementation source for Rust users and for downstream language bindings.

## What Ships Here

| Package | Purpose |
|---|---|
| `nnrp-core` | Wire codecs, strict validation, protocol enums/errors, lifecycle state machines, runtime control frames, runtime object descriptors, cache/schema semantics, recovery, and conformance-facing core types. |
| `nnrp-runtime` | Transport-neutral async client/server session runtime over framed transport slots. |
| `nnrp-transport-provider` | Provider registry, local/remote capability intersection, native library discovery, policy resolution, and probe-score selection. |
| `nnrp-transport-tcp` | TCP provider package for runtime transport/listener slots. |
| `nnrp-transport-quic` | Default Quinn/Rustls QUIC provider, certificate config helpers, and injection hooks for custom backends. |
| `nnrp-transport-ipc` | Local IPC provider for same-node schedulers, local agents, and host-side runtime services. |
| `nnrp-transport-websocket` | Native WebSocket provider for browser-compatible edge paths and WebSocket service endpoints. |
| `nnrp-ffi` | C-compatible ABI facade, handle/event model, header surface, and transport-scoped native link-library packaging. |
| `nnrp-wasm` | Browser WASM primitives and TypeScript declarations for WebSocket-substrate JS/TS wrappers. |
| `nnrp-conformance` | Suite-facing adapter wrapper, wire conformance dry-runner, fixture-backed validation, and protocol regression helpers. |

## Install

For a Rust client/server application using the runtime plus TCP transport:

```powershell
cargo add nnrp-core@1.0.0-preview.4.14 nnrp-runtime@1.0.0-preview.4.14 nnrp-transport-tcp@1.0.0-preview.4.14
cargo add tokio --features macros,rt-multi-thread,net,io-util,time
```

Add optional packages only when your application needs them:

```powershell
cargo add nnrp-transport-quic@1.0.0-preview.4.14
cargo add nnrp-transport-ipc@1.0.0-preview.4.14
cargo add nnrp-transport-websocket@1.0.0-preview.4.14
cargo add nnrp-transport-provider@1.0.0-preview.4.14
cargo add nnrp-ffi@1.0.0-preview.4.14
cargo add nnrp-wasm@1.0.0-preview.4.14
```

Equivalent `Cargo.toml` form:

```toml
[dependencies]
nnrp-core = "1.0.0-preview.4.14"
nnrp-runtime = "1.0.0-preview.4.14"
nnrp-transport-tcp = "1.0.0-preview.4.14"

# Optional packages
nnrp-transport-provider = "1.0.0-preview.4.14"
nnrp-transport-quic = "1.0.0-preview.4.14"
nnrp-transport-ipc = "1.0.0-preview.4.14"
nnrp-transport-websocket = "1.0.0-preview.4.14"
nnrp-ffi = "1.0.0-preview.4.14"
nnrp-wasm = "1.0.0-preview.4.14"
```

For repository builds before publishing:

```toml
[dependencies]
nnrp-runtime = { git = "https://github.com/NagareWorks/nnrp-rs", package = "nnrp-runtime" }
nnrp-transport-tcp = { git = "https://github.com/NagareWorks/nnrp-rs", package = "nnrp-transport-tcp" }
```

## Runtime Shape

```rust
use nnrp_runtime::{NnrpClient, NnrpClientConfig, RuntimeTransportKind};

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let config = NnrpClientConfig::default().with_transport(RuntimeTransportKind::Tcp);
let client = NnrpClient::connect_tcp("127.0.0.1:4433", config).await?;
let session = client.open_session().await?;
# let _ = session;
# Ok(())
# }
```

TCP, QUIC, IPC, and WebSocket are provider packages. Install only the transports the host needs; when multiple transports are present, the provider registry can probe and select from the claimed transport capabilities instead of treating the packages as configuration flags over hidden implementation elsewhere.

Preview4 adds runtime control and object-reference surfaces for workloads where cancellation, priority changes, progress, partial results, cache references, route hints, trace context, and result drop reasons need protocol-level representation.

## Native And WASM Artifacts

Native link libraries are for C#/Python/Unity and Node.js backend native-addon scenarios:

```powershell
python scripts\package_native_artifacts.py --out artifacts\native
```

Native artifacts are transport-scoped. The default packaging script emits `tcp`, `quic`, `ipc`, and `websocket` package directories, each with a manifest that declares one transport slot and its enabled Rust feature set. Release CI rejects native artifacts that collapse every transport into one hidden package.

Each native artifact includes `include/nnrp/nnrp.h` as the C/C++ umbrella header, plus the FFI/runtime/error/version headers. Release CI packages Windows, Linux, macOS, Android, and iOS targets, including 32-bit x86/ARM variants where Rust and the platform toolchain expose supported targets. Desktop and Android packages ship dynamic libraries; iOS packages ship static libraries for app/toolchain linking.

Browser WASM artifacts are browser-scoped and WebSocket-substrate only. Native TCP, QUIC, and IPC libraries are not packaged into browser artifacts. Node.js should probe native transport packages first and fall back only when the host intentionally chooses a WASM-capable path.

```powershell
rustup target add wasm32-unknown-unknown
python scripts\package_wasm_primitives.py --out artifacts\wasm
```

## Workspace Layout

- `crates/`: Rust crates listed above.
- `include/nnrp/`: C ABI headers for native consumers.
- `scripts/`: native and WASM packaging helpers.
- `doc/todo/`: Preview implementation planning and rollout checklists.

## Quality Gates

Before commits, the Rust workspace is expected to pass:

```powershell
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo llvm-cov --workspace --lcov --output-path target\llvm-cov\lcov.info
```

The current project rule is 90%+ total line coverage and 90%+ incremental line coverage for every commit.

Preview4 release validation can also run the Rust-owned benchmark entrypoint:

```powershell
cargo run -p nnrp-conformance --bin nnrp-preview4-benchmarks -- --iterations 100000 --transport-iterations 1000
```

The benchmark report covers control-frame encode/decode, runtime object declare/ref/release metadata, IPC loopback, and WebSocket loopback.

## Documentation

- Protocol and SDK docs: <https://nagareworks.github.io/nnrp-doc/>
- Rust SDK docs: <https://nagareworks.github.io/nnrp-doc/en/sdk/rust/>
- Conformance docs: <https://nagareworks.github.io/nnrp-doc/en/conformance/>
- Native link library packaging: [`doc/native-link-library.md`](doc/native-link-library.md)
- Browser WASM packaging: [`doc/browser-wasm.md`](doc/browser-wasm.md)
- Wire conformance CLI: [`doc/wire-conformance.md`](doc/wire-conformance.md)
- Preview4 release notes: [`doc/v1-preview4-release-notes.md`](doc/v1-preview4-release-notes.md)
- Downstream SDK checklist: [`doc/v1-preview4-downstream-sdk-checklist.md`](doc/v1-preview4-downstream-sdk-checklist.md)

## License

Apache-2.0. See [LICENSE](LICENSE).

## Contributors

Thanks to everyone shaping NNRP. See the
[contributors graph](https://github.com/NagareWorks/nnrp-rs/graphs/contributors)
for individual GitHub profiles and contribution history.
