use crate::{HeaderFlags, MessageType, NnrpError, CURRENT_WIRE_FORMAT};

pub const COMMON_HEADER_LEN: usize = 40;
pub const CURRENT_VERSION_MAJOR: u8 = 1;
pub const ALPN: &str = "nnrp/1";
const MAGIC: [u8; 4] = *b"NNRP";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonHeader {
    pub version_major: u8,
    pub wire_format: u8,
    pub message_type: MessageType,
    pub header_len: u8,
    pub flags: HeaderFlags,
    pub meta_len: u32,
    pub body_len: u32,
    pub session_id: u32,
    pub frame_id: u32,
    pub view_id: u16,
    pub route_id: u16,
    pub trace_id: u64,
}

impl CommonHeader {
    pub fn new(message_type: MessageType, meta_len: u32, body_len: u32) -> Self {
        Self {
            version_major: CURRENT_VERSION_MAJOR,
            wire_format: CURRENT_WIRE_FORMAT,
            message_type,
            header_len: COMMON_HEADER_LEN as u8,
            flags: HeaderFlags::NONE,
            meta_len,
            body_len,
            session_id: 0,
            frame_id: 0,
            view_id: 0,
            route_id: 0,
            trace_id: 0,
        }
    }

    pub fn packet_len(&self) -> Result<usize, NnrpError> {
        let payload_len = self
            .meta_len
            .checked_add(self.body_len)
            .ok_or(NnrpError::MessageLengthOverflow)? as usize;
        COMMON_HEADER_LEN
            .checked_add(payload_len)
            .ok_or(NnrpError::MessageLengthOverflow)
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        if destination.len() < COMMON_HEADER_LEN {
            return Err(NnrpError::DestinationTooShort {
                expected: COMMON_HEADER_LEN,
                actual: destination.len(),
            });
        }

        if self.header_len != COMMON_HEADER_LEN as u8 {
            return Err(NnrpError::InvalidHeaderLength(self.header_len));
        }

        self.flags.validate_known()?;

        destination[0..4].copy_from_slice(&MAGIC);
        destination[4] = self.version_major;
        destination[5] = self.wire_format;
        destination[6] = self.message_type as u8;
        destination[7] = self.header_len;
        destination[8..12].copy_from_slice(&self.flags.0.to_le_bytes());
        destination[12..16].copy_from_slice(&self.meta_len.to_le_bytes());
        destination[16..20].copy_from_slice(&self.body_len.to_le_bytes());
        destination[20..24].copy_from_slice(&self.session_id.to_le_bytes());
        destination[24..28].copy_from_slice(&self.frame_id.to_le_bytes());
        destination[28..30].copy_from_slice(&self.view_id.to_le_bytes());
        destination[30..32].copy_from_slice(&self.route_id.to_le_bytes());
        destination[32..40].copy_from_slice(&self.trace_id.to_le_bytes());
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; COMMON_HEADER_LEN], NnrpError> {
        let mut bytes = [0u8; COMMON_HEADER_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        if source.len() < COMMON_HEADER_LEN {
            return Err(NnrpError::SourceTooShort {
                expected: COMMON_HEADER_LEN,
                actual: source.len(),
            });
        }

        if source[0..4] != MAGIC {
            return Err(NnrpError::InvalidMagic);
        }

        let version_major = source[4];
        if version_major != CURRENT_VERSION_MAJOR {
            return Err(NnrpError::UnsupportedVersionMajor(version_major));
        }

        let wire_format = source[5];
        if wire_format != CURRENT_WIRE_FORMAT {
            return Err(NnrpError::UnsupportedWireFormat(wire_format));
        }

        let header_len = source[7];
        if header_len != COMMON_HEADER_LEN as u8 {
            return Err(NnrpError::InvalidHeaderLength(header_len));
        }

        let flags = HeaderFlags(u32::from_le_bytes(
            source[8..12].try_into().expect("slice length"),
        ));
        flags.validate_known()?;

        Ok(Self {
            version_major,
            wire_format,
            message_type: MessageType::try_from_u8(source[6])?,
            header_len,
            flags,
            meta_len: u32::from_le_bytes(source[12..16].try_into().expect("slice length")),
            body_len: u32::from_le_bytes(source[16..20].try_into().expect("slice length")),
            session_id: u32::from_le_bytes(source[20..24].try_into().expect("slice length")),
            frame_id: u32::from_le_bytes(source[24..28].try_into().expect("slice length")),
            view_id: u16::from_le_bytes(source[28..30].try_into().expect("slice length")),
            route_id: u16::from_le_bytes(source[30..32].try_into().expect("slice length")),
            trace_id: u64::from_le_bytes(source[32..40].try_into().expect("slice length")),
        })
    }

    pub fn parse_packet(source: &[u8]) -> Result<(Self, &[u8], &[u8]), NnrpError> {
        let header = Self::parse(source)?;
        let declared = header.packet_len()?;
        if declared != source.len() {
            return Err(NnrpError::PacketLengthMismatch {
                declared,
                actual: source.len(),
            });
        }

        let meta_start = COMMON_HEADER_LEN;
        let meta_end = meta_start + header.meta_len as usize;
        Ok((header, &source[meta_start..meta_end], &source[meta_end..]))
    }
}

#[cfg(test)]
mod tests {
    use super::{CommonHeader, COMMON_HEADER_LEN};
    use crate::{HeaderFlags, MessageType, NnrpError};

    #[test]
    fn common_header_round_trips_flow_update_vector_header() {
        let packet = hex_to_bytes("4e4e5250010017280000000020000000000000002a000000000000000000090088776655443322110104020000000200000000000000000000000000780000000700000003000000");

        let header = CommonHeader::parse(&packet).expect("header should parse");

        assert_eq!(header.version_major, 1);
        assert_eq!(header.message_type, MessageType::FlowUpdate);
        assert_eq!(header.header_len, COMMON_HEADER_LEN as u8);
        assert_eq!(header.meta_len, 32);
        assert_eq!(header.body_len, 0);
        assert_eq!(header.session_id, 42);
        assert_eq!(header.view_id, 0);
        assert_eq!(header.route_id, 9);
        assert_eq!(header.trace_id, 0x1122_3344_5566_7788);
        assert_eq!(
            header.to_bytes().unwrap().as_slice(),
            &packet[..COMMON_HEADER_LEN]
        );
    }

    #[test]
    fn common_header_rejects_length_mismatch() {
        let mut packet = CommonHeader::new(MessageType::Ping, 4, 0)
            .to_bytes()
            .expect("header writes")
            .to_vec();
        packet.extend_from_slice(&[1, 2]);

        assert_eq!(
            CommonHeader::parse_packet(&packet),
            Err(NnrpError::PacketLengthMismatch {
                declared: 44,
                actual: 42
            })
        );
    }

    #[test]
    fn common_header_rejects_reserved_flags() {
        let mut header = CommonHeader::new(MessageType::Ping, 0, 0);
        header.flags = HeaderFlags(0x40);

        assert_eq!(
            header.to_bytes(),
            Err(NnrpError::ReservedBitsSet {
                value: 0x40,
                allowed: HeaderFlags::KNOWN_MASK as u64
            })
        );
    }

    fn hex_to_bytes(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect()
    }
}
