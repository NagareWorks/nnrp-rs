# Rust Preview3 Core Surface

- [x] Keep `nnrp-core` responsible for wire primitives, protocol validation, state-machine core types, cache/schema semantics, and host-neutral logic.
- [x] Keep inherited preview1/preview2/current NNRP/1 wire and data-plane primitives visible in `nnrp-core`, not only preview3 extension types.
- [x] Implement preview3 typed payload descriptors, extension descriptors, and schema/profile binding rules.
- [x] Implement strict validation for illegal lifecycle, cache, and schema combinations.
