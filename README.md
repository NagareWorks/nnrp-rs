<p align="center">
  <img src="https://raw.githubusercontent.com/NagareWorks/nnrp-rs/main/assets/nnrp-readme-banner.svg" alt="NNRP - Neural Network Runtime Protocol" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/NagareWorks/nnrp-rs/actions"><img alt="CI" src="https://img.shields.io/badge/CI-preview3-22c55e"></a>
  <a href="https://www.rust-lang.org"><img alt="Rust 1.82+" src="https://img.shields.io/badge/Rust-1.82%2B-f97316?logo=rust&logoColor=white"></a>
  <a href="https://nagareworks.github.io/nnrp-doc/"><img alt="Docs" src="https://img.shields.io/badge/docs-nnrp--doc-38bdf8"></a>
  <a href="https://github.com/NagareWorks/nnrp-rs/blob/main/LICENSE"><img alt="Apache-2.0" src="https://img.shields.io/badge/license-Apache--2.0-64748b"></a>
  <img alt="Native FFI" src="https://img.shields.io/badge/native-FFI-0f766e">
  <img alt="WASM primitives" src="https://img.shields.io/badge/WASM-primitives-b45309">
</p>

# nnrp-rs

`nnrp-rs` is the Rust canonical SDK workspace for NNRP Preview3. NNRP is a domain-level application-layer protocol for long-lived, real-time AI model runtime communication: session lifecycle, flow control, cache/schema negotiation, recovery, transport selection, and typed payload exchange live above TCP/QUIC/Web transports.

This repository is intended to be the implementation source for Rust users and for downstream language bindings.

## What Ships Here

| Package | Purpose |
|---|---|
| `nnrp-core` | Wire codecs, strict validation, protocol enums/errors, lifecycle state machines, cache/schema semantics, recovery, and conformance-facing core types. |
| `nnrp-runtime` | Transport-neutral async client/server session runtime over framed transport slots. |
| `nnrp-transport-provider` | Provider registry, local/remote capability intersection, native library discovery, policy resolution, and probe-score selection. |
| `nnrp-transport-tcp` | TCP provider package for runtime transport/listener slots. |
| `nnrp-transport-quic` | Default Quinn/Rustls QUIC provider, certificate config helpers, and injection hooks for custom backends. |
| `nnrp-ffi` | C-compatible ABI facade, handle/event model, header surface, and native link-library packaging. |
| `nnrp-wasm` | Low-level WASM primitives and TypeScript declarations for future `nnrp-js` wrappers. |
| `nnrp-conformance` | Suite-facing adapter wrapper, fixture-backed validation, and protocol regression helpers. |

## Install

For a Rust client/server application using the runtime plus TCP transport:

```powershell
cargo add nnrp-core@1.0.0-preview.3.2 nnrp-runtime@1.0.0-preview.3.2 nnrp-transport-tcp@1.0.0-preview.3.2
cargo add tokio --features macros,rt-multi-thread,net,io-util,time
```

Add optional packages only when your application needs them:

```powershell
cargo add nnrp-transport-quic@1.0.0-preview.3.2
cargo add nnrp-transport-provider@1.0.0-preview.3.2
cargo add nnrp-ffi@1.0.0-preview.3.2
cargo add nnrp-wasm@1.0.0-preview.3.2
```

Equivalent `Cargo.toml` form:

```toml
[dependencies]
nnrp-core = "1.0.0-preview.3.2"
nnrp-runtime = "1.0.0-preview.3.2"
nnrp-transport-tcp = "1.0.0-preview.3.2"

# Optional packages
nnrp-transport-provider = "1.0.0-preview.3.2"
nnrp-transport-quic = "1.0.0-preview.3.2"
nnrp-ffi = "1.0.0-preview.3.2"
nnrp-wasm = "1.0.0-preview.3.2"
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

TCP is available as a provider package. QUIC is also available out of the box through `nnrp-transport-quic` using Quinn/Rustls, while the runtime still exposes framed transport/listener slots for deployments that need native, WASM-facing, or platform-specific QUIC backends.

## Native And WASM Artifacts

Native link libraries are for C#/Python/Unity and Node.js backend native-addon scenarios:

```powershell
python scripts\package_native_artifacts.py --out artifacts\native
```

Native artifacts include `include/nnrp/nnrp.h` as the C/C++ umbrella header,
plus `nnrp_ffi.h`, `nnrp_error.h`, `nnrp_runtime.h`, and `nnrp_version.h`.
Release CI packages Windows, Linux, macOS, Android, and iOS targets, including
32-bit x86/ARM variants where Rust and the platform toolchain expose supported
targets. Desktop and Android packages ship dynamic libraries; iOS packages ship
static libraries for app/toolchain linking.

WASM primitives are for future `nnrp-js` wrapping. Node.js should probe native libraries first and fall back to WASM when native loading is unavailable; browsers consume WASM plus WebSocket/WebTransport adapters from the JS/TS layer.

```powershell
rustup target add wasm32-unknown-unknown
python scripts\package_wasm_primitives.py --out artifacts\wasm
```

## Workspace Layout

- `crates/`: Rust crates listed above.
- `include/nnrp/`: C ABI headers for native consumers.
- `scripts/`: native and WASM packaging helpers.
- `doc/todo/`: Preview3 implementation planning and rollout checklists.

## Quality Gates

Before commits, the Rust workspace is expected to pass:

```powershell
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo llvm-cov --workspace --lcov --output-path target\llvm-cov\lcov.info
```

The current project rule is 90%+ total line coverage and 90%+ incremental line coverage for every commit.

## Documentation

- Protocol and SDK docs: <https://nagareworks.github.io/nnrp-doc/>
- Rust SDK docs: <https://nagareworks.github.io/nnrp-doc/en/sdk/rust/>
- Conformance docs: <https://nagareworks.github.io/nnrp-doc/en/conformance/>

## License

Apache-2.0. See [LICENSE](LICENSE).

## Contributors

Thanks to everyone shaping NNRP. See the
[contributors graph](https://github.com/NagareWorks/nnrp-rs/graphs/contributors)
for individual GitHub profiles and contribution history.
