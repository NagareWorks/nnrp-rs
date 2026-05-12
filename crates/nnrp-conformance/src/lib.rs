use nnrp_core::ProtocolVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoldenVersionVector {
    pub major: u8,
    pub stage: u8,
}

pub fn export_preview3_version_vector() -> GoldenVersionVector {
    GoldenVersionVector {
        major: ProtocolVersion::PREVIEW3.major,
        stage: 3,
    }
}

#[cfg(test)]
mod tests {
    use super::export_preview3_version_vector;

    #[test]
    fn preview3_version_vector_is_stable() {
        let vector = export_preview3_version_vector();
        assert_eq!(vector.major, 1);
        assert_eq!(vector.stage, 3);
    }
}