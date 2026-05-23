use nnrp_core::{CommonHeader, NnrpError, COMMON_HEADER_LEN};

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
