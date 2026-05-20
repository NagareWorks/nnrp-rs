pub mod adapter_conformance;

use nnrp_core::ProtocolVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoldenVersionVector {
    pub major: u8,
    pub wire_format: u8,
}

pub fn export_current_version_vector() -> GoldenVersionVector {
    GoldenVersionVector {
        major: ProtocolVersion::CURRENT.major,
        wire_format: ProtocolVersion::CURRENT.wire_format,
    }
}

#[cfg(test)]
mod tests {
    use super::export_current_version_vector;

    #[test]
    fn current_version_vector_is_stable() {
        let vector = export_current_version_vector();
        assert_eq!(vector.major, 1);
        assert_eq!(vector.wire_format, 0);
    }
}