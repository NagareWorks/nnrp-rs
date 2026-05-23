use std::collections::BTreeMap;

use crate::{
    NnrpError, SCHEMA_ERROR_HASH_CONFLICT, SCHEMA_ERROR_INCOMPATIBLE, SCHEMA_ERROR_UNKNOWN,
    SCHEMA_ERROR_UPDATE_REJECTED, SCHEMA_ERROR_VERSION_UNKNOWN,
};

pub const SCHEMA_DESCRIPTOR_HEADER_LEN: usize = 32;
pub const TYPED_PAYLOAD_DESCRIPTOR_LEN: usize = 24;
pub const SCHEMA_FLAGS_KNOWN_MASK: u16 = 0x000f;
pub const DESCRIPTOR_FLAGS_KNOWN_MASK: u16 = 0x000f;
pub const PROFILE_UNSPECIFIED: u16 = 0;
pub const PROFILE_TENSOR: u16 = 1;
pub const PROFILE_TOKEN: u16 = 2;
pub const TOKEN_DELTA_SCHEMA_ID: u32 = 0x0000_1001;
pub const TOKEN_DELTA_SCHEMA_VERSION: u32 = 3;
pub const STREAM_SEMANTICS_TOKEN_DELTA: u16 = 2;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaRegistryAction {
    Installed,
    AlreadyInstalled,
    Updated,
    Invalidated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaRegistryFailure {
    Unknown,
    VersionUnknown,
    HashConflict,
    Incompatible,
    UpdateRejected,
}

impl SchemaRegistryFailure {
    pub fn error_code(self) -> u32 {
        match self {
            Self::Unknown => SCHEMA_ERROR_UNKNOWN,
            Self::VersionUnknown => SCHEMA_ERROR_VERSION_UNKNOWN,
            Self::HashConflict => SCHEMA_ERROR_HASH_CONFLICT,
            Self::Incompatible => SCHEMA_ERROR_INCOMPATIBLE,
            Self::UpdateRejected => SCHEMA_ERROR_UPDATE_REJECTED,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SchemaRegistry {
    entries: BTreeMap<(u32, u32), SchemaDescriptorHeader>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_standard_preview3_profiles() -> Self {
        let mut registry = Self::new();
        registry
            .install(token_delta_schema_descriptor())
            .expect("standard token delta schema is valid");
        registry
    }

    pub fn install(
        &mut self,
        descriptor: SchemaDescriptorHeader,
    ) -> Result<SchemaRegistryAction, SchemaRegistryFailure> {
        validate_profile_assignment(descriptor.profile_id)?;

        let key = (descriptor.schema_id, descriptor.schema_version);
        if let Some(existing) = self.entries.get(&key) {
            if existing.schema_hash == descriptor.schema_hash
                && existing.profile_id == descriptor.profile_id
            {
                return Ok(SchemaRegistryAction::AlreadyInstalled);
            }

            return Err(SchemaRegistryFailure::HashConflict);
        }

        let has_older_version = self.entries.keys().any(|(schema_id, version)| {
            *schema_id == descriptor.schema_id && *version < descriptor.schema_version
        });
        self.entries.insert(key, descriptor);

        if has_older_version {
            Ok(SchemaRegistryAction::Updated)
        } else {
            Ok(SchemaRegistryAction::Installed)
        }
    }

    pub fn get(&self, schema_id: u32, schema_version: u32) -> Option<&SchemaDescriptorHeader> {
        self.entries.get(&(schema_id, schema_version))
    }

    pub fn invalidate(
        &mut self,
        schema_id: u32,
        schema_version: u32,
    ) -> Result<SchemaRegistryAction, SchemaRegistryFailure> {
        self.entries
            .remove(&(schema_id, schema_version))
            .map(|_| SchemaRegistryAction::Invalidated)
            .ok_or(SchemaRegistryFailure::VersionUnknown)
    }

    pub fn validate_descriptor_binding(
        &self,
        descriptor: &TypedPayloadDescriptor,
    ) -> Result<(), SchemaRegistryFailure> {
        validate_profile_assignment(descriptor.profile_id)?;

        if descriptor.profile_id == PROFILE_UNSPECIFIED {
            if descriptor.schema_id == 0 && descriptor.schema_version == 0 {
                return Ok(());
            }

            return Err(SchemaRegistryFailure::Incompatible);
        }

        if descriptor.schema_id == 0 {
            return Err(SchemaRegistryFailure::Unknown);
        }

        let Some(schema) = self.get(descriptor.schema_id, descriptor.schema_version) else {
            if self
                .entries
                .keys()
                .any(|(schema_id, _)| *schema_id == descriptor.schema_id)
            {
                return Err(SchemaRegistryFailure::VersionUnknown);
            }

            return Err(SchemaRegistryFailure::Unknown);
        };

        if schema.profile_id != descriptor.profile_id {
            return Err(SchemaRegistryFailure::Incompatible);
        }

        Ok(())
    }
}

pub fn token_delta_schema_descriptor() -> SchemaDescriptorHeader {
    SchemaDescriptorHeader {
        schema_id: TOKEN_DELTA_SCHEMA_ID,
        schema_version: TOKEN_DELTA_SCHEMA_VERSION,
        profile_id: PROFILE_TOKEN,
        schema_flags: 0,
        min_version_major: 1,
        max_version_major: 1,
        body_bytes: 0,
        dependency_count: 0,
        default_stream_semantics: STREAM_SEMANTICS_TOKEN_DELTA,
        schema_hash: 0x6e6e_7270_746f_6b33,
    }
}

pub fn validate_profile_assignment(profile_id: u16) -> Result<(), SchemaRegistryFailure> {
    match profile_id {
        PROFILE_UNSPECIFIED | PROFILE_TENSOR | PROFILE_TOKEN => Ok(()),
        _ => Err(SchemaRegistryFailure::UpdateRejected),
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
    use super::{
        token_delta_schema_descriptor, SchemaDescriptorHeader, SchemaRegistry,
        SchemaRegistryAction, SchemaRegistryFailure, TypedPayloadDescriptor, PROFILE_TENSOR,
        PROFILE_TOKEN, PROFILE_UNSPECIFIED, STREAM_SEMANTICS_TOKEN_DELTA, TOKEN_DELTA_SCHEMA_ID,
        TOKEN_DELTA_SCHEMA_VERSION,
    };
    use crate::{
        NnrpError, DESCRIPTOR_FLAGS_KNOWN_MASK, SCHEMA_ERROR_HASH_CONFLICT,
        SCHEMA_ERROR_INCOMPATIBLE, SCHEMA_ERROR_UPDATE_REJECTED, SCHEMA_FLAGS_KNOWN_MASK,
    };

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

    #[test]
    fn schema_registry_installs_updates_and_rejects_hash_conflicts() {
        let mut registry = SchemaRegistry::new();
        let descriptor = schema_descriptor(0x20, 1, PROFILE_TENSOR, 0x11);

        assert_eq!(
            registry.install(descriptor),
            Ok(SchemaRegistryAction::Installed)
        );
        assert_eq!(
            registry.install(descriptor),
            Ok(SchemaRegistryAction::AlreadyInstalled)
        );

        let conflict = SchemaDescriptorHeader {
            schema_hash: 0x12,
            ..descriptor
        };
        assert_eq!(
            registry.install(conflict),
            Err(SchemaRegistryFailure::HashConflict)
        );
        assert_eq!(
            SchemaRegistryFailure::HashConflict.error_code(),
            SCHEMA_ERROR_HASH_CONFLICT
        );

        let newer = schema_descriptor(0x20, 2, PROFILE_TENSOR, 0x22);
        assert_eq!(registry.install(newer), Ok(SchemaRegistryAction::Updated));
        assert_eq!(registry.get(0x20, 2), Some(&newer));
        assert_eq!(
            registry.invalidate(0x20, 2),
            Ok(SchemaRegistryAction::Invalidated)
        );
        assert_eq!(
            registry.invalidate(0x20, 2),
            Err(SchemaRegistryFailure::VersionUnknown)
        );
    }

    #[test]
    fn schema_registry_validates_descriptor_bindings_without_implicit_tensor_default() {
        let mut registry = SchemaRegistry::new();
        registry
            .install(schema_descriptor(0x30, 1, PROFILE_TENSOR, 0x33))
            .unwrap();

        assert_eq!(
            registry.validate_descriptor_binding(&typed_descriptor(PROFILE_UNSPECIFIED, 0, 0, 0)),
            Ok(())
        );
        assert_eq!(
            registry.validate_descriptor_binding(&typed_descriptor(
                PROFILE_UNSPECIFIED,
                0x30,
                1,
                0
            )),
            Err(SchemaRegistryFailure::Incompatible)
        );
        assert_eq!(
            SchemaRegistryFailure::Incompatible.error_code(),
            SCHEMA_ERROR_INCOMPATIBLE
        );

        assert_eq!(
            registry.validate_descriptor_binding(&typed_descriptor(PROFILE_TENSOR, 0x30, 1, 0)),
            Ok(())
        );
        assert_eq!(
            registry.validate_descriptor_binding(&typed_descriptor(PROFILE_TOKEN, 0x30, 1, 0)),
            Err(SchemaRegistryFailure::Incompatible)
        );
        assert_eq!(
            registry.validate_descriptor_binding(&typed_descriptor(PROFILE_TENSOR, 0x30, 2, 0)),
            Err(SchemaRegistryFailure::VersionUnknown)
        );
        assert_eq!(
            registry.validate_descriptor_binding(&typed_descriptor(PROFILE_TENSOR, 0x31, 1, 0)),
            Err(SchemaRegistryFailure::Unknown)
        );
    }

    #[test]
    fn schema_registry_exposes_standard_preview3_token_profile() {
        let registry = SchemaRegistry::with_standard_preview3_profiles();
        let descriptor = token_delta_schema_descriptor();

        assert_eq!(descriptor.schema_id, TOKEN_DELTA_SCHEMA_ID);
        assert_eq!(descriptor.schema_version, TOKEN_DELTA_SCHEMA_VERSION);
        assert_eq!(descriptor.profile_id, PROFILE_TOKEN);
        assert_eq!(
            descriptor.default_stream_semantics,
            STREAM_SEMANTICS_TOKEN_DELTA
        );
        assert_eq!(
            registry.validate_descriptor_binding(&typed_descriptor(
                PROFILE_TOKEN,
                TOKEN_DELTA_SCHEMA_ID,
                TOKEN_DELTA_SCHEMA_VERSION,
                STREAM_SEMANTICS_TOKEN_DELTA
            )),
            Ok(())
        );
    }

    #[test]
    fn schema_registry_rejects_unknown_public_profile_assignments() {
        let mut registry = SchemaRegistry::new();
        assert_eq!(
            registry.install(schema_descriptor(0x40, 1, 0xffff, 0x44)),
            Err(SchemaRegistryFailure::UpdateRejected)
        );
        assert_eq!(
            SchemaRegistryFailure::UpdateRejected.error_code(),
            SCHEMA_ERROR_UPDATE_REJECTED
        );
    }

    fn schema_descriptor(
        schema_id: u32,
        schema_version: u32,
        profile_id: u16,
        schema_hash: u64,
    ) -> SchemaDescriptorHeader {
        SchemaDescriptorHeader {
            schema_id,
            schema_version,
            profile_id,
            schema_flags: 0,
            min_version_major: 1,
            max_version_major: 1,
            body_bytes: 0,
            dependency_count: 0,
            default_stream_semantics: 0,
            schema_hash,
        }
    }

    fn typed_descriptor(
        profile_id: u16,
        schema_id: u32,
        schema_version: u32,
        stream_semantics: u16,
    ) -> TypedPayloadDescriptor {
        TypedPayloadDescriptor {
            profile_id,
            descriptor_flags: 0,
            schema_id,
            schema_version,
            stream_semantics,
            offset: 0,
            length: 0,
        }
    }

    fn hex_to_bytes(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect()
    }
}
