# Wire Conformance CLI

`nnrp-conformance-wire` builds a suite-owned wire execution plan from a preview4 suite manifest and a target manifest. It is for direct wire-level validation: the suite can act as client, server, or proxy without going through an SDK adapter wrapper.

## Command

```bash
cargo run -p nnrp-conformance --bin nnrp-conformance-wire -- \
  --suite path/to/wire-conformance/nnrp-1-preview4/manifest.json \
  --target path/to/target.json \
  --output artifacts/wire-results.json
```

Run a subset with repeated `--case` arguments:

```bash
cargo run -p nnrp-conformance --bin nnrp-conformance-wire -- \
  --suite path/to/wire-conformance/nnrp-1-preview4/manifest.json \
  --target path/to/target.json \
  --output artifacts/wire-results.json \
  --case wire.cancel.ipc \
  --case wire.progress.websocket
```

## Target Manifest

Target manifests declare the protocol version, available wire endpoints, and capability names. Example:

```json
{
  "target_name": "nnrp-rs-reference",
  "protocol_version": "nnrp-1-preview4",
  "endpoints": [
    { "transport": "tcp", "mode": "server", "address": "127.0.0.1:4433" },
    { "transport": "ipc", "mode": "server", "address": "nnrp-preview4.sock" },
    { "transport": "websocket", "mode": "server", "address": "ws://127.0.0.1:4434/nnrp" }
  ],
  "capabilities": [
    "runtime.control.cancel",
    "runtime.control.progress",
    "runtime.object.reference"
  ]
}
```

Supported endpoint transports are `tcp`, `quic`, `ipc`, and `websocket`. Supported endpoint modes are `client` and `server`.

## Result Shape

The dry-run report preserves ready and skipped cases explicitly. Each case includes its scenario id, outcome, skip reason when a target lacks a required transport or capability, and evidence paths for frame logs and timing traces.
