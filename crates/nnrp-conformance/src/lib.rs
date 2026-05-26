pub mod adapter_conformance;
mod preview2_baseline;
pub mod preview3_vectors;

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

pub use preview3_vectors::{
    execute_preview3_case, preview3_case_ids, preview3_fixture_manifest, preview3_golden_vectors,
    public_preview3_case_ids, PREVIEW3_PROTOCOL_VERSION,
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
