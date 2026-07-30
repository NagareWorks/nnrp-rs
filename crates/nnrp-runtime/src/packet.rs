use nnrp_core::{CommonHeader, HeaderFlags, MessageType, NnrpError, COMMON_HEADER_LEN};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeFrameHeader {
    pub version_major: u8,
    pub wire_format: u8,
    pub message_type: MessageType,
    pub flags: HeaderFlags,
    pub session_id: u32,
    pub frame_id: u32,
    pub view_id: u16,
    pub route_id: u16,
    pub trace_id: u64,
}

impl From<&CommonHeader> for RuntimeFrameHeader {
    fn from(header: &CommonHeader) -> Self {
        Self {
            version_major: header.version_major,
            wire_format: header.wire_format,
            message_type: header.message_type,
            flags: header.flags,
            session_id: header.session_id,
            frame_id: header.frame_id,
            view_id: header.view_id,
            route_id: header.route_id,
            trace_id: header.trace_id,
        }
    }
}

impl From<CommonHeader> for RuntimeFrameHeader {
    fn from(header: CommonHeader) -> Self {
        Self::from(&header)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePacket {
    pub header: CommonHeader,
    pub metadata: Vec<u8>,
    pub body: Vec<u8>,
}

impl RuntimePacket {
    pub fn new(
        mut header: CommonHeader,
        metadata: Vec<u8>,
        body: Vec<u8>,
    ) -> Result<Self, NnrpError> {
        header.meta_len = checked_len(metadata.len())?;
        header.body_len = checked_len(body.len())?;
        header.packet_len()?;
        Ok(Self {
            header,
            metadata,
            body,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, NnrpError> {
        let packet_len = self.header.packet_len()?;
        let mut bytes = Vec::with_capacity(packet_len);
        bytes.extend_from_slice(&self.header.to_bytes()?);
        bytes.extend_from_slice(&self.metadata);
        bytes.extend_from_slice(&self.body);
        Ok(bytes)
    }

    pub fn from_parts(
        header: CommonHeader,
        metadata: Vec<u8>,
        body: Vec<u8>,
    ) -> Result<Self, NnrpError> {
        let declared_len = header.packet_len()?;
        let actual_len = COMMON_HEADER_LEN + metadata.len() + body.len();
        if declared_len != actual_len {
            return Err(NnrpError::PacketLengthMismatch {
                declared: declared_len,
                actual: actual_len,
            });
        }

        Ok(Self {
            header,
            metadata,
            body,
        })
    }
}

fn checked_len(value: usize) -> Result<u32, NnrpError> {
    value
        .try_into()
        .map_err(|_| NnrpError::MessageLengthOverflow)
}

#[cfg(test)]
mod tests {
    use super::RuntimeFrameHeader;
    use nnrp_core::{CommonHeader, HeaderFlags, MessageType};

    #[test]
    fn runtime_frame_header_preserves_every_non_derived_common_header_field() {
        let mut common = CommonHeader::new(MessageType::Progress, 12, 34);
        common.flags = HeaderFlags(3);
        common.session_id = 5;
        common.frame_id = 7;
        common.view_id = 11;
        common.route_id = 13;
        common.trace_id = 17;

        let header = RuntimeFrameHeader::from(common);

        assert_eq!(header.version_major, 1);
        assert_eq!(header.wire_format, 0);
        assert_eq!(header.message_type, MessageType::Progress);
        assert_eq!(header.flags, HeaderFlags(3));
        assert_eq!(header.session_id, 5);
        assert_eq!(header.frame_id, 7);
        assert_eq!(header.view_id, 11);
        assert_eq!(header.route_id, 13);
        assert_eq!(header.trace_id, 17);
    }
}
