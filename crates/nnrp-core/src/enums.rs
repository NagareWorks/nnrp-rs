use crate::NnrpError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    ClientHello = 0x01,
    ServerHelloAck = 0x02,
    SessionPatch = 0x03,
    SessionPatchAck = 0x04,
    Close = 0x05,
    Error = 0x06,
    SessionOpen = 0x07,
    SessionOpenAck = 0x08,
    SessionClose = 0x09,
    SessionCloseAck = 0x0a,
    FrameSubmit = 0x10,
    FrameCancel = 0x11,
    ResultPush = 0x12,
    ResultDrop = 0x13,
    CachePut = 0x14,
    CacheAck = 0x15,
    CacheInvalidate = 0x16,
    FlowUpdate = 0x17,
    ResultHint = 0x18,
    TransportProbe = 0x19,
    TransportProbeAck = 0x1a,
    SessionMigrate = 0x1b,
    SessionMigrateAck = 0x1c,
    Ping = 0x20,
    Pong = 0x21,
}

impl MessageType {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        let message_type = match value {
            0x01 => Self::ClientHello,
            0x02 => Self::ServerHelloAck,
            0x03 => Self::SessionPatch,
            0x04 => Self::SessionPatchAck,
            0x05 => Self::Close,
            0x06 => Self::Error,
            0x07 => Self::SessionOpen,
            0x08 => Self::SessionOpenAck,
            0x09 => Self::SessionClose,
            0x0a => Self::SessionCloseAck,
            0x10 => Self::FrameSubmit,
            0x11 => Self::FrameCancel,
            0x12 => Self::ResultPush,
            0x13 => Self::ResultDrop,
            0x14 => Self::CachePut,
            0x15 => Self::CacheAck,
            0x16 => Self::CacheInvalidate,
            0x17 => Self::FlowUpdate,
            0x18 => Self::ResultHint,
            0x19 => Self::TransportProbe,
            0x1a => Self::TransportProbeAck,
            0x1b => Self::SessionMigrate,
            0x1c => Self::SessionMigrateAck,
            0x20 => Self::Ping,
            0x21 => Self::Pong,
            _ => return Err(NnrpError::UnknownMessageType(value)),
        };

        Ok(message_type)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderFlags(pub u32);

impl HeaderFlags {
    pub const NONE: Self = Self(0);
    pub const ACK_REQUIRED: Self = Self(0x0000_0001);
    pub const CAN_DROP: Self = Self(0x0000_0002);
    pub const STALE: Self = Self(0x0000_0004);
    pub const EOS: Self = Self(0x0000_0008);
    pub const RETRANSMIT: Self = Self(0x0000_0010);
    pub const KEYFRAME: Self = Self(0x0000_0020);
    pub const KNOWN_MASK: u32 = Self::ACK_REQUIRED.0
        | Self::CAN_DROP.0
        | Self::STALE.0
        | Self::EOS.0
        | Self::RETRANSMIT.0
        | Self::KEYFRAME.0;

    pub fn validate_known(self) -> Result<(), NnrpError> {
        if self.0 & !Self::KNOWN_MASK != 0 {
            return Err(NnrpError::ReservedBitsSet {
                value: self.0 as u64,
                allowed: Self::KNOWN_MASK as u64,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::MessageType;

    #[test]
    fn preview3_session_message_type_assignments_are_frozen() {
        assert_eq!(MessageType::try_from_u8(0x07), Ok(MessageType::SessionOpen));
        assert_eq!(
            MessageType::try_from_u8(0x08),
            Ok(MessageType::SessionOpenAck)
        );
        assert_eq!(
            MessageType::try_from_u8(0x09),
            Ok(MessageType::SessionClose)
        );
        assert_eq!(
            MessageType::try_from_u8(0x0a),
            Ok(MessageType::SessionCloseAck)
        );
    }
}
