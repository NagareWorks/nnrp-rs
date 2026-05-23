use crate::NnrpError;

pub const CACHE_PUT_METADATA_LEN: usize = 32;
pub const CACHE_ACK_METADATA_LEN: usize = 28;
pub const CACHE_INVALIDATE_METADATA_LEN: usize = 20;
pub const CACHE_PUT_FLAGS_KNOWN_MASK: u32 = 0x0000_0003;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CacheObjectKind {
    CameraBlock = 0x0001,
    TileIndexBlock = 0x0002,
    TensorSectionTable = 0x0003,
    CodecTable = 0x0004,
    ReusableResultObject = 0x0005,
    PayloadLayoutTemplate = 0x0006,
    PromptSegment = 0x0007,
    ToolSchema = 0x0008,
    StructuredEventSchema = 0x0009,
}

impl CacheObjectKind {
    pub fn try_from_u32(value: u32) -> Result<Self, NnrpError> {
        match value {
            0x0001 => Ok(Self::CameraBlock),
            0x0002 => Ok(Self::TileIndexBlock),
            0x0003 => Ok(Self::TensorSectionTable),
            0x0004 => Ok(Self::CodecTable),
            0x0005 => Ok(Self::ReusableResultObject),
            0x0006 => Ok(Self::PayloadLayoutTemplate),
            0x0007 => Ok(Self::PromptSegment),
            0x0008 => Ok(Self::ToolSchema),
            0x0009 => Ok(Self::StructuredEventSchema),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "cache_object_kind",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CacheAckStatus {
    Accepted = 0,
    Rejected = 1,
    Replaced = 2,
}

impl CacheAckStatus {
    pub fn try_from_u32(value: u32) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Accepted),
            1 => Ok(Self::Rejected),
            2 => Ok(Self::Replaced),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "cache_ack_status",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CacheInvalidateScope {
    WholeSession = 0,
    Namespace = 1,
    ObjectKind = 2,
    ObjectKey = 3,
}

impl CacheInvalidateScope {
    pub fn try_from_u32(value: u32) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::WholeSession),
            1 => Ok(Self::Namespace),
            2 => Ok(Self::ObjectKind),
            3 => Ok(Self::ObjectKey),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "cache_invalidate_scope",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CachePutMetadata {
    pub cache_namespace: u32,
    pub cache_key_hi: u32,
    pub cache_key_lo: u32,
    pub object_kind: CacheObjectKind,
    pub ttl_ms: u32,
    pub object_bytes: u32,
    pub codec_bitmap: u32,
    pub flags: u32,
}

impl CachePutMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, CACHE_PUT_METADATA_LEN)?;
        let flags = read_u32(source, 28);
        validate_mask_u32(flags, CACHE_PUT_FLAGS_KNOWN_MASK)?;

        Ok(Self {
            cache_namespace: read_u32(source, 0),
            cache_key_hi: read_u32(source, 4),
            cache_key_lo: read_u32(source, 8),
            object_kind: CacheObjectKind::try_from_u32(read_u32(source, 12))?,
            ttl_ms: read_u32(source, 16),
            object_bytes: read_u32(source, 20),
            codec_bitmap: read_u32(source, 24),
            flags,
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, CACHE_PUT_METADATA_LEN)?;
        validate_mask_u32(self.flags, CACHE_PUT_FLAGS_KNOWN_MASK)?;

        write_u32(destination, 0, self.cache_namespace);
        write_u32(destination, 4, self.cache_key_hi);
        write_u32(destination, 8, self.cache_key_lo);
        write_u32(destination, 12, self.object_kind as u32);
        write_u32(destination, 16, self.ttl_ms);
        write_u32(destination, 20, self.object_bytes);
        write_u32(destination, 24, self.codec_bitmap);
        write_u32(destination, 28, self.flags);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; CACHE_PUT_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; CACHE_PUT_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheAckMetadata {
    pub cache_namespace: u32,
    pub cache_key_hi: u32,
    pub cache_key_lo: u32,
    pub status: CacheAckStatus,
    pub accepted_ttl_ms: u32,
    pub max_object_bytes: u32,
    pub detail_code: u32,
}

impl CacheAckMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, CACHE_ACK_METADATA_LEN)?;
        Ok(Self {
            cache_namespace: read_u32(source, 0),
            cache_key_hi: read_u32(source, 4),
            cache_key_lo: read_u32(source, 8),
            status: CacheAckStatus::try_from_u32(read_u32(source, 12))?,
            accepted_ttl_ms: read_u32(source, 16),
            max_object_bytes: read_u32(source, 20),
            detail_code: read_u32(source, 24),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, CACHE_ACK_METADATA_LEN)?;
        write_u32(destination, 0, self.cache_namespace);
        write_u32(destination, 4, self.cache_key_hi);
        write_u32(destination, 8, self.cache_key_lo);
        write_u32(destination, 12, self.status as u32);
        write_u32(destination, 16, self.accepted_ttl_ms);
        write_u32(destination, 20, self.max_object_bytes);
        write_u32(destination, 24, self.detail_code);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; CACHE_ACK_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; CACHE_ACK_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheInvalidateMetadata {
    pub invalidate_scope: CacheInvalidateScope,
    pub cache_namespace: u32,
    pub cache_key_hi: u32,
    pub cache_key_lo: u32,
    pub reason_code: u32,
}

impl CacheInvalidateMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, CACHE_INVALIDATE_METADATA_LEN)?;
        Ok(Self {
            invalidate_scope: CacheInvalidateScope::try_from_u32(read_u32(source, 0))?,
            cache_namespace: read_u32(source, 4),
            cache_key_hi: read_u32(source, 8),
            cache_key_lo: read_u32(source, 12),
            reason_code: read_u32(source, 16),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, CACHE_INVALIDATE_METADATA_LEN)?;
        write_u32(destination, 0, self.invalidate_scope as u32);
        write_u32(destination, 4, self.cache_namespace);
        write_u32(destination, 8, self.cache_key_hi);
        write_u32(destination, 12, self.cache_key_lo);
        write_u32(destination, 16, self.reason_code);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; CACHE_INVALIDATE_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; CACHE_INVALIDATE_METADATA_LEN];
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

fn validate_mask_u32(value: u32, allowed: u32) -> Result<(), NnrpError> {
    if value & !allowed != 0 {
        return Err(NnrpError::ReservedBitsSet {
            value: value as u64,
            allowed: allowed as u64,
        });
    }
    Ok(())
}

fn read_u32(source: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(source[offset..offset + 4].try_into().expect("slice length"))
}

fn write_u32(destination: &mut [u8], offset: usize, value: u32) {
    destination[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_metadata_round_trips_python_golden_vectors() {
        let put_bytes =
            hex_to_bytes("01000000040302010807060501000000983a0000000800000300000003000000");
        let put = CachePutMetadata::parse(&put_bytes).unwrap();
        assert_eq!(put.cache_namespace, 1);
        assert_eq!(put.cache_key_hi, 0x0102_0304);
        assert_eq!(put.cache_key_lo, 0x0506_0708);
        assert_eq!(put.object_kind, CacheObjectKind::CameraBlock);
        assert_eq!(put.ttl_ms, 15_000);
        assert_eq!(put.object_bytes, 2048);
        assert_eq!(put.flags, 3);
        assert_eq!(put.to_bytes().unwrap().as_slice(), put_bytes.as_slice());

        let ack_bytes = hex_to_bytes("01000000040302010807060500000000983a00000020000000000000");
        let ack = CacheAckMetadata::parse(&ack_bytes).unwrap();
        assert_eq!(ack.status, CacheAckStatus::Accepted);
        assert_eq!(ack.max_object_bytes, 8192);
        assert_eq!(ack.to_bytes().unwrap().as_slice(), ack_bytes.as_slice());

        let invalidate_bytes = hex_to_bytes("0000000001000000040302010807060502000000");
        let invalidate = CacheInvalidateMetadata::parse(&invalidate_bytes).unwrap();
        assert_eq!(
            invalidate.invalidate_scope,
            CacheInvalidateScope::WholeSession
        );
        assert_eq!(invalidate.cache_namespace, 1);
        assert_eq!(invalidate.cache_key_lo, 0x0506_0708);
        assert_eq!(
            invalidate.to_bytes().unwrap().as_slice(),
            invalidate_bytes.as_slice()
        );
    }

    #[test]
    fn cache_metadata_rejects_unknown_assignments_and_flags() {
        for value in 1..=9 {
            assert!(CacheObjectKind::try_from_u32(value).is_ok());
        }
        for value in 0..=2 {
            assert!(CacheAckStatus::try_from_u32(value).is_ok());
        }
        for value in 0..=3 {
            assert!(CacheInvalidateScope::try_from_u32(value).is_ok());
        }

        assert_eq!(
            CacheObjectKind::try_from_u32(0xffff),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "cache_object_kind",
                value: 0xffff
            })
        );
        assert_eq!(
            CacheInvalidateScope::try_from_u32(0xff),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "cache_invalidate_scope",
                value: 0xff
            })
        );

        let mut put_bytes = [0u8; CACHE_PUT_METADATA_LEN];
        write_u32(&mut put_bytes, 12, CacheObjectKind::CameraBlock as u32);
        write_u32(&mut put_bytes, 28, 0x4);
        assert_eq!(
            CachePutMetadata::parse(&put_bytes),
            Err(NnrpError::ReservedBitsSet {
                value: 0x4,
                allowed: CACHE_PUT_FLAGS_KNOWN_MASK as u64
            })
        );

        assert_eq!(
            CacheAckStatus::try_from_u32(99),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "cache_ack_status",
                value: 99
            })
        );
        assert_eq!(
            CachePutMetadata::parse(&[0u8; CACHE_PUT_METADATA_LEN - 1]),
            Err(NnrpError::SourceTooShort {
                expected: CACHE_PUT_METADATA_LEN,
                actual: CACHE_PUT_METADATA_LEN - 1
            })
        );
        let put = CachePutMetadata {
            cache_namespace: 1,
            cache_key_hi: 2,
            cache_key_lo: 3,
            object_kind: CacheObjectKind::CameraBlock,
            ttl_ms: 4,
            object_bytes: 5,
            codec_bitmap: 6,
            flags: 0,
        };
        assert_eq!(
            put.write(&mut [0u8; CACHE_PUT_METADATA_LEN - 1]),
            Err(NnrpError::DestinationTooShort {
                expected: CACHE_PUT_METADATA_LEN,
                actual: CACHE_PUT_METADATA_LEN - 1
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
