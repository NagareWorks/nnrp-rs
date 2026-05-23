use crate::{CacheObjectKind, CommonHeader, MessageType, NnrpError};

pub const FRAME_SUBMIT_METADATA_LEN: usize = 72;
pub const RESULT_PUSH_METADATA_LEN: usize = 64;
pub const BODY_REGION_PRELUDE_LEN: usize = 32;
pub const OBJECT_REFERENCE_BLOCK_LEN: usize = 20;

pub const BUDGET_POLICY_KNOWN_MASK: u8 = 0x0f;
pub const RESULT_FLAGS_KNOWN_MASK: u16 = 0x0007;
pub const PAYLOAD_KIND_KNOWN_MASK: u32 = 0x0000_007f;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InputProfile {
    Unspecified = 0,
    ChangedTilesLuma = 1,
    DenseLumaFrame = 2,
}

impl InputProfile {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Unspecified),
            1 => Ok(Self::ChangedTilesLuma),
            2 => Ok(Self::DenseLumaFrame),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "input_profile",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TileIndexMode {
    DenseRange = 0,
    RawU16 = 1,
    DeltaU16 = 2,
    Bitset = 3,
}

impl TileIndexMode {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::DenseRange),
            1 => Ok(Self::RawU16),
            2 => Ok(Self::DeltaU16),
            3 => Ok(Self::Bitset),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "tile_index_mode",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SubmitMode {
    Inline = 0,
    Reference = 1,
    Mixed = 2,
}

impl SubmitMode {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Inline),
            1 => Ok(Self::Reference),
            2 => Ok(Self::Mixed),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "submit_mode",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResultClass {
    Complete = 0,
    Partial = 1,
    StaleReuse = 2,
    Degraded = 3,
}

impl ResultClass {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Complete),
            1 => Ok(Self::Partial),
            2 => Ok(Self::StaleReuse),
            3 => Ok(Self::Degraded),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "result_class",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadKindBitmap(pub u32);

impl PayloadKindBitmap {
    pub const TENSOR: u32 = 0x0000_0001;
    pub const TOKEN_CHUNK: u32 = 0x0000_0002;
    pub const AUDIO_CHUNK: u32 = 0x0000_0004;
    pub const VIDEO_CHUNK: u32 = 0x0000_0008;
    pub const STRUCTURED_EVENT: u32 = 0x0000_0010;
    pub const TOOL_DELTA: u32 = 0x0000_0020;
    pub const OPAQUE_BYTES: u32 = 0x0000_0040;

    pub fn validate(self) -> Result<(), NnrpError> {
        validate_mask_u32(self.0, PAYLOAD_KIND_KNOWN_MASK)
    }

    pub fn contains_tensor(self) -> bool {
        self.0 & Self::TENSOR != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSubmitMetadata {
    pub src_width: u16,
    pub src_height: u16,
    pub tile_width: u16,
    pub tile_height: u16,
    pub tile_count: u16,
    pub section_count: u16,
    pub frame_class: u8,
    pub input_profile: InputProfile,
    pub tile_index_mode: TileIndexMode,
    pub latency_budget_ms: u16,
    pub target_fps_x100: u16,
    pub retry_of_frame: u32,
    pub tile_base_id: u32,
    pub camera_bytes: u32,
    pub tile_index_bytes: u32,
    pub submit_mode: SubmitMode,
    pub budget_policy: u8,
    pub loss_tolerance_policy: u8,
    pub object_ref_mask: u32,
    pub dependency_frame_id: u32,
    pub payload_kind_bitmap: PayloadKindBitmap,
    pub payload_frame_count: u16,
}

impl FrameSubmitMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, FRAME_SUBMIT_METADATA_LEN)?;
        validate_zero_u8("frame_submit.reserved0", source[15])?;
        validate_zero_u64("frame_submit.reserved1", read_u64(source, 32))?;
        validate_zero_u64("frame_submit.reserved2", read_u64(source, 40))?;
        validate_zero_u8("frame_submit.reserved3", source[51])?;
        validate_zero_u16("frame_submit.reserved4", read_u16(source, 70))?;

        let budget_policy = source[49];
        validate_mask_u8(budget_policy, BUDGET_POLICY_KNOWN_MASK)?;
        let payload_kind_bitmap = PayloadKindBitmap(read_u32(source, 60));
        payload_kind_bitmap.validate()?;

        let metadata = Self {
            src_width: read_u16(source, 0),
            src_height: read_u16(source, 2),
            tile_width: read_u16(source, 4),
            tile_height: read_u16(source, 6),
            tile_count: read_u16(source, 8),
            section_count: read_u16(source, 10),
            frame_class: source[12],
            input_profile: InputProfile::try_from_u8(source[13])?,
            tile_index_mode: TileIndexMode::try_from_u8(source[14])?,
            latency_budget_ms: read_u16(source, 16),
            target_fps_x100: read_u16(source, 18),
            retry_of_frame: read_u32(source, 20),
            tile_base_id: read_u32(source, 24),
            camera_bytes: read_u32(source, 28),
            tile_index_bytes: read_u32(source, 36),
            submit_mode: SubmitMode::try_from_u8(source[48])?,
            budget_policy,
            loss_tolerance_policy: source[50],
            object_ref_mask: read_u32(source, 52),
            dependency_frame_id: read_u32(source, 56),
            payload_kind_bitmap,
            payload_frame_count: read_u16(source, 64),
        };
        metadata.validate_payload_shape()?;
        Ok(metadata)
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, FRAME_SUBMIT_METADATA_LEN)?;
        validate_mask_u8(self.budget_policy, BUDGET_POLICY_KNOWN_MASK)?;
        self.payload_kind_bitmap.validate()?;
        self.validate_payload_shape()?;

        destination[..FRAME_SUBMIT_METADATA_LEN].fill(0);
        write_u16(destination, 0, self.src_width);
        write_u16(destination, 2, self.src_height);
        write_u16(destination, 4, self.tile_width);
        write_u16(destination, 6, self.tile_height);
        write_u16(destination, 8, self.tile_count);
        write_u16(destination, 10, self.section_count);
        destination[12] = self.frame_class;
        destination[13] = self.input_profile as u8;
        destination[14] = self.tile_index_mode as u8;
        write_u16(destination, 16, self.latency_budget_ms);
        write_u16(destination, 18, self.target_fps_x100);
        write_u32(destination, 20, self.retry_of_frame);
        write_u32(destination, 24, self.tile_base_id);
        write_u32(destination, 28, self.camera_bytes);
        write_u32(destination, 36, self.tile_index_bytes);
        destination[48] = self.submit_mode as u8;
        destination[49] = self.budget_policy;
        destination[50] = self.loss_tolerance_policy;
        write_u32(destination, 52, self.object_ref_mask);
        write_u32(destination, 56, self.dependency_frame_id);
        write_u32(destination, 60, self.payload_kind_bitmap.0);
        write_u16(destination, 64, self.payload_frame_count);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; FRAME_SUBMIT_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; FRAME_SUBMIT_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn validate_payload_shape(&self) -> Result<(), NnrpError> {
        if self.payload_kind_bitmap.contains_tensor() {
            return Ok(());
        }

        if self.src_width != 0
            || self.src_height != 0
            || self.tile_width != 0
            || self.tile_height != 0
            || self.tile_count != 0
            || self.section_count != 0
            || self.tile_base_id != 0
            || self.camera_bytes != 0
            || self.tile_index_bytes != 0
            || self.input_profile != InputProfile::Unspecified
        {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "non-tensor FRAME_SUBMIT must clear tensor tile fields",
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultPushMetadata {
    pub status_code: u16,
    pub result_flags: u16,
    pub section_count: u16,
    pub tile_count: u16,
    pub active_profile_id: u16,
    pub inference_ms: u16,
    pub queue_ms: u16,
    pub server_total_ms: u16,
    pub tile_base_id: u32,
    pub tile_index_bytes: u32,
    pub result_class: ResultClass,
    pub applied_budget_policy: u8,
    pub reused_frame_id: u32,
    pub covered_tile_count: u16,
    pub dropped_tile_count: u16,
    pub payload_kind_bitmap: PayloadKindBitmap,
    pub payload_frame_count: u16,
}

impl ResultPushMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, RESULT_PUSH_METADATA_LEN)?;
        validate_zero_u16("result_push.reserved0", read_u16(source, 10))?;
        validate_zero_u16("result_push.reserved1", read_u16(source, 18))?;
        validate_zero_u64("result_push.reserved2", read_u64(source, 28))?;
        validate_zero_u64("result_push.reserved3", read_u64(source, 36))?;
        validate_zero_u16("result_push.reserved4", read_u16(source, 46))?;
        validate_zero_u16("result_push.reserved5", read_u16(source, 62))?;

        let result_flags = read_u16(source, 2);
        validate_mask_u16(result_flags, RESULT_FLAGS_KNOWN_MASK)?;
        let applied_budget_policy = source[45];
        validate_mask_u8(applied_budget_policy, BUDGET_POLICY_KNOWN_MASK)?;
        let payload_kind_bitmap = PayloadKindBitmap(read_u32(source, 56));
        payload_kind_bitmap.validate()?;

        let metadata = Self {
            status_code: read_u16(source, 0),
            result_flags,
            section_count: read_u16(source, 4),
            tile_count: read_u16(source, 6),
            active_profile_id: read_u16(source, 8),
            inference_ms: read_u16(source, 12),
            queue_ms: read_u16(source, 14),
            server_total_ms: read_u16(source, 16),
            tile_base_id: read_u32(source, 20),
            tile_index_bytes: read_u32(source, 24),
            result_class: ResultClass::try_from_u8(source[44])?,
            applied_budget_policy,
            reused_frame_id: read_u32(source, 48),
            covered_tile_count: read_u16(source, 52),
            dropped_tile_count: read_u16(source, 54),
            payload_kind_bitmap,
            payload_frame_count: read_u16(source, 60),
        };
        metadata.validate_payload_shape()?;
        Ok(metadata)
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, RESULT_PUSH_METADATA_LEN)?;
        validate_mask_u16(self.result_flags, RESULT_FLAGS_KNOWN_MASK)?;
        validate_mask_u8(self.applied_budget_policy, BUDGET_POLICY_KNOWN_MASK)?;
        self.payload_kind_bitmap.validate()?;
        self.validate_payload_shape()?;

        destination[..RESULT_PUSH_METADATA_LEN].fill(0);
        write_u16(destination, 0, self.status_code);
        write_u16(destination, 2, self.result_flags);
        write_u16(destination, 4, self.section_count);
        write_u16(destination, 6, self.tile_count);
        write_u16(destination, 8, self.active_profile_id);
        write_u16(destination, 12, self.inference_ms);
        write_u16(destination, 14, self.queue_ms);
        write_u16(destination, 16, self.server_total_ms);
        write_u32(destination, 20, self.tile_base_id);
        write_u32(destination, 24, self.tile_index_bytes);
        destination[44] = self.result_class as u8;
        destination[45] = self.applied_budget_policy;
        write_u32(destination, 48, self.reused_frame_id);
        write_u16(destination, 52, self.covered_tile_count);
        write_u16(destination, 54, self.dropped_tile_count);
        write_u32(destination, 56, self.payload_kind_bitmap.0);
        write_u16(destination, 60, self.payload_frame_count);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; RESULT_PUSH_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; RESULT_PUSH_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn validate_payload_shape(&self) -> Result<(), NnrpError> {
        if self.payload_kind_bitmap.contains_tensor() {
            return Ok(());
        }

        if self.section_count != 0
            || self.tile_count != 0
            || self.tile_base_id != 0
            || self.tile_index_bytes != 0
            || self.covered_tile_count != 0
            || self.dropped_tile_count != 0
        {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "non-tensor RESULT_PUSH must clear tensor coverage fields",
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyRegionPrelude {
    pub inline_object_bytes: u32,
    pub object_reference_bytes: u32,
    pub typed_payload_descriptor_bytes: u32,
    pub typed_payload_frame_bytes: u32,
    pub extension_descriptor_bytes: u32,
    pub extension_payload_bytes: u32,
}

impl BodyRegionPrelude {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, BODY_REGION_PRELUDE_LEN)?;
        validate_zero_u32("body_region_prelude.body_flags", read_u32(source, 24))?;
        validate_zero_u32("body_region_prelude.reserved", read_u32(source, 28))?;

        let prelude = Self {
            inline_object_bytes: read_u32(source, 0),
            object_reference_bytes: read_u32(source, 4),
            typed_payload_descriptor_bytes: read_u32(source, 8),
            typed_payload_frame_bytes: read_u32(source, 12),
            extension_descriptor_bytes: read_u32(source, 16),
            extension_payload_bytes: read_u32(source, 20),
        };
        prelude.validate_alignment()?;
        Ok(prelude)
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, BODY_REGION_PRELUDE_LEN)?;
        self.validate_alignment()?;

        destination[..BODY_REGION_PRELUDE_LEN].fill(0);
        write_u32(destination, 0, self.inline_object_bytes);
        write_u32(destination, 4, self.object_reference_bytes);
        write_u32(destination, 8, self.typed_payload_descriptor_bytes);
        write_u32(destination, 12, self.typed_payload_frame_bytes);
        write_u32(destination, 16, self.extension_descriptor_bytes);
        write_u32(destination, 20, self.extension_payload_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; BODY_REGION_PRELUDE_LEN], NnrpError> {
        let mut bytes = [0u8; BODY_REGION_PRELUDE_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn total_region_bytes(&self) -> Result<u32, NnrpError> {
        [
            self.inline_object_bytes,
            self.object_reference_bytes,
            self.typed_payload_descriptor_bytes,
            self.typed_payload_frame_bytes,
            self.extension_descriptor_bytes,
            self.extension_payload_bytes,
        ]
        .into_iter()
        .try_fold(0u32, |sum, value| {
            sum.checked_add(value)
                .ok_or(NnrpError::MessageLengthOverflow)
        })
    }

    fn validate_alignment(&self) -> Result<(), NnrpError> {
        if self.object_reference_bytes as usize % OBJECT_REFERENCE_BLOCK_LEN != 0 {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "object_reference_bytes must be a multiple of object reference block length",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectReferenceBlock {
    pub object_kind: CacheObjectKind,
    pub ref_flags: u16,
    pub cache_namespace: u32,
    pub cache_key_hi: u32,
    pub cache_key_lo: u32,
}

impl ObjectReferenceBlock {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, OBJECT_REFERENCE_BLOCK_LEN)?;
        let ref_flags = read_u16(source, 2);
        validate_zero_u16("object_reference.ref_flags", ref_flags)?;

        Ok(Self {
            object_kind: CacheObjectKind::try_from_u32(read_u16(source, 0) as u32)?,
            ref_flags,
            cache_namespace: read_u32(source, 4),
            cache_key_hi: read_u32(source, 8),
            cache_key_lo: read_u32(source, 12),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, OBJECT_REFERENCE_BLOCK_LEN)?;
        validate_zero_u16("object_reference.ref_flags", self.ref_flags)?;

        destination[..OBJECT_REFERENCE_BLOCK_LEN].fill(0);
        write_u16(destination, 0, self.object_kind as u16);
        write_u32(destination, 4, self.cache_namespace);
        write_u32(destination, 8, self.cache_key_hi);
        write_u32(destination, 12, self.cache_key_lo);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; OBJECT_REFERENCE_BLOCK_LEN], NnrpError> {
        let mut bytes = [0u8; OBJECT_REFERENCE_BLOCK_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

pub fn validate_result_drop_header(header: &CommonHeader) -> Result<(), NnrpError> {
    if header.message_type != MessageType::ResultDrop
        || header.meta_len != 0
        || header.body_len != 0
    {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "RESULT_DROP is header-only and requires meta_len=0 and body_len=0",
        });
    }
    Ok(())
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

fn validate_zero_u8(field: &'static str, value: u8) -> Result<(), NnrpError> {
    if value != 0 {
        return Err(NnrpError::NonZeroReservedField { field });
    }
    Ok(())
}

fn validate_zero_u16(field: &'static str, value: u16) -> Result<(), NnrpError> {
    if value != 0 {
        return Err(NnrpError::NonZeroReservedField { field });
    }
    Ok(())
}

fn validate_zero_u32(field: &'static str, value: u32) -> Result<(), NnrpError> {
    if value != 0 {
        return Err(NnrpError::NonZeroReservedField { field });
    }
    Ok(())
}

fn validate_zero_u64(field: &'static str, value: u64) -> Result<(), NnrpError> {
    if value != 0 {
        return Err(NnrpError::NonZeroReservedField { field });
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

fn validate_mask_u16(value: u16, allowed: u16) -> Result<(), NnrpError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_submit_metadata_round_trips_current_v2_layout() {
        let metadata = FrameSubmitMetadata {
            src_width: 640,
            src_height: 360,
            tile_width: 32,
            tile_height: 32,
            tile_count: 84,
            section_count: 2,
            frame_class: 1,
            input_profile: InputProfile::DenseLumaFrame,
            tile_index_mode: TileIndexMode::DenseRange,
            latency_budget_ms: 100,
            target_fps_x100: 6000,
            retry_of_frame: 7,
            tile_base_id: 0,
            camera_bytes: 192,
            tile_index_bytes: 0,
            submit_mode: SubmitMode::Mixed,
            budget_policy: 0x05,
            loss_tolerance_policy: 0xff,
            object_ref_mask: 0x0000_0003,
            dependency_frame_id: 41,
            payload_kind_bitmap: PayloadKindBitmap(
                PayloadKindBitmap::TENSOR | PayloadKindBitmap::STRUCTURED_EVENT,
            ),
            payload_frame_count: 2,
        };
        let bytes = metadata.to_bytes().unwrap();

        assert_eq!(bytes.len(), FRAME_SUBMIT_METADATA_LEN);
        assert_eq!(FrameSubmitMetadata::parse(&bytes).unwrap(), metadata);
    }

    #[test]
    fn frame_submit_rejects_unknown_budget_and_non_tensor_shape() {
        let mut bytes = [0u8; FRAME_SUBMIT_METADATA_LEN];
        bytes[49] = 0x80;
        write_u32(&mut bytes, 60, PayloadKindBitmap::TENSOR);
        assert_eq!(
            FrameSubmitMetadata::parse(&bytes),
            Err(NnrpError::ReservedBitsSet {
                value: 0x80,
                allowed: BUDGET_POLICY_KNOWN_MASK as u64
            })
        );

        let metadata = FrameSubmitMetadata {
            src_width: 0,
            src_height: 0,
            tile_width: 0,
            tile_height: 0,
            tile_count: 1,
            section_count: 0,
            frame_class: 0,
            input_profile: InputProfile::Unspecified,
            tile_index_mode: TileIndexMode::DenseRange,
            latency_budget_ms: 0,
            target_fps_x100: 0,
            retry_of_frame: 0,
            tile_base_id: 0,
            camera_bytes: 0,
            tile_index_bytes: 0,
            submit_mode: SubmitMode::Inline,
            budget_policy: 0,
            loss_tolerance_policy: 0xff,
            object_ref_mask: 0,
            dependency_frame_id: 0,
            payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::STRUCTURED_EVENT),
            payload_frame_count: 1,
        };
        assert_eq!(
            metadata.to_bytes(),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "non-tensor FRAME_SUBMIT must clear tensor tile fields"
            })
        );
    }

    #[test]
    fn result_push_metadata_round_trips_current_v2_layout() {
        let metadata = ResultPushMetadata {
            status_code: 0,
            result_flags: 0x0004,
            section_count: 1,
            tile_count: 84,
            active_profile_id: 2,
            inference_ms: 843,
            queue_ms: 2,
            server_total_ms: 846,
            tile_base_id: 0,
            tile_index_bytes: 16,
            result_class: ResultClass::Partial,
            applied_budget_policy: 0x01,
            reused_frame_id: 41,
            covered_tile_count: 53,
            dropped_tile_count: 31,
            payload_kind_bitmap: PayloadKindBitmap(
                PayloadKindBitmap::TENSOR | PayloadKindBitmap::TOKEN_CHUNK,
            ),
            payload_frame_count: 3,
        };
        let bytes = metadata.to_bytes().unwrap();

        assert_eq!(bytes.len(), RESULT_PUSH_METADATA_LEN);
        assert_eq!(ResultPushMetadata::parse(&bytes).unwrap(), metadata);
    }

    #[test]
    fn result_push_rejects_unknown_payload_bits_and_non_tensor_coverage() {
        let mut bytes = [0u8; RESULT_PUSH_METADATA_LEN];
        write_u32(&mut bytes, 56, 0x8000_0000);
        assert_eq!(
            ResultPushMetadata::parse(&bytes),
            Err(NnrpError::ReservedBitsSet {
                value: 0x8000_0000,
                allowed: PAYLOAD_KIND_KNOWN_MASK as u64
            })
        );

        let metadata = ResultPushMetadata {
            status_code: 0,
            result_flags: 0,
            section_count: 0,
            tile_count: 0,
            active_profile_id: 0,
            inference_ms: 0,
            queue_ms: 0,
            server_total_ms: 0,
            tile_base_id: 0,
            tile_index_bytes: 0,
            result_class: ResultClass::Complete,
            applied_budget_policy: 0,
            reused_frame_id: 0,
            covered_tile_count: 1,
            dropped_tile_count: 0,
            payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
            payload_frame_count: 1,
        };
        assert_eq!(
            metadata.to_bytes(),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "non-tensor RESULT_PUSH must clear tensor coverage fields"
            })
        );
    }

    #[test]
    fn body_region_prelude_and_object_reference_round_trip() {
        let prelude = BodyRegionPrelude {
            inline_object_bytes: 16,
            object_reference_bytes: OBJECT_REFERENCE_BLOCK_LEN as u32,
            typed_payload_descriptor_bytes: 24,
            typed_payload_frame_bytes: 64,
            extension_descriptor_bytes: 0,
            extension_payload_bytes: 0,
        };
        let prelude_bytes = prelude.to_bytes().unwrap();

        assert_eq!(BodyRegionPrelude::parse(&prelude_bytes).unwrap(), prelude);
        assert_eq!(prelude.total_region_bytes().unwrap(), 124);

        let object_ref = ObjectReferenceBlock {
            object_kind: CacheObjectKind::TileIndexBlock,
            ref_flags: 0,
            cache_namespace: 7,
            cache_key_hi: 0x1122_3344,
            cache_key_lo: 0x5566_7788,
        };
        let object_ref_bytes = object_ref.to_bytes().unwrap();

        assert_eq!(
            ObjectReferenceBlock::parse(&object_ref_bytes).unwrap(),
            object_ref
        );
    }

    #[test]
    fn body_region_prelude_rejects_reserved_and_misaligned_reference_region() {
        let mut bytes = [0u8; BODY_REGION_PRELUDE_LEN];
        write_u32(&mut bytes, 24, 1);
        assert_eq!(
            BodyRegionPrelude::parse(&bytes),
            Err(NnrpError::NonZeroReservedField {
                field: "body_region_prelude.body_flags"
            })
        );

        let prelude = BodyRegionPrelude {
            inline_object_bytes: 0,
            object_reference_bytes: 1,
            typed_payload_descriptor_bytes: 0,
            typed_payload_frame_bytes: 0,
            extension_descriptor_bytes: 0,
            extension_payload_bytes: 0,
        };
        assert_eq!(
            prelude.to_bytes(),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "object_reference_bytes must be a multiple of object reference block length"
            })
        );
    }

    #[test]
    fn result_drop_is_header_only() {
        let header = CommonHeader::new(MessageType::ResultDrop, 0, 0);
        validate_result_drop_header(&header).unwrap();

        let bad = CommonHeader::new(MessageType::ResultDrop, 1, 0);
        assert_eq!(
            validate_result_drop_header(&bad),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "RESULT_DROP is header-only and requires meta_len=0 and body_len=0"
            })
        );
    }
}
