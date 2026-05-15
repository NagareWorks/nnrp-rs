# Rust Preview3 Cache, Schema, And Profile Registry

## Cache Contract

- [ ] Implement preview3 cache lease, object version, dependency tracking, and invalidation primitives.
- [ ] Implement stable error behavior for cache miss, lease expiry, version mismatch, and dependency invalidation.
- [ ] Keep model-private cache layouts out of the public protocol layer.

## Schema And Profile Registry

- [ ] Implement schema/profile registry primitives, install/update/invalidate/version-conflict handling.
- [ ] Implement schema descriptor common header at the frozen 32B layout.
- [ ] Implement typed payload descriptor common layout at the frozen 24B layout.
- [ ] Implement `descriptor_flags` semantics and validation.
- [ ] Implement binding between schema/profile identifiers and typed payload descriptors.

## Standard Profiles And Payload Families

- [ ] Keep the public preview3 layer profile-neutral.
- [ ] Implement `tensor` and `token` as peer first-round standard profiles.
- [ ] Implement minimum token-profile public semantics before downstream bindings expose token-native host APIs.
- [ ] Keep `structured_event` and `tool_delta` as payload families unless the protocol doc promotes them.