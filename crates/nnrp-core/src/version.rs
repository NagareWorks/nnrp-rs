use crate::NnrpError;

pub const CURRENT_WIRE_FORMAT: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u8,
    pub wire_format: u8,
}

impl ProtocolVersion {
    pub const CURRENT: Self = Self {
        major: 1,
        wire_format: CURRENT_WIRE_FORMAT,
    };

    pub fn try_new(major: u8, wire_format: u8) -> Result<Self, NnrpError> {
        if wire_format != CURRENT_WIRE_FORMAT {
            return Err(NnrpError::UnsupportedWireFormat(wire_format));
        }

        Ok(Self { major, wire_format })
    }
}

#[cfg(test)]
mod tests {
    use super::{ProtocolVersion, CURRENT_WIRE_FORMAT};
    use crate::NnrpError;

    #[test]
    fn protocol_version_accepts_current_wire_format() {
        assert_eq!(
            ProtocolVersion::try_new(1, CURRENT_WIRE_FORMAT),
            Ok(ProtocolVersion::CURRENT)
        );
    }

    #[test]
    fn protocol_version_rejects_unknown_wire_format() {
        assert_eq!(
            ProtocolVersion::try_new(1, 1),
            Err(NnrpError::UnsupportedWireFormat(1))
        );
    }

    #[test]
    fn current_protocol_version_uses_wire_format_zero() {
        assert_eq!(ProtocolVersion::CURRENT.major, 1);
        assert_eq!(ProtocolVersion::CURRENT.wire_format, CURRENT_WIRE_FORMAT);
    }
}
