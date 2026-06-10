# 02 - Runtime Object And Cache Reference

## Runtime Object Model

- [x] Add object identity primitives.
  - [x] Object ID.
  - [x] Object kind.
  - [x] Producer role.
  - [x] Consumer role.
  - [x] Session ownership.
- [x] Add object lifecycle frames.
  - [x] `OBJECT_DECLARE`.
  - [x] `OBJECT_REF`.
  - [x] `OBJECT_RELEASE`.
  - [x] Release reason.
- [x] Add object delta frames.
  - [x] `OBJECT_PATCH`.
  - [x] `OBJECT_DELTA`.
  - [x] Region or segment descriptor.
  - [x] Delta sequence number.
- [x] Add object metadata.
  - [x] Byte size.
  - [x] Compute cost.
  - [x] Memory location hint.
  - [x] Lifetime hint.
  - [x] Ownership hint.

## Cache Reference Model

- [x] Add cache reference frames.
  - [x] `CACHE_REFERENCE`.
  - [x] `CACHE_MISS`.
  - [x] `CACHE_INVALIDATE` is inherited from the existing NNRP/1 message type, not re-assigned in preview4.
- [x] Add cache identity fields.
  - [x] Cache key.
  - [x] Schema/profile anchor.
  - [x] Optional lease ID.
  - [x] Optional producer trace ID.
- [x] Add cache policy fields.
  - [x] Reuse scope.
  - [x] Expiration hint.
  - [x] Invalidation reason is carried by inherited `CACHE_INVALIDATE` metadata.
  - [x] Miss reason.
- [x] Keep cache references optional and workload-declared.
  - [x] Do not assume dynamic rendering frames are cache-friendly.
  - [x] Do not add cache lookup to hot paths unless the profile declares it.

## Encoding And Copy Boundaries

- [x] Encode object references without copying object payloads.
- [x] Encode object deltas with bounded metadata.
- [x] Decode object references into borrowed or handle-backed views.
- [x] Decode object delta payloads into borrowed views.
- [x] Keep copied snapshot boundaries explicit for downstream SDKs.
- [x] Validate declared metadata and diagnostic segment lengths before exposing borrowed views.
- [x] Surface declared-length mismatches as typed decode errors.
- [x] Add tests for object release after result delivery.
- [x] Add tests for release after cancellation.
- [x] Add tests for cache miss and invalidation diagnostics.
  - [x] Cache miss diagnostic loopback coverage.
  - [x] Cache invalidation diagnostic loopback coverage.

## FFI Readiness

- [x] Define C ABI structs for object descriptors.
- [x] Define C ABI structs for object delta descriptors.
- [x] Define C ABI structs for cache reference descriptors.
- [x] Define release functions for native-owned object metadata buffers.
- [x] Keep FFI calls coarse for declare/request/progress/result/release loops.
