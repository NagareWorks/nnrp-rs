# Preview4 Design-To-Code Audit

This audit records the release evidence for the frozen Preview4 host-route contract. It compares the protocol-facing
Rust API, the native and WASM artifact surfaces, and the three downstream SDK implementations available at the time of
the `1.0.0-preview.4.18` release. Carrier-local APIs remain singular by design; logical client and server host APIs own
transport-keyed route sets.

## Frozen Host-Route Contract

| Contract item | Rust implementation | Executable evidence |
| --- | --- | --- |
| Application endpoint remains `nnrp://` or `nnrps://` | `NnrpEndpoint` | `nnrp-runtime::route::tests` |
| A role owns at most one route per transport | `ClientProviderRoutes` and `ServerProviderRoutes` are `BTreeMap<TransportId, ...>` | client/server provider route tests and host-route conformance |
| A route owns its provider-local locator | `ClientProviderRoute::provider_endpoint`, `ServerProviderRoute::provider_endpoint` | route validation and native host-route cases |
| Client security is route-local | `ClientTransportSecurity { server_name, trusted_certificate_der }` | TCP TLS, QUIC, WSS, and route-security isolation cases |
| Server security is route-local | `ServerTransportSecurity { certificate_der, private_key_pkcs8_der }` | TCP TLS, QUIC, WSS, and route-security isolation cases |
| Client Auto/Prefer adopts one selected carrier | `connect_client_with_providers` | multi-route selection and forced-route failure cases |
| Server Auto/Prefer owns the listener set atomically | `listen_server_with_providers` | multi-listener bind, rollback, and terminal-listener failure cases |
| Runtime FFI remains coarse and singular per carrier | transport-scoped connect/listen handles feed session-scoped runtime frame calls | native library isolation and packet-batch loopback checks |

The exact rejection registry is implemented by `TransportRejectionReason` and projected through native FFI and WASM:

- `policy-disallowed`
- `local-unavailable`
- `peer-unsupported`
- `limit-exceeded`
- `route-unresolved`
- `security-unsatisfied`
- `probe-missing`
- `probe-failed`

Packaging validation rejects a native or WASM artifact whose published rejection registry, ABI, transport identity, or
role entry points differ from this contract.

## Downstream API Mapping

The release audit used the following downstream revisions. These revisions consume ABI `4.1.0`; they will update their
artifact pin from Preview4 revision 17 to revision 18 after this Rust release is published.

| SDK | Audited revision | Logical route-set API | Route-local API |
| --- | --- | --- | --- |
| Python | `7c2f3efcff5545dc1b08935da5b390614b2db075` | `provider_routes` on native client/server hosts | `NativeClientProviderRoute`, `NativeServerProviderRoute` |
| JavaScript | `777d4144fcc9e39962ad778a6487bebe9dca2a03` | `providerRoutes` on native/browser clients and native server | `NnrpClientProviderRoute`, `NnrpServerProviderRoute` |
| C# | `8b3cf91946f5a98370c6f44c047eb315fd0dd50b` | `ProviderRoutes` on `NnrpClientOptions` and `NnrpServerOptions` | `NnrpClientProviderRoute`, `NnrpServerProviderRoute` |

No audited production role exposes a singular route override. Singular provider endpoint parameters remain only on
one-provider transport APIs, per-route records, and carrier internals.

## Workstream Evidence

| Workstream | Release evidence |
| --- | --- |
| Runtime control protocol | core metadata round trips, semantic validation, runtime dispatch, FFI/WASM projection, Preview4 vectors |
| Runtime objects and cache references | object/cache metadata tests, lease lifecycle tests, native/WASM projection, hot-path benchmarks |
| IPC and WebSocket transports | Rust loopbacks, native ABI packet-batch loopbacks, secure WebSocket tests, browser WASM role tests |
| FFI and WASM artifacts | release-surface sync, helper tests, native library isolation, platform artifact inspection, WASM declaration inspection |
| Wire conformance | Preview4 baseline, external target roles, host-route client/server cases, known-uninstalled route case |
| Release validation | format, clippy, workspace tests, changed-line and project coverage, benchmark FFI build, native/WASM packaging |

The canonical local and CI entry points are `.github/workflows/ci.yml`, `.github/workflows/release.yml`,
`scripts/run_preview4_host_route_conformance.py`, `scripts/check_release_surface_sync.py`, and the release artifact
packaging and inspection scripts under `scripts/`.

## Release Decision

Preview4 revision 18 is one coordinated correction release. It closes the host-route cardinality, route-local security,
provider-identity evidence, host-route wire execution, and complete WASM rejection-registry contract together. No part
of that correction is deferred to a downstream SDK-specific exception.
