pub mod adapter_conformance;
mod nnrp1_baseline;
pub mod preview4_vectors;
pub mod wire_conformance;
pub mod wire_endpoint;
pub mod wire_external;
pub mod wire_reference;

use nnrp_core::ProtocolVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoldenVersionVector {
    pub major: u8,
    pub wire_format: u8,
}

pub fn current_version_vector() -> GoldenVersionVector {
    GoldenVersionVector {
        major: ProtocolVersion::CURRENT.major,
        wire_format: ProtocolVersion::CURRENT.wire_format,
    }
}

pub use preview4_vectors::{
    execute_preview4_case, execute_preview4_public_case, preview4_capability_tokens,
    preview4_case_ids, preview4_fixture_manifest, preview4_public_case_ids,
    PREVIEW4_PROTOCOL_VERSION,
};

#[cfg(test)]
mod tests {
    use super::current_version_vector;

    #[test]
    fn current_version_vector_is_stable() {
        let vector = current_version_vector();
        assert_eq!(vector.major, 1);
        assert_eq!(vector.wire_format, 0);
    }
}
