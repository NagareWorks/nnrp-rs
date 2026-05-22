use crate::NnrpError;

pub const SCHEMA_DESCRIPTOR_HEADER_LEN: usize = 32;
pub const TYPED_PAYLOAD_DESCRIPTOR_LEN: usize = 24;
pub const SCHEMA_FLAGS_KNOWN_MASK: u16 = 0x000f;
pub const DESCRIPTOR_FLAGS_KNOWN_MASK: u16 = 0x000f;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaDescriptorHeader {
    pub schema_id: u32,
    pub schema_version: u32,
    pub profile_id: u16,
    pub schema_flags: u16,
    pub min_version_major: u8,
    pub max_version_major: u8,
    pub body_bytes: u32,
    pub dependency_count: u16,
    pub default_stream_semantics: u16,
    pub schema_hash: u64,
}

impl SchemaDescriptorHeader {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, SCHEMA_DESCRIPTOR_HEADER_LEN)?;
        validate_zero_u16("schema_descriptor.reserved0", read_u16(source, 14))?;
        let schema_flags = read_u16(source, 10);
        validate_mask_u16(schema_flags, SCHEMA_FLAGS_KNOWN_MASK)?;

        Ok(Self {
            schema_id: read_u32(source, 0),
            schema_version: read_u32(source, 4),
            profile_id: read_u16(source, 8),
            schema_flags,
            min_version_major: source[12],
            max_version_major: source[13],
            body_bytes: read_u32(source, 16),
            dependency_count: read_u16(source, 20),
            default_stream_semantics: read_u16(source, 22),
            schema_hash: read_u64(source, 24),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, SCHEMA_DESCRIPTOR_HEADER_LEN)?;
        validate_mask_u16(self.schema_flags, SCHEMA_FLAGS_KNOWN_MASK)?;

        destination[..SCHEMA_DESCRIPTOR_HEADER_LEN].fill(0);
        write_u32(destination, 0, self.schema_id);
        write_u32(destination, 4, self.schema_version);
        write_u16(destination, 8, self.profile_id);
        write_u16(destination, 10, self.schema_flags);
        destination[12] = self.min_version_major;
        destination[13] = self.max_version_major;
        write_u32(destination, 16, self.body_bytes);
        write_u16(destination, 20, self.dependency_count);
        write_u16(destination, 22, self.default_stream_semantics);
        write_u64(destination, 24, self.schema_hash);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; SCHEMA_DESCRIPTOR_HEADER_LEN], NnrpError> {
        let mut bytes = [0u8; SCHEMA_DESCRIPTOR_HEADER_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypedPayloadDescriptor {
    pub profile_id: u16,
    pub descriptor_flags: u16,
    pub schema_id: u32,
    pub schema_version: u32,
    pub stream_semantics: u16,
    pub offset: u32,
    pub length: u32,
}

impl TypedPayloadDescriptor {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, TYPED_PAYLOAD_DESCRIPTOR_LEN)?;
        validate_zero_u16("typed_payload_descriptor.reserved0", read_u16(source, 14))?;
        let descriptor_flags = read_u16(source, 2);
        validate_mask_u16(descriptor_flags, DESCRIPTOR_FLAGS_KNOWN_MASK)?;

        Ok(Self {
            profile_id: read_u16(source, 0),
            descriptor_flags,
            schema_id: read_u32(source, 4),
            schema_version: read_u32(source, 8),
            stream_semantics: read_u16(source, 12),
            offset: read_u32(source, 16),
            length: read_u32(source, 20),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, TYPED_PAYLOAD_DESCRIPTOR_LEN)?;
        validate_mask_u16(self.descriptor_flags, DESCRIPTOR_FLAGS_KNOWN_MASK)?;

        destination[..TYPED_PAYLOAD_DESCRIPTOR_LEN].fill(0);
        write_u16(destination, 0, self.profile_id);
        write_u16(destination, 2, self.descriptor_flags);
        write_u32(destination, 4, self.schema_id);
        write_u32(destination, 8, self.schema_version);
        write_u16(destination, 12, self.stream_semantics);
        write_u32(destination, 16, self.offset);
        write_u32(destination, 20, self.length);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; TYPED_PAYLOAD_DESCRIPTOR_LEN], NnrpError> {
        let mut bytes = [0u8; TYPED_PAYLOAD_DESCRIPTOR_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

fn require_len(source: &[u8], expected: usize) -> Result<(), NnrpError> {
    if source.len() < expected {
        return Err(NnrpError::SourceTooShort {
            expected,
            actual: source.len(),
        });
    }

    Ok(())
}

fn require_destination_len(destination: &[u8], expected: usize) -> Result<(), NnrpError> {
    if destination.len() < expected {
        return Err(NnrpError::DestinationTooShort {
            expected,
            actual: destination.len(),
        });
    }

    Ok(())
}

fn validate_zero_u16(field: &'static str, value: u16) -> Result<(), NnrpError> {
    if value != 0 {
        return Err(NnrpError::NonZeroReservedField { field });
    }

    Ok(())
}

fn validate_mask_u16(value: u16, allowed: u16) -> Result<(), NnrpError> {
    if value & !allowed != 0 {
        return Err(NnrpError::ReservedBitsSet {
            value: value as u64,
            allowed: allowed as u64,
        });
    }

    Ok(())
}

fn read_u16(source: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(source[offset..offset + 2].try_into().expect("slice length"))
}

fn read_u32(source: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(source[offset..offset + 4].try_into().expect("slice length"))
}

fn read_u64(source: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(source[offset..offset + 8].try_into().expect("slice length"))
}

fn write_u16(destination: &mut [u8], offset: usize, value: u16) {
    destination[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(destination: &mut [u8], offset: usize, value: u32) {
    destination[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(destination: &mut [u8], offset: usize, value: u64) {
    destination[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::{SchemaDescriptorHeader, TypedPayloadDescriptor};
    use crate::{NnrpError, DESCRIPTOR_FLAGS_KNOWN_MASK, SCHEMA_FLAGS_KNOWN_MASK};

    #[test]
    fn schema_descriptor_header_round_trips_golden_vector() {
        let bytes =
            hex_to_bytes("011000000300000002000f000101000040000000020002008877665544332211");

        let descriptor = SchemaDescriptorHeader::parse(&bytes).unwrap();

        assert_eq!(descriptor.schema_id, 0x0000_1001);
        assert_eq!(descriptor.schema_version, 3);
        assert_eq!(descriptor.profile_id, 2);
        assert_eq!(descriptor.schema_flags, 0x000f);
        assert_eq!(descriptor.min_version_major, 1);
        assert_eq!(descriptor.max_version_major, 1);
        assert_eq!(descriptor.body_bytes, 64);
        assert_eq!(descriptor.dependency_count, 2);
        assert_eq!(descriptor.default_stream_semantics, 2);
        assert_eq!(descriptor.schema_hash, 0x1122_3344_5566_7788);
        assert_eq!(descriptor.to_bytes().unwrap().as_slice(), bytes.as_slice());
    }

    #[test]
    fn schema_descriptor_rejects_reserved_flags() {
        let mut bytes =
            hex_to_bytes("011000000300000002000f000101000040000000020002008877665544332211");
        bytes[10..12].copy_from_slice(&0x0010u16.to_le_bytes());

        assert_eq!(
            SchemaDescriptorHeader::parse(&bytes),
            Err(NnrpError::ReservedBitsSet {
                value: 0x10,
                allowed: SCHEMA_FLAGS_KNOWN_MASK as u64
            })
        );
    }

    #[test]
    fn typed_payload_descriptor_round_trips_golden_vector() {
        let bytes = hex_to_bytes("020002000110000003000000020000000000000018000000");

        let descriptor = TypedPayloadDescriptor::parse(&bytes).unwrap();

        assert_eq!(descriptor.profile_id, 2);
        assert_eq!(descriptor.descriptor_flags, 2);
        assert_eq!(descriptor.schema_id, 0x0000_1001);
        assert_eq!(descriptor.schema_version, 3);
        assert_eq!(descriptor.stream_semantics, 2);
        assert_eq!(descriptor.offset, 0);
        assert_eq!(descriptor.length, 24);
        assert_eq!(descriptor.to_bytes().unwrap().as_slice(), bytes.as_slice());
    }

    #[test]
    fn typed_payload_descriptor_rejects_reserved_flags() {
        let mut bytes = hex_to_bytes("020002000110000003000000020000000000000018000000");
        bytes[2..4].copy_from_slice(&0x0010u16.to_le_bytes());

        assert_eq!(
            TypedPayloadDescriptor::parse(&bytes),
            Err(NnrpError::ReservedBitsSet {
                value: 0x10,
                allowed: DESCRIPTOR_FLAGS_KNOWN_MASK as u64
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
