use crate::NnrpError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewStage {
    Preview1,
    Preview2,
    Preview3,
}

impl TryFrom<u8> for PreviewStage {
    type Error = NnrpError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Preview1),
            2 => Ok(Self::Preview2),
            3 => Ok(Self::Preview3),
            other => Err(NnrpError::UnsupportedPreviewStage(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u8,
    pub stage: PreviewStage,
}

impl ProtocolVersion {
    pub const PREVIEW3: Self = Self {
        major: 1,
        stage: PreviewStage::Preview3,
    };
}

#[cfg(test)]
mod tests {
    use super::{PreviewStage, ProtocolVersion};

    #[test]
    fn preview_stage_maps_frozen_values() {
        assert_eq!(PreviewStage::try_from(1), Ok(PreviewStage::Preview1));
        assert_eq!(PreviewStage::try_from(2), Ok(PreviewStage::Preview2));
        assert_eq!(PreviewStage::try_from(3), Ok(PreviewStage::Preview3));
    }

    #[test]
    fn preview3_constant_uses_stage_three() {
        assert_eq!(ProtocolVersion::PREVIEW3.major, 1);
        assert_eq!(ProtocolVersion::PREVIEW3.stage, PreviewStage::Preview3);
    }
}
