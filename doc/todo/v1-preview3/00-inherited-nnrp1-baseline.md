# Rust Inherited NNRP/1 Baseline

This shard tracks protocol work that `nnrp-rs` must implement for `NNRP/1-preview3` even when the rule was inherited from preview1, preview2, or the current NNRP/1 public docs rather than newly frozen by preview3.

## Wire Identity And Common Envelope

- [x] Implement the inherited `NNRP/1.0` code-level identity: `version_major = 1`, `wire_format = 0`, and ALPN `nnrp/1`.
- [x] Implement the inherited 40B common header with `meta_len + body_len` packet framing.
- [x] Preserve inherited little-endian fixed-layout encoding, no ABI padding, strict reserved-field rejection, and strict unknown-bit rejection.
- [x] Preserve inherited preview1/preview2 `msg_type` assignments while adding preview3 control-plane extensions only in reserved slots.

## Preview2 Control Plane

- [x] Implement inherited `CLIENT_HELLO` fixed metadata and capability-window validation.
- [x] Implement inherited `SERVER_HELLO_ACK` fixed metadata and negotiated capability echo/denial semantics.
- [x] Implement inherited `SESSION_PATCH` and `SESSION_PATCH_ACK` metadata and validation.
- [x] Implement inherited 32B `FLOW_UPDATE` metadata, flags, scope zeroing rules, retry-after validity, and strict routing validation.
- [x] Implement inherited `RESULT_HINT` 16B metadata and stable budget/congestion/reason enums.
- [x] Implement inherited `TRANSPORT_PROBE` and `TRANSPORT_PROBE_ACK` 16B metadata.
- [x] Implement inherited `SESSION_MIGRATE` and `SESSION_MIGRATE_ACK` 24B metadata.
- [x] Implement inherited `PING`, `PONG`, connection-level `CLOSE`, and `ERROR` validation rules.

## Preview2 Data Plane

- [x] Implement inherited `FRAME_SUBMIT` v2 metadata fields, including `submit_mode`, `object_ref_mask`, budget policy, loss tolerance, payload bitmap, and payload frame count.
- [x] Implement inherited `RESULT_PUSH` v2 metadata fields, including result class, applied budget policy, reuse linkage, coverage counts, payload bitmap, and payload frame count.
- [x] Implement inherited `RESULT_DROP` stable reason semantics.
- [x] Implement inherited body-region prelude layout and fixed region ordering.
- [ ] Implement inherited object-reference block parsing, cache-backed region validation, and unresolved-reference rejection.
- [x] Implement inherited/preview3 typed payload descriptor fixed layout and strict descriptor flag validation.
- [ ] Implement inherited typed-payload frame region packing for non-tensor payload families.

## Preview2 Cache And Object Semantics

- [x] Implement inherited `CACHE_PUT`, `CACHE_ACK`, and `CACHE_INVALIDATE` metadata and lifecycle validation.
- [x] Implement inherited object kind assignments and reject unassigned object kinds on strict/conformance paths.
- [x] Implement inherited invalidate-scope assignments and reject unassigned scopes on strict/conformance paths.
- [ ] Preserve inherited cache miss behavior as explicit stable protocol errors rather than silent fallback.

## Conformance Baseline

- [ ] Consume preview2 mandatory L0 wire vectors as inherited `NNRP/1` baseline fixtures.
- [ ] Consume preview2 mandatory L1 control-plane cases as inherited `NNRP/1` baseline fixtures.
- [ ] Consume preview2 mandatory L1 data-plane cases as inherited `NNRP/1` baseline fixtures.
- [ ] Keep preview3 conformance additions layered on top of the inherited preview2 baseline instead of replacing it.
