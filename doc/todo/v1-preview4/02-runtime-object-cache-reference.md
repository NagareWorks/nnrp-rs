# 02 - Runtime Object And Cache Reference

## Runtime Object Model

- [ ] Add object identity primitives.
  - [ ] Object ID.
  - [ ] Object kind.
  - [ ] Producer role.
  - [ ] Consumer role.
  - [ ] Session ownership.
- [ ] Add object lifecycle frames.
  - [ ] `OBJECT_DECLARE`.
  - [ ] `OBJECT_REF`.
  - [ ] `OBJECT_RELEASE`.
  - [ ] Release reason.
- [ ] Add object delta frames.
  - [ ] `OBJECT_PATCH`.
  - [ ] `OBJECT_DELTA`.
  - [ ] Region or segment descriptor.
  - [ ] Delta sequence number.
- [ ] Add object metadata.
  - [ ] Byte size.
  - [ ] Compute cost.
  - [ ] Memory location hint.
  - [ ] Lifetime hint.
  - [ ] Ownership hint.

## Cache Reference Model

- [ ] Add cache reference frames.
  - [ ] `CACHE_REFERENCE`.
  - [ ] `CACHE_MISS`.
  - [ ] `CACHE_INVALIDATE`.
- [ ] Add cache identity fields.
  - [ ] Cache key.
  - [ ] Schema/profile anchor.
  - [ ] Optional lease ID.
  - [ ] Optional producer trace ID.
- [ ] Add cache policy fields.
  - [ ] Reuse scope.
  - [ ] Expiration hint.
  - [ ] Invalidation reason.
  - [ ] Miss reason.
- [ ] Keep cache references optional and workload-declared.
  - [ ] Do not assume dynamic rendering frames are cache-friendly.
  - [ ] Do not add cache lookup to hot paths unless the profile declares it.

## Encoding And Copy Boundaries

- [ ] Encode object references without copying object payloads.
- [ ] Encode object deltas with bounded metadata.
- [ ] Decode object references into borrowed or handle-backed views.
- [ ] Keep copied fallback paths explicit for downstream SDKs.
- [ ] Add tests for object release after result delivery.
- [ ] Add tests for release after cancellation.
- [ ] Add tests for cache miss and invalidation diagnostics.

## FFI Readiness

- [ ] Define C ABI structs for object descriptors.
- [ ] Define C ABI structs for object delta descriptors.
- [ ] Define C ABI structs for cache reference descriptors.
- [ ] Define release functions for native-owned object metadata buffers.
- [ ] Keep FFI calls coarse for declare/request/progress/result/release loops.
