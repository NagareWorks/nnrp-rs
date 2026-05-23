# Rust Preview3 Cache, Schema, And Profile Registry

## Cache Contract

- [x] Implement preview3 cache lease, object version, dependency tracking, and invalidation primitives.
- [x] Implement stable error behavior for cache miss, lease expiry, version mismatch, and dependency invalidation.
- [x] Keep model-private cache layouts out of the public protocol layer.

## Schema And Profile Registry

- [x] Implement schema/profile registry primitives, install/update/invalidate/version-conflict handling.
- [x] Implement schema descriptor common header at the frozen 32B layout.
- [x] Implement typed payload descriptor common layout at the inherited/preview3 24B layout.
- [x] Implement `descriptor_flags` semantics and validation.
- [x] Implement binding between frozen schema/profile identifiers and typed payload descriptors, including `profile_id = 0` as `unspecified` rather than an implicit tensor default.
- [x] Land the first-round standard registry assignments consumed by conformance, including the peer `tensor` / `token` profile IDs and the canonical token delta schema anchor.

## Standard Profiles And Payload Families

- [x] Keep the public preview3 layer profile-neutral.
- [x] Implement `tensor` and `token` as peer first-round standard profiles.
- [x] Implement minimum token-profile public semantics before downstream bindings expose token-native host APIs.
- [ ] Keep `structured_event` and `tool_delta` as payload families unless the protocol doc promotes them.
