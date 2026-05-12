use nnrp_core::{PreviewStage, ProtocolVersion};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpProtocolVersion {
    pub major: u8,
    pub stage: u8,
}

impl From<ProtocolVersion> for NnrpProtocolVersion {
    fn from(value: ProtocolVersion) -> Self {
        let stage = match value.stage {
            PreviewStage::Preview1 => 1,
            PreviewStage::Preview2 => 2,
            PreviewStage::Preview3 => 3,
        };
        Self {
            major: value.major,
            stage,
        }
    }
}

pub fn preview3_protocol_version() -> NnrpProtocolVersion {
    ProtocolVersion::PREVIEW3.into()
}

#[cfg(test)]
mod tests {
    use super::preview3_protocol_version;

    #[test]
    fn ffi_preview3_version_stays_aligned() {
        let version = preview3_protocol_version();
        assert_eq!(version.major, 1);
        assert_eq!(version.stage, 3);
    }
}
