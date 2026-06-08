use crate::NnrpError;

pub const OBJECT_DESCRIPTOR_METADATA_LEN: usize = 48;
pub const OBJECT_REFERENCE_METADATA_LEN: usize = 48;
pub const OBJECT_RELEASE_METADATA_LEN: usize = 32;
pub const OBJECT_DELTA_METADATA_LEN: usize = 40;
pub const CACHE_REFERENCE_METADATA_LEN: usize = 48;
pub const CACHE_MISS_METADATA_LEN: usize = 32;

pub const OBJECT_REFERENCE_FLAGS_KNOWN_MASK: u32 = 0x0000_0007;
pub const OBJECT_RELEASE_FLAGS_KNOWN_MASK: u8 = 0x03;
pub const OBJECT_DELTA_FLAGS_KNOWN_MASK: u32 = 0x0000_0007;
pub const CACHE_REFERENCE_FLAGS_KNOWN_MASK: u32 = 0x0000_0003;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum RuntimeObjectKind {
    Unspecified = 0x0000,
    Tensor = 0x0001,
    TokenBlock = 0x0002,
    ImageTile = 0x0003,
    FeatureMap = 0x0004,
    ToolResult = 0x0005,
    TraceSegment = 0x0006,
    OpaqueBytes = 0x0007,
    DocumentChunk = 0x0008,
    AudioChunk = 0x0009,
    VideoChunk = 0x000a,
    RoutePlan = 0x000b,
    CacheManifest = 0x000c,
}

impl RuntimeObjectKind {
    pub fn try_from_u16(value: u16) -> Result<Self, NnrpError> {
        match value {
            0x0000 => Ok(Self::Unspecified),
            0x0001 => Ok(Self::Tensor),
            0x0002 => Ok(Self::TokenBlock),
            0x0003 => Ok(Self::ImageTile),
            0x0004 => Ok(Self::FeatureMap),
            0x0005 => Ok(Self::ToolResult),
            0x0006 => Ok(Self::TraceSegment),
            0x0007 => Ok(Self::OpaqueBytes),
            0x0008 => Ok(Self::DocumentChunk),
            0x0009 => Ok(Self::AudioChunk),
            0x000a => Ok(Self::VideoChunk),
            0x000b => Ok(Self::RoutePlan),
            0x000c => Ok(Self::CacheManifest),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "runtime_object_kind",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RuntimeRole {
    Unspecified = 0x00,
    Client = 0x01,
    Server = 0x02,
    Runtime = 0x03,
    Subagent = 0x04,
    Tool = 0x05,
    Scheduler = 0x06,
    ConformanceRunner = 0x07,
}

impl RuntimeRole {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0x00 => Ok(Self::Unspecified),
            0x01 => Ok(Self::Client),
            0x02 => Ok(Self::Server),
            0x03 => Ok(Self::Runtime),
            0x04 => Ok(Self::Subagent),
            0x05 => Ok(Self::Tool),
            0x06 => Ok(Self::Scheduler),
            0x07 => Ok(Self::ConformanceRunner),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "runtime_role",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MemoryLocationHint {
    Unspecified = 0x0000,
    HostMemory = 0x0001,
    DeviceMemory = 0x0002,
    SharedMemory = 0x0003,
    RemoteMemory = 0x0004,
    MmapFile = 0x0005,
    ObjectStore = 0x0006,
}

impl MemoryLocationHint {
    pub fn try_from_u16(value: u16) -> Result<Self, NnrpError> {
        match value {
            0x0000 => Ok(Self::Unspecified),
            0x0001 => Ok(Self::HostMemory),
            0x0002 => Ok(Self::DeviceMemory),
            0x0003 => Ok(Self::SharedMemory),
            0x0004 => Ok(Self::RemoteMemory),
            0x0005 => Ok(Self::MmapFile),
            0x0006 => Ok(Self::ObjectStore),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "memory_location_hint",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum OwnershipHint {
    Unspecified = 0x0000,
    ProducerOwned = 0x0001,
    ConsumerOwned = 0x0002,
    SessionOwned = 0x0003,
    Borrowed = 0x0004,
    TransferOnRef = 0x0005,
    ReleaseOnDrop = 0x0006,
}

impl OwnershipHint {
    pub fn try_from_u16(value: u16) -> Result<Self, NnrpError> {
        match value {
            0x0000 => Ok(Self::Unspecified),
            0x0001 => Ok(Self::ProducerOwned),
            0x0002 => Ok(Self::ConsumerOwned),
            0x0003 => Ok(Self::SessionOwned),
            0x0004 => Ok(Self::Borrowed),
            0x0005 => Ok(Self::TransferOnRef),
            0x0006 => Ok(Self::ReleaseOnDrop),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "ownership_hint",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ObjectReleaseReason {
    Completed = 0x0000,
    Cancelled = 0x0001,
    Expired = 0x0002,
    Replaced = 0x0003,
    Invalidated = 0x0004,
    OwnerClosed = 0x0005,
    LeaseExpired = 0x0006,
    ConformanceInjection = 0x0007,
}

impl ObjectReleaseReason {
    pub fn try_from_u16(value: u16) -> Result<Self, NnrpError> {
        match value {
            0x0000 => Ok(Self::Completed),
            0x0001 => Ok(Self::Cancelled),
            0x0002 => Ok(Self::Expired),
            0x0003 => Ok(Self::Replaced),
            0x0004 => Ok(Self::Invalidated),
            0x0005 => Ok(Self::OwnerClosed),
            0x0006 => Ok(Self::LeaseExpired),
            0x0007 => Ok(Self::ConformanceInjection),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "object_release_reason",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum CacheReuseScope {
    Operation = 0x0000,
    Session = 0x0001,
    Connection = 0x0002,
    Global = 0x0003,
    Tenant = 0x0004,
    Profile = 0x0005,
}

impl CacheReuseScope {
    pub fn try_from_u16(value: u16) -> Result<Self, NnrpError> {
        match value {
            0x0000 => Ok(Self::Operation),
            0x0001 => Ok(Self::Session),
            0x0002 => Ok(Self::Connection),
            0x0003 => Ok(Self::Global),
            0x0004 => Ok(Self::Tenant),
            0x0005 => Ok(Self::Profile),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "cache_reuse_scope",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum CacheMissReason {
    Unknown = 0x0000,
    NotFound = 0x0001,
    Expired = 0x0002,
    Invalidated = 0x0003,
    SchemaMismatch = 0x0004,
    ProducerUnavailable = 0x0005,
    LeaseRequired = 0x0006,
    PermissionDenied = 0x0007,
}

impl CacheMissReason {
    pub fn try_from_u16(value: u16) -> Result<Self, NnrpError> {
        match value {
            0x0000 => Ok(Self::Unknown),
            0x0001 => Ok(Self::NotFound),
            0x0002 => Ok(Self::Expired),
            0x0003 => Ok(Self::Invalidated),
            0x0004 => Ok(Self::SchemaMismatch),
            0x0005 => Ok(Self::ProducerUnavailable),
            0x0006 => Ok(Self::LeaseRequired),
            0x0007 => Ok(Self::PermissionDenied),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "cache_miss_reason",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectDescriptorMetadata {
    pub object_id: u64,
    pub object_kind: RuntimeObjectKind,
    pub producer_role: RuntimeRole,
    pub consumer_role: RuntimeRole,
    pub session_id: u32,
    pub byte_size: u64,
    pub compute_cost_units: u32,
    pub memory_location_hint: MemoryLocationHint,
    pub ownership_hint: OwnershipHint,
    pub lifetime_hint_ms: u32,
    pub metadata_bytes: u32,
}

impl ObjectDescriptorMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, OBJECT_DESCRIPTOR_METADATA_LEN)?;
        validate_zero_u64("object_descriptor.reserved", read_u64(source, 40))?;
        Ok(Self {
            object_id: read_u64(source, 0),
            object_kind: RuntimeObjectKind::try_from_u16(read_u16(source, 8))?,
            producer_role: RuntimeRole::try_from_u8(source[10])?,
            consumer_role: RuntimeRole::try_from_u8(source[11])?,
            session_id: read_u32(source, 12),
            byte_size: read_u64(source, 16),
            compute_cost_units: read_u32(source, 24),
            memory_location_hint: MemoryLocationHint::try_from_u16(read_u16(source, 28))?,
            ownership_hint: OwnershipHint::try_from_u16(read_u16(source, 30))?,
            lifetime_hint_ms: read_u32(source, 32),
            metadata_bytes: read_u32(source, 36),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, OBJECT_DESCRIPTOR_METADATA_LEN)?;
        destination[..OBJECT_DESCRIPTOR_METADATA_LEN].fill(0);
        write_u64(destination, 0, self.object_id);
        write_u16(destination, 8, self.object_kind as u16);
        destination[10] = self.producer_role as u8;
        destination[11] = self.consumer_role as u8;
        write_u32(destination, 12, self.session_id);
        write_u64(destination, 16, self.byte_size);
        write_u32(destination, 24, self.compute_cost_units);
        write_u16(destination, 28, self.memory_location_hint as u16);
        write_u16(destination, 30, self.ownership_hint as u16);
        write_u32(destination, 32, self.lifetime_hint_ms);
        write_u32(destination, 36, self.metadata_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; OBJECT_DESCRIPTOR_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; OBJECT_DESCRIPTOR_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectReferenceMetadata {
    pub object_id: u64,
    pub operation_id: u64,
    pub object_version: u64,
    pub offset: u64,
    pub length: u64,
    pub flags: u32,
    pub metadata_bytes: u32,
}

impl ObjectReferenceMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, OBJECT_REFERENCE_METADATA_LEN)?;
        let flags = read_u32(source, 40);
        validate_mask_u32(flags, OBJECT_REFERENCE_FLAGS_KNOWN_MASK)?;
        Ok(Self {
            object_id: read_u64(source, 0),
            operation_id: read_u64(source, 8),
            object_version: read_u64(source, 16),
            offset: read_u64(source, 24),
            length: read_u64(source, 32),
            flags,
            metadata_bytes: read_u32(source, 44),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, OBJECT_REFERENCE_METADATA_LEN)?;
        validate_mask_u32(self.flags, OBJECT_REFERENCE_FLAGS_KNOWN_MASK)?;
        write_u64(destination, 0, self.object_id);
        write_u64(destination, 8, self.operation_id);
        write_u64(destination, 16, self.object_version);
        write_u64(destination, 24, self.offset);
        write_u64(destination, 32, self.length);
        write_u32(destination, 40, self.flags);
        write_u32(destination, 44, self.metadata_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; OBJECT_REFERENCE_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; OBJECT_REFERENCE_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectReleaseMetadata {
    pub object_id: u64,
    pub operation_id: u64,
    pub release_reason: ObjectReleaseReason,
    pub source_role: RuntimeRole,
    pub flags: u8,
    pub diagnostic_bytes: u32,
}

impl ObjectReleaseMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, OBJECT_RELEASE_METADATA_LEN)?;
        let flags = source[19];
        validate_mask_u8(flags, OBJECT_RELEASE_FLAGS_KNOWN_MASK)?;
        validate_zero_u64("object_release.reserved", read_u64(source, 24))?;
        Ok(Self {
            object_id: read_u64(source, 0),
            operation_id: read_u64(source, 8),
            release_reason: ObjectReleaseReason::try_from_u16(read_u16(source, 16))?,
            source_role: RuntimeRole::try_from_u8(source[18])?,
            flags,
            diagnostic_bytes: read_u32(source, 20),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, OBJECT_RELEASE_METADATA_LEN)?;
        validate_mask_u8(self.flags, OBJECT_RELEASE_FLAGS_KNOWN_MASK)?;
        destination[..OBJECT_RELEASE_METADATA_LEN].fill(0);
        write_u64(destination, 0, self.object_id);
        write_u64(destination, 8, self.operation_id);
        write_u16(destination, 16, self.release_reason as u16);
        destination[18] = self.source_role as u8;
        destination[19] = self.flags;
        write_u32(destination, 20, self.diagnostic_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; OBJECT_RELEASE_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; OBJECT_RELEASE_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectDeltaMetadata {
    pub object_id: u64,
    pub delta_sequence: u64,
    pub region_offset: u64,
    pub region_bytes: u32,
    pub delta_bytes: u32,
    pub flags: u32,
    pub metadata_bytes: u32,
}

impl ObjectDeltaMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, OBJECT_DELTA_METADATA_LEN)?;
        let flags = read_u32(source, 32);
        validate_mask_u32(flags, OBJECT_DELTA_FLAGS_KNOWN_MASK)?;
        Ok(Self {
            object_id: read_u64(source, 0),
            delta_sequence: read_u64(source, 8),
            region_offset: read_u64(source, 16),
            region_bytes: read_u32(source, 24),
            delta_bytes: read_u32(source, 28),
            flags,
            metadata_bytes: read_u32(source, 36),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, OBJECT_DELTA_METADATA_LEN)?;
        validate_mask_u32(self.flags, OBJECT_DELTA_FLAGS_KNOWN_MASK)?;
        write_u64(destination, 0, self.object_id);
        write_u64(destination, 8, self.delta_sequence);
        write_u64(destination, 16, self.region_offset);
        write_u32(destination, 24, self.region_bytes);
        write_u32(destination, 28, self.delta_bytes);
        write_u32(destination, 32, self.flags);
        write_u32(destination, 36, self.metadata_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; OBJECT_DELTA_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; OBJECT_DELTA_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheReferenceMetadata {
    pub cache_key_hi: u64,
    pub cache_key_lo: u64,
    pub profile_id: u16,
    pub reuse_scope: CacheReuseScope,
    pub lease_id: u64,
    pub producer_trace_id: u64,
    pub expiration_hint_ms: u32,
    pub metadata_bytes: u32,
    pub flags: u32,
}

impl CacheReferenceMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, CACHE_REFERENCE_METADATA_LEN)?;
        let flags = read_u32(source, 44);
        validate_mask_u32(flags, CACHE_REFERENCE_FLAGS_KNOWN_MASK)?;
        Ok(Self {
            cache_key_hi: read_u64(source, 0),
            cache_key_lo: read_u64(source, 8),
            profile_id: read_u16(source, 16),
            reuse_scope: CacheReuseScope::try_from_u16(read_u16(source, 18))?,
            lease_id: read_u64(source, 20),
            producer_trace_id: read_u64(source, 28),
            expiration_hint_ms: read_u32(source, 36),
            metadata_bytes: read_u32(source, 40),
            flags,
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, CACHE_REFERENCE_METADATA_LEN)?;
        validate_mask_u32(self.flags, CACHE_REFERENCE_FLAGS_KNOWN_MASK)?;
        write_u64(destination, 0, self.cache_key_hi);
        write_u64(destination, 8, self.cache_key_lo);
        write_u16(destination, 16, self.profile_id);
        write_u16(destination, 18, self.reuse_scope as u16);
        write_u64(destination, 20, self.lease_id);
        write_u64(destination, 28, self.producer_trace_id);
        write_u32(destination, 36, self.expiration_hint_ms);
        write_u32(destination, 40, self.metadata_bytes);
        write_u32(destination, 44, self.flags);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; CACHE_REFERENCE_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; CACHE_REFERENCE_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheMissMetadata {
    pub cache_key_hi: u64,
    pub cache_key_lo: u64,
    pub miss_reason: CacheMissReason,
    pub profile_id: u16,
    pub diagnostic_bytes: u32,
}

impl CacheMissMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, CACHE_MISS_METADATA_LEN)?;
        validate_zero_u64("cache_miss.reserved", read_u64(source, 24))?;
        Ok(Self {
            cache_key_hi: read_u64(source, 0),
            cache_key_lo: read_u64(source, 8),
            miss_reason: CacheMissReason::try_from_u16(read_u16(source, 16))?,
            profile_id: read_u16(source, 18),
            diagnostic_bytes: read_u32(source, 20),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, CACHE_MISS_METADATA_LEN)?;
        destination[..CACHE_MISS_METADATA_LEN].fill(0);
        write_u64(destination, 0, self.cache_key_hi);
        write_u64(destination, 8, self.cache_key_lo);
        write_u16(destination, 16, self.miss_reason as u16);
        write_u16(destination, 18, self.profile_id);
        write_u32(destination, 20, self.diagnostic_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; CACHE_MISS_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; CACHE_MISS_METADATA_LEN];
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

fn validate_mask_u8(value: u8, allowed: u8) -> Result<(), NnrpError> {
    if value & !allowed != 0 {
        return Err(NnrpError::ReservedBitsSet {
            value: value as u64,
            allowed: allowed as u64,
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

fn validate_zero_u64(field: &'static str, value: u64) -> Result<(), NnrpError> {
    if value != 0 {
        return Err(NnrpError::NonZeroReservedField { field });
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
    use super::*;

    #[test]
    fn object_and_cache_metadata_round_trip_fixed_layouts() {
        let descriptor = ObjectDescriptorMetadata {
            object_id: 1,
            object_kind: RuntimeObjectKind::Tensor,
            producer_role: RuntimeRole::Runtime,
            consumer_role: RuntimeRole::Client,
            session_id: 2,
            byte_size: 4096,
            compute_cost_units: 7,
            memory_location_hint: MemoryLocationHint::DeviceMemory,
            ownership_hint: OwnershipHint::Borrowed,
            lifetime_hint_ms: 500,
            metadata_bytes: 12,
        };
        assert_eq!(
            ObjectDescriptorMetadata::parse(&descriptor.to_bytes().unwrap()).unwrap(),
            descriptor
        );

        let reference = ObjectReferenceMetadata {
            object_id: 1,
            operation_id: 3,
            object_version: 4,
            offset: 8,
            length: 16,
            flags: OBJECT_REFERENCE_FLAGS_KNOWN_MASK,
            metadata_bytes: 20,
        };
        assert_eq!(
            ObjectReferenceMetadata::parse(&reference.to_bytes().unwrap()).unwrap(),
            reference
        );

        let release = ObjectReleaseMetadata {
            object_id: 1,
            operation_id: 3,
            release_reason: ObjectReleaseReason::Cancelled,
            source_role: RuntimeRole::Scheduler,
            flags: OBJECT_RELEASE_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 8,
        };
        assert_eq!(
            ObjectReleaseMetadata::parse(&release.to_bytes().unwrap()).unwrap(),
            release
        );

        let delta = ObjectDeltaMetadata {
            object_id: 1,
            delta_sequence: 2,
            region_offset: 64,
            region_bytes: 32,
            delta_bytes: 16,
            flags: OBJECT_DELTA_FLAGS_KNOWN_MASK,
            metadata_bytes: 4,
        };
        assert_eq!(
            ObjectDeltaMetadata::parse(&delta.to_bytes().unwrap()).unwrap(),
            delta
        );

        let cache_ref = CacheReferenceMetadata {
            cache_key_hi: 0x1122,
            cache_key_lo: 0x3344,
            profile_id: 3,
            reuse_scope: CacheReuseScope::Session,
            lease_id: 5,
            producer_trace_id: 6,
            expiration_hint_ms: 700,
            metadata_bytes: 24,
            flags: CACHE_REFERENCE_FLAGS_KNOWN_MASK,
        };
        assert_eq!(
            CacheReferenceMetadata::parse(&cache_ref.to_bytes().unwrap()).unwrap(),
            cache_ref
        );

        let miss = CacheMissMetadata {
            cache_key_hi: 0x1122,
            cache_key_lo: 0x3344,
            miss_reason: CacheMissReason::SchemaMismatch,
            profile_id: 3,
            diagnostic_bytes: 9,
        };
        assert_eq!(
            CacheMissMetadata::parse(&miss.to_bytes().unwrap()).unwrap(),
            miss
        );
    }

    #[test]
    fn object_and_cache_metadata_reject_reserved_bits_and_fields() {
        assert_eq!(
            ObjectReferenceMetadata {
                object_id: 1,
                operation_id: 0,
                object_version: 0,
                offset: 0,
                length: 0,
                flags: 0x08,
                metadata_bytes: 0,
            }
            .to_bytes(),
            Err(NnrpError::ReservedBitsSet {
                value: 0x08,
                allowed: OBJECT_REFERENCE_FLAGS_KNOWN_MASK as u64
            })
        );

        let mut release = ObjectReleaseMetadata {
            object_id: 1,
            operation_id: 0,
            release_reason: ObjectReleaseReason::Completed,
            source_role: RuntimeRole::Runtime,
            flags: 0,
            diagnostic_bytes: 0,
        }
        .to_bytes()
        .unwrap();
        write_u64(&mut release, 24, 1);
        assert_eq!(
            ObjectReleaseMetadata::parse(&release),
            Err(NnrpError::NonZeroReservedField {
                field: "object_release.reserved"
            })
        );

        assert_eq!(
            ObjectDeltaMetadata {
                object_id: 1,
                delta_sequence: 1,
                region_offset: 0,
                region_bytes: 1,
                delta_bytes: 1,
                flags: 0x08,
                metadata_bytes: 0,
            }
            .to_bytes(),
            Err(NnrpError::ReservedBitsSet {
                value: 0x08,
                allowed: OBJECT_DELTA_FLAGS_KNOWN_MASK as u64
            })
        );

        assert_eq!(
            CacheReferenceMetadata {
                cache_key_hi: 1,
                cache_key_lo: 2,
                profile_id: 3,
                reuse_scope: CacheReuseScope::Operation,
                lease_id: 0,
                producer_trace_id: 0,
                expiration_hint_ms: 0,
                metadata_bytes: 0,
                flags: 0x04,
            }
            .to_bytes(),
            Err(NnrpError::ReservedBitsSet {
                value: 0x04,
                allowed: CACHE_REFERENCE_FLAGS_KNOWN_MASK as u64
            })
        );

        let mut miss = CacheMissMetadata {
            cache_key_hi: 1,
            cache_key_lo: 2,
            miss_reason: CacheMissReason::NotFound,
            profile_id: 3,
            diagnostic_bytes: 0,
        }
        .to_bytes()
        .unwrap();
        write_u64(&mut miss, 24, 1);
        assert_eq!(
            CacheMissMetadata::parse(&miss),
            Err(NnrpError::NonZeroReservedField {
                field: "cache_miss.reserved"
            })
        );
    }

    #[test]
    fn object_and_cache_metadata_reject_short_buffers_and_destinations() {
        assert_eq!(
            ObjectDescriptorMetadata::parse(&[0; OBJECT_DESCRIPTOR_METADATA_LEN - 1]),
            Err(NnrpError::SourceTooShort {
                expected: OBJECT_DESCRIPTOR_METADATA_LEN,
                actual: OBJECT_DESCRIPTOR_METADATA_LEN - 1
            })
        );
        assert_eq!(
            ObjectReferenceMetadata::parse(&[0; OBJECT_REFERENCE_METADATA_LEN - 1]),
            Err(NnrpError::SourceTooShort {
                expected: OBJECT_REFERENCE_METADATA_LEN,
                actual: OBJECT_REFERENCE_METADATA_LEN - 1
            })
        );

        let descriptor = ObjectDescriptorMetadata {
            object_id: 1,
            object_kind: RuntimeObjectKind::Tensor,
            producer_role: RuntimeRole::Runtime,
            consumer_role: RuntimeRole::Client,
            session_id: 2,
            byte_size: 4096,
            compute_cost_units: 7,
            memory_location_hint: MemoryLocationHint::DeviceMemory,
            ownership_hint: OwnershipHint::Borrowed,
            lifetime_hint_ms: 500,
            metadata_bytes: 12,
        };
        let mut descriptor_destination = [0u8; OBJECT_DESCRIPTOR_METADATA_LEN - 1];
        assert_eq!(
            descriptor.write(&mut descriptor_destination),
            Err(NnrpError::DestinationTooShort {
                expected: OBJECT_DESCRIPTOR_METADATA_LEN,
                actual: OBJECT_DESCRIPTOR_METADATA_LEN - 1
            })
        );

        let cache_miss = CacheMissMetadata {
            cache_key_hi: 1,
            cache_key_lo: 2,
            miss_reason: CacheMissReason::NotFound,
            profile_id: 3,
            diagnostic_bytes: 4,
        };
        let mut cache_miss_destination = [0u8; CACHE_MISS_METADATA_LEN - 1];
        assert_eq!(
            cache_miss.write(&mut cache_miss_destination),
            Err(NnrpError::DestinationTooShort {
                expected: CACHE_MISS_METADATA_LEN,
                actual: CACHE_MISS_METADATA_LEN - 1
            })
        );
    }

    #[test]
    fn object_and_cache_registries_accept_standard_values() {
        assert_eq!(
            (0x0000..=0x000c)
                .map(RuntimeObjectKind::try_from_u16)
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![
                RuntimeObjectKind::Unspecified,
                RuntimeObjectKind::Tensor,
                RuntimeObjectKind::TokenBlock,
                RuntimeObjectKind::ImageTile,
                RuntimeObjectKind::FeatureMap,
                RuntimeObjectKind::ToolResult,
                RuntimeObjectKind::TraceSegment,
                RuntimeObjectKind::OpaqueBytes,
                RuntimeObjectKind::DocumentChunk,
                RuntimeObjectKind::AudioChunk,
                RuntimeObjectKind::VideoChunk,
                RuntimeObjectKind::RoutePlan,
                RuntimeObjectKind::CacheManifest,
            ]
        );
        assert_eq!(
            (0x00..=0x07)
                .map(RuntimeRole::try_from_u8)
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![
                RuntimeRole::Unspecified,
                RuntimeRole::Client,
                RuntimeRole::Server,
                RuntimeRole::Runtime,
                RuntimeRole::Subagent,
                RuntimeRole::Tool,
                RuntimeRole::Scheduler,
                RuntimeRole::ConformanceRunner,
            ]
        );
        assert_eq!(
            (0x0000..=0x0006)
                .map(MemoryLocationHint::try_from_u16)
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![
                MemoryLocationHint::Unspecified,
                MemoryLocationHint::HostMemory,
                MemoryLocationHint::DeviceMemory,
                MemoryLocationHint::SharedMemory,
                MemoryLocationHint::RemoteMemory,
                MemoryLocationHint::MmapFile,
                MemoryLocationHint::ObjectStore,
            ]
        );
        assert_eq!(
            (0x0000..=0x0006)
                .map(OwnershipHint::try_from_u16)
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![
                OwnershipHint::Unspecified,
                OwnershipHint::ProducerOwned,
                OwnershipHint::ConsumerOwned,
                OwnershipHint::SessionOwned,
                OwnershipHint::Borrowed,
                OwnershipHint::TransferOnRef,
                OwnershipHint::ReleaseOnDrop,
            ]
        );
        assert_eq!(
            (0x0000..=0x0007)
                .map(ObjectReleaseReason::try_from_u16)
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![
                ObjectReleaseReason::Completed,
                ObjectReleaseReason::Cancelled,
                ObjectReleaseReason::Expired,
                ObjectReleaseReason::Replaced,
                ObjectReleaseReason::Invalidated,
                ObjectReleaseReason::OwnerClosed,
                ObjectReleaseReason::LeaseExpired,
                ObjectReleaseReason::ConformanceInjection,
            ]
        );
        assert_eq!(
            (0x0000..=0x0005)
                .map(CacheReuseScope::try_from_u16)
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![
                CacheReuseScope::Operation,
                CacheReuseScope::Session,
                CacheReuseScope::Connection,
                CacheReuseScope::Global,
                CacheReuseScope::Tenant,
                CacheReuseScope::Profile,
            ]
        );
        assert_eq!(
            (0x0000..=0x0007)
                .map(CacheMissReason::try_from_u16)
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![
                CacheMissReason::Unknown,
                CacheMissReason::NotFound,
                CacheMissReason::Expired,
                CacheMissReason::Invalidated,
                CacheMissReason::SchemaMismatch,
                CacheMissReason::ProducerUnavailable,
                CacheMissReason::LeaseRequired,
                CacheMissReason::PermissionDenied,
            ]
        );
    }

    #[test]
    fn object_and_cache_registries_reject_reserved_standard_values() {
        assert_eq!(
            RuntimeObjectKind::try_from_u16(0x000d),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "runtime_object_kind",
                value: 0x000d
            })
        );
        assert_eq!(
            RuntimeRole::try_from_u8(0x08),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "runtime_role",
                value: 0x08
            })
        );
        assert_eq!(
            MemoryLocationHint::try_from_u16(0x0007),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "memory_location_hint",
                value: 0x0007
            })
        );
        assert_eq!(
            OwnershipHint::try_from_u16(0x0007),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "ownership_hint",
                value: 0x0007
            })
        );
        assert_eq!(
            ObjectReleaseReason::try_from_u16(0x0008),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "object_release_reason",
                value: 0x0008
            })
        );
        assert_eq!(
            CacheReuseScope::try_from_u16(0x0006),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "cache_reuse_scope",
                value: 0x0006
            })
        );
        assert_eq!(
            CacheMissReason::try_from_u16(0x0008),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "cache_miss_reason",
                value: 0x0008
            })
        );
    }
}
