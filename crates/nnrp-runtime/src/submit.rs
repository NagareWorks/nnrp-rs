use nnrp_core::{
    BodyRegionPrelude, BudgetPolicy, CacheObjectKind, FrameSubmitMetadata, HeaderFlags,
    InlineObjectBlockHeader, InputProfile, LossTolerancePolicy, NnrpError, ObjectReferenceBlock,
    ObjectReferenceRegion, PayloadKind, PayloadKindBitmap, SubmitMode, TensorSectionDescriptor,
    TileIndexMode, TypedPayloadDescriptor, TypedPayloadRegion, BODY_REGION_PRELUDE_LEN,
    PROFILE_TOKEN, STREAM_SEMANTICS_TOKEN_DELTA, TENSOR_SECTION_DESCRIPTOR_LEN,
    TOKEN_DELTA_SCHEMA_ID, TOKEN_DELTA_SCHEMA_VERSION,
};

const ALIGNMENT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpSubmitHeaderContext {
    pub flags: HeaderFlags,
    pub view_id: u16,
    pub route_id: u16,
    pub trace_id: u64,
}

impl Default for NnrpSubmitHeaderContext {
    fn default() -> Self {
        Self {
            flags: HeaderFlags::NONE,
            view_id: 0,
            route_id: 0,
            trace_id: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpSubmitIdentity {
    pub operation_id: u64,
    pub frame_id: u32,
    pub header: NnrpSubmitHeaderContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpSubmitPolicy {
    pub frame_class: u8,
    pub latency_budget_ms: u16,
    pub target_fps_x100: u16,
    pub retry_of_frame: u32,
    pub budget_policy: BudgetPolicy,
    pub loss_tolerance_policy: LossTolerancePolicy,
    pub dependency_frame_id: u32,
}

impl Default for NnrpSubmitPolicy {
    fn default() -> Self {
        Self {
            frame_class: 0,
            latency_budget_ms: 0,
            target_fps_x100: 0,
            retry_of_frame: 0,
            budget_policy: BudgetPolicy::NONE,
            loss_tolerance_policy: LossTolerancePolicy::InheritSession,
            dependency_frame_id: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpTensorSection {
    pub role_id: u16,
    pub default_codec_id: u8,
    pub dtype_id: u8,
    pub layout_id: u8,
    pub scale_policy: u8,
    pub element_count_per_tile: u32,
    pub tile_payloads: Vec<Vec<u8>>,
    pub codec_ids: Vec<u8>,
    pub payload_stride_bytes: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NnrpSubmitObjectReferences {
    pub camera: Option<ObjectReferenceBlock>,
    pub tile_index: Option<ObjectReferenceBlock>,
    pub tensor_section_table: Option<ObjectReferenceBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpTensorSubmitInput {
    pub identity: NnrpSubmitIdentity,
    pub policy: NnrpSubmitPolicy,
    pub src_width: u16,
    pub src_height: u16,
    pub tile_width: u16,
    pub tile_height: u16,
    pub tile_ids: Vec<u16>,
    pub sections: Vec<NnrpTensorSection>,
    pub camera_block: Vec<u8>,
    pub input_profile: InputProfile,
    pub tile_index_mode: TileIndexMode,
    pub tile_base_id: u32,
    pub references: NnrpSubmitObjectReferences,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpTokenChunk {
    pub payload: Vec<u8>,
    pub descriptor_flags: u8,
}

impl NnrpTokenChunk {
    pub fn partial(payload: impl Into<Vec<u8>>) -> Self {
        Self {
            payload: payload.into(),
            descriptor_flags: 0x02,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpTokenSubmitInput {
    pub identity: NnrpSubmitIdentity,
    pub policy: NnrpSubmitPolicy,
    pub chunks: Vec<NnrpTokenChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpTypedPayloadInputFrame {
    pub profile_id: u16,
    pub payload_kind: PayloadKind,
    pub descriptor_flags: u8,
    pub schema_id: u32,
    pub schema_version: u32,
    pub stream_semantics: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpTypedPayloadSubmitInput {
    pub identity: NnrpSubmitIdentity,
    pub policy: NnrpSubmitPolicy,
    pub frames: Vec<NnrpTypedPayloadInputFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpSubmitRequest {
    pub operation_id: u64,
    pub frame_id: u32,
    pub header: NnrpSubmitHeaderContext,
    pub metadata: FrameSubmitMetadata,
    pub body: Vec<u8>,
}

impl NnrpSubmitRequest {
    pub fn tensor(input: NnrpTensorSubmitInput) -> Result<Self, NnrpError> {
        validate_identity(input.identity)?;
        input.identity.header.flags.validate_known()?;
        if input.tile_ids.len() > u16::MAX as usize || input.sections.len() > u16::MAX as usize {
            return Err(NnrpError::MessageLengthOverflow);
        }

        let tile_index_payload =
            encode_tile_indices(&input.tile_ids, input.tile_index_mode, input.tile_base_id)?;
        let section_payloads = encode_tensor_sections(&input.sections, input.tile_ids.len())?;
        let references = standard_references(&input.references)?;
        let object_ref_mask = reference_mask(&references);
        let has_inline = (!input.camera_block.is_empty() && input.references.camera.is_none())
            || (!input.tile_ids.is_empty() && input.references.tile_index.is_none())
            || (!input.sections.is_empty() && input.references.tensor_section_table.is_none());
        let has_references = !references.is_empty();
        let submit_mode = match (has_inline, has_references) {
            (false, true) => SubmitMode::Reference,
            (true, true) => SubmitMode::Mixed,
            _ => SubmitMode::Inline,
        };

        let mut inline_region = Vec::new();
        if input.references.camera.is_none() && !input.camera_block.is_empty() {
            append_inline_object(
                &mut inline_region,
                CacheObjectKind::CameraBlock,
                &input.camera_block,
            )?;
        }
        if input.references.tile_index.is_none() && !input.tile_ids.is_empty() {
            append_inline_object(
                &mut inline_region,
                CacheObjectKind::TileIndexBlock,
                &tile_index_payload,
            )?;
        }
        if input.references.tensor_section_table.is_none() && !section_payloads.is_empty() {
            let mut section_region = Vec::new();
            for section in &section_payloads {
                align(&mut section_region);
                section_region.extend_from_slice(section);
            }
            append_inline_object(
                &mut inline_region,
                CacheObjectKind::TensorSectionTable,
                &section_region,
            )?;
        }

        let reference_region = ObjectReferenceRegion::from_blocks(references);
        reference_region.validate_submit_mask(submit_mode, object_ref_mask)?;
        let reference_bytes = reference_region.to_bytes()?;
        let body = encode_body(&inline_region, &reference_bytes, &[], &[])?;
        let metadata = FrameSubmitMetadata {
            src_width: input.src_width,
            src_height: input.src_height,
            tile_width: input.tile_width,
            tile_height: input.tile_height,
            tile_count: input.tile_ids.len() as u16,
            section_count: input.sections.len() as u16,
            frame_class: input.policy.frame_class,
            input_profile: input.input_profile,
            tile_index_mode: input.tile_index_mode,
            latency_budget_ms: input.policy.latency_budget_ms,
            target_fps_x100: input.policy.target_fps_x100,
            retry_of_frame: input.policy.retry_of_frame,
            tile_base_id: input.tile_base_id,
            camera_bytes: if input.references.camera.is_some() {
                0
            } else {
                to_u32(input.camera_block.len())?
            },
            tile_index_bytes: if input.references.tile_index.is_some() {
                0
            } else {
                to_u32(tile_index_payload.len())?
            },
            operation_id: input.identity.operation_id,
            submit_mode,
            budget_policy: input.policy.budget_policy,
            loss_tolerance_policy: input.policy.loss_tolerance_policy,
            object_ref_mask,
            dependency_frame_id: input.policy.dependency_frame_id,
            payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TENSOR),
            payload_frame_count: 0,
        };
        metadata.to_bytes()?;
        Ok(Self::new(input.identity, metadata, body))
    }

    pub fn token(input: NnrpTokenSubmitInput) -> Result<Self, NnrpError> {
        let frames = input
            .chunks
            .into_iter()
            .map(|chunk| NnrpTypedPayloadInputFrame {
                profile_id: PROFILE_TOKEN,
                payload_kind: PayloadKind::TokenChunk,
                descriptor_flags: chunk.descriptor_flags,
                schema_id: TOKEN_DELTA_SCHEMA_ID,
                schema_version: TOKEN_DELTA_SCHEMA_VERSION,
                stream_semantics: STREAM_SEMANTICS_TOKEN_DELTA,
                payload: chunk.payload,
            })
            .collect();
        Self::typed_payload(NnrpTypedPayloadSubmitInput {
            identity: input.identity,
            policy: input.policy,
            frames,
        })
    }

    pub fn typed_payload(input: NnrpTypedPayloadSubmitInput) -> Result<Self, NnrpError> {
        validate_identity(input.identity)?;
        input.identity.header.flags.validate_known()?;
        if input.frames.is_empty() || input.frames.len() > u16::MAX as usize {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "typed payload submit requires between one and 65535 frames",
            });
        }
        let frame_count = input.frames.len() as u16;
        let mut descriptors = Vec::with_capacity(input.frames.len());
        let mut payload_region = Vec::new();
        let mut bitmap = 0u32;
        for frame in input.frames {
            let offset = to_u32(payload_region.len())?;
            let length = to_u32(frame.payload.len())?;
            let descriptor = TypedPayloadDescriptor {
                profile_id: frame.profile_id,
                payload_kind: frame.payload_kind,
                descriptor_flags: frame.descriptor_flags,
                schema_id: frame.schema_id,
                schema_version: frame.schema_version,
                stream_semantics: frame.stream_semantics,
                offset,
                length,
            };
            descriptors.push(descriptor);
            payload_region.extend_from_slice(&frame.payload);
            bitmap |= frame.payload_kind.bit();
        }
        let payload_kind_bitmap = PayloadKindBitmap(bitmap);
        let typed_region =
            TypedPayloadRegion::from_parts(payload_kind_bitmap, descriptors, &payload_region)?;
        let descriptor_region = typed_region.descriptor_region_bytes()?;
        let body = encode_body(&[], &[], &descriptor_region, &payload_region)?;
        let metadata = FrameSubmitMetadata {
            src_width: 0,
            src_height: 0,
            tile_width: 0,
            tile_height: 0,
            tile_count: 0,
            section_count: 0,
            frame_class: input.policy.frame_class,
            input_profile: InputProfile::Unspecified,
            tile_index_mode: TileIndexMode::RawU16,
            latency_budget_ms: input.policy.latency_budget_ms,
            target_fps_x100: input.policy.target_fps_x100,
            retry_of_frame: input.policy.retry_of_frame,
            tile_base_id: 0,
            camera_bytes: 0,
            tile_index_bytes: 0,
            operation_id: input.identity.operation_id,
            submit_mode: SubmitMode::Inline,
            budget_policy: input.policy.budget_policy,
            loss_tolerance_policy: input.policy.loss_tolerance_policy,
            object_ref_mask: 0,
            dependency_frame_id: input.policy.dependency_frame_id,
            payload_kind_bitmap,
            payload_frame_count: frame_count,
        };
        metadata.to_bytes()?;
        Ok(Self::new(input.identity, metadata, body))
    }

    fn new(identity: NnrpSubmitIdentity, metadata: FrameSubmitMetadata, body: Vec<u8>) -> Self {
        Self {
            operation_id: identity.operation_id,
            frame_id: identity.frame_id,
            header: identity.header,
            metadata,
            body,
        }
    }

    pub fn encoded_payload(&self) -> Result<Vec<u8>, NnrpError> {
        let mut payload = Vec::with_capacity(72 + self.body.len());
        payload.extend_from_slice(&self.metadata.to_bytes()?);
        payload.extend_from_slice(&self.body);
        Ok(payload)
    }
}

fn validate_identity(identity: NnrpSubmitIdentity) -> Result<(), NnrpError> {
    if identity.operation_id == 0 || identity.frame_id == 0 {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "submit operation_id and frame_id must be non-zero",
        });
    }
    Ok(())
}

fn encode_body(
    inline_region: &[u8],
    reference_region: &[u8],
    descriptor_region: &[u8],
    payload_region: &[u8],
) -> Result<Vec<u8>, NnrpError> {
    let prelude = BodyRegionPrelude {
        inline_object_bytes: to_u32(inline_region.len())?,
        object_reference_bytes: to_u32(reference_region.len())?,
        typed_payload_descriptor_bytes: to_u32(descriptor_region.len())?,
        typed_payload_frame_bytes: to_u32(payload_region.len())?,
        extension_descriptor_bytes: 0,
        extension_payload_bytes: 0,
    };
    let mut body = Vec::with_capacity(
        BODY_REGION_PRELUDE_LEN
            + inline_region.len()
            + reference_region.len()
            + descriptor_region.len()
            + payload_region.len(),
    );
    body.extend_from_slice(&prelude.to_bytes()?);
    body.extend_from_slice(inline_region);
    body.extend_from_slice(reference_region);
    body.extend_from_slice(descriptor_region);
    body.extend_from_slice(payload_region);
    Ok(body)
}

fn append_inline_object(
    region: &mut Vec<u8>,
    object_kind: CacheObjectKind,
    payload: &[u8],
) -> Result<(), NnrpError> {
    let header = InlineObjectBlockHeader {
        object_kind,
        object_flags: 0,
        profile_id: 0,
        object_bytes: to_u32(payload.len())?,
    };
    region.extend_from_slice(&header.to_bytes()?);
    region.extend_from_slice(payload);
    align(region);
    Ok(())
}

fn standard_references(
    references: &NnrpSubmitObjectReferences,
) -> Result<Vec<ObjectReferenceBlock>, NnrpError> {
    let expected = [
        (references.camera, CacheObjectKind::CameraBlock),
        (references.tile_index, CacheObjectKind::TileIndexBlock),
        (
            references.tensor_section_table,
            CacheObjectKind::TensorSectionTable,
        ),
    ];
    let mut blocks = Vec::new();
    for (block, kind) in expected {
        if let Some(block) = block {
            if block.object_kind != kind {
                return Err(NnrpError::InvalidProtocolCombination {
                    rule: "submit object reference is in the wrong standard slot",
                });
            }
            blocks.push(block);
        }
    }
    Ok(blocks)
}

fn reference_mask(blocks: &[ObjectReferenceBlock]) -> u32 {
    blocks.iter().fold(0, |mask, block| {
        mask | match block.object_kind {
            CacheObjectKind::CameraBlock => 1 << 0,
            CacheObjectKind::TileIndexBlock => 1 << 1,
            CacheObjectKind::TensorSectionTable => 1 << 2,
            CacheObjectKind::PayloadLayoutTemplate => 1 << 3,
            _ => 0,
        }
    })
}

fn encode_tile_indices(
    tile_ids: &[u16],
    mode: TileIndexMode,
    tile_base_id: u32,
) -> Result<Vec<u8>, NnrpError> {
    match mode {
        TileIndexMode::DenseRange => {
            for (index, tile_id) in tile_ids.iter().copied().enumerate() {
                if u32::from(tile_id) != tile_base_id + index as u32 {
                    return Err(NnrpError::InvalidProtocolCombination {
                        rule: "dense tile ids must be contiguous from tile_base_id",
                    });
                }
            }
            Ok(Vec::new())
        }
        TileIndexMode::RawU16 => Ok(tile_ids.iter().flat_map(|id| id.to_le_bytes()).collect()),
        TileIndexMode::DeltaU16 => {
            let mut encoded = Vec::with_capacity(tile_ids.len() * 2);
            let mut previous = None;
            for tile_id in tile_ids.iter().copied() {
                let value = previous.map_or(u32::from(tile_id), |prior| {
                    u32::from(tile_id).saturating_sub(u32::from(prior))
                });
                if previous.is_some_and(|prior| tile_id <= prior) || value > u16::MAX as u32 {
                    return Err(NnrpError::InvalidProtocolCombination {
                        rule: "delta tile ids must be strictly increasing with u16 deltas",
                    });
                }
                encoded.extend_from_slice(&(value as u16).to_le_bytes());
                previous = Some(tile_id);
            }
            Ok(encoded)
        }
        TileIndexMode::Bitset => {
            if tile_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(NnrpError::InvalidProtocolCombination {
                    rule: "bitset tile ids must be strictly increasing",
                });
            }
            let mut encoded = vec![0u8; tile_ids.last().map_or(0, |id| usize::from(*id) / 8 + 1)];
            for id in tile_ids {
                encoded[usize::from(*id) / 8] |= 1 << (id % 8);
            }
            Ok(encoded)
        }
    }
}

fn encode_tensor_sections(
    sections: &[NnrpTensorSection],
    tile_count: usize,
) -> Result<Vec<Vec<u8>>, NnrpError> {
    let mut previous_role = None;
    sections
        .iter()
        .map(|section| {
            if previous_role.is_some_and(|role| section.role_id <= role) {
                return Err(NnrpError::InvalidProtocolCombination {
                    rule: "tensor sections must be strictly ordered by role_id",
                });
            }
            previous_role = Some(section.role_id);
            if section.tile_payloads.len() != tile_count {
                return Err(NnrpError::InvalidProtocolCombination {
                    rule: "tensor section tile payload count must match tile_count",
                });
            }
            if !section.codec_ids.is_empty() && section.codec_ids.len() != tile_count {
                return Err(NnrpError::InvalidProtocolCombination {
                    rule: "tensor codec id count must match tile_count",
                });
            }
            let mixed_codec = !section.codec_ids.is_empty()
                && section
                    .codec_ids
                    .iter()
                    .any(|codec| *codec != section.default_codec_id);
            let codec_table = if mixed_codec {
                section.codec_ids.as_slice()
            } else {
                &[]
            };
            let mut length_table = Vec::with_capacity(tile_count * 4);
            let mut payload = Vec::new();
            for tile_payload in &section.tile_payloads {
                length_table.extend_from_slice(&to_u32(tile_payload.len())?.to_le_bytes());
                if section.payload_stride_bytes == 0 {
                    payload.extend_from_slice(tile_payload);
                } else {
                    let stride = section.payload_stride_bytes as usize;
                    if tile_payload.len() > stride {
                        return Err(NnrpError::InvalidProtocolCombination {
                            rule: "tensor tile payload exceeds fixed stride",
                        });
                    }
                    payload.extend_from_slice(tile_payload);
                    payload.resize(payload.len() + stride - tile_payload.len(), 0);
                }
            }
            let descriptor = TensorSectionDescriptor {
                role_id: section.role_id,
                codec_id: section.default_codec_id,
                dtype_id: section.dtype_id,
                layout_id: section.layout_id,
                scale_policy: section.scale_policy,
                section_flags: (if mixed_codec {
                    TensorSectionDescriptor::MIXED_CODEC
                } else {
                    0
                }) | (if section.payload_stride_bytes != 0 {
                    TensorSectionDescriptor::FIXED_STRIDE
                } else {
                    0
                }),
                element_count_per_tile: section.element_count_per_tile,
                codec_table_bytes: to_u32(codec_table.len())?,
                length_table_bytes: to_u32(length_table.len())?,
                payload_bytes: to_u32(payload.len())?,
                payload_stride_bytes: section.payload_stride_bytes,
            };
            let mut encoded = Vec::with_capacity(
                TENSOR_SECTION_DESCRIPTOR_LEN
                    + codec_table.len()
                    + length_table.len()
                    + payload.len(),
            );
            encoded.extend_from_slice(&descriptor.to_bytes()?);
            encoded.extend_from_slice(codec_table);
            encoded.extend_from_slice(&length_table);
            encoded.extend_from_slice(&payload);
            Ok(encoded)
        })
        .collect()
}

fn align(bytes: &mut Vec<u8>) {
    let aligned = (bytes.len() + ALIGNMENT - 1) & !(ALIGNMENT - 1);
    bytes.resize(aligned, 0);
}

fn to_u32(value: usize) -> Result<u32, NnrpError> {
    value
        .try_into()
        .map_err(|_| NnrpError::MessageLengthOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> NnrpSubmitIdentity {
        NnrpSubmitIdentity {
            operation_id: 7,
            frame_id: 9,
            header: NnrpSubmitHeaderContext {
                flags: HeaderFlags::ACK_REQUIRED,
                view_id: 3,
                route_id: 4,
                trace_id: 5,
            },
        }
    }

    fn reference(object_kind: CacheObjectKind, key: u64) -> ObjectReferenceBlock {
        ObjectReferenceBlock {
            object_kind,
            ref_flags: 0,
            cache_namespace: 1,
            cache_key_hi: 0,
            cache_key_lo: key,
        }
    }

    fn tensor_section(role_id: u16) -> NnrpTensorSection {
        NnrpTensorSection {
            role_id,
            default_codec_id: 2,
            dtype_id: 3,
            layout_id: 4,
            scale_policy: 0,
            element_count_per_tile: 16,
            tile_payloads: vec![vec![1], vec![2, 3]],
            codec_ids: Vec::new(),
            payload_stride_bytes: 0,
        }
    }

    #[test]
    fn token_builder_derives_descriptor_regions_and_bitmap() {
        let request = NnrpSubmitRequest::token(NnrpTokenSubmitInput {
            identity: identity(),
            policy: NnrpSubmitPolicy::default(),
            chunks: vec![NnrpTokenChunk::partial(b"abc".to_vec())],
        })
        .unwrap();
        assert_eq!(
            request.metadata.payload_kind_bitmap.0,
            PayloadKindBitmap::TOKEN_CHUNK
        );
        assert_eq!(request.metadata.payload_frame_count, 1);
        let prelude = BodyRegionPrelude::parse(&request.body).unwrap();
        assert_eq!(prelude.typed_payload_descriptor_bytes, 24);
        assert_eq!(prelude.typed_payload_frame_bytes, 3);
    }

    #[test]
    fn typed_payload_builder_derives_offsets_lengths_bitmap_and_header_context() {
        let request = NnrpSubmitRequest::typed_payload(NnrpTypedPayloadSubmitInput {
            identity: identity(),
            policy: NnrpSubmitPolicy {
                frame_class: 2,
                latency_budget_ms: 17,
                target_fps_x100: 6_000,
                retry_of_frame: 8,
                budget_policy: BudgetPolicy::ALLOW_DEGRADED,
                loss_tolerance_policy: LossTolerancePolicy::LowLatency,
                dependency_frame_id: 6,
            },
            frames: vec![
                NnrpTypedPayloadInputFrame {
                    profile_id: 41,
                    payload_kind: PayloadKind::AudioChunk,
                    descriptor_flags: 1,
                    schema_id: 42,
                    schema_version: 3,
                    stream_semantics: 4,
                    payload: vec![1, 2, 3],
                },
                NnrpTypedPayloadInputFrame {
                    profile_id: 51,
                    payload_kind: PayloadKind::StructuredEvent,
                    descriptor_flags: 2,
                    schema_id: 52,
                    schema_version: 5,
                    stream_semantics: 6,
                    payload: vec![4, 5],
                },
            ],
        })
        .unwrap();

        assert_eq!(request.operation_id, identity().operation_id);
        assert_eq!(request.frame_id, identity().frame_id);
        assert_eq!(request.header, identity().header);
        assert_eq!(request.metadata.frame_class, 2);
        assert_eq!(request.metadata.latency_budget_ms, 17);
        assert_eq!(request.metadata.target_fps_x100, 6_000);
        assert_eq!(request.metadata.retry_of_frame, 8);
        assert_eq!(request.metadata.dependency_frame_id, 6);
        assert_eq!(request.metadata.payload_frame_count, 2);
        assert_eq!(
            request.metadata.payload_kind_bitmap.0,
            PayloadKind::AudioChunk.bit() | PayloadKind::StructuredEvent.bit()
        );

        let prelude = BodyRegionPrelude::parse(&request.body).unwrap();
        assert_eq!(prelude.inline_object_bytes, 0);
        assert_eq!(prelude.object_reference_bytes, 0);
        assert_eq!(prelude.typed_payload_descriptor_bytes, 48);
        assert_eq!(prelude.typed_payload_frame_bytes, 5);
        let descriptor_offset = BODY_REGION_PRELUDE_LEN;
        let first =
            TypedPayloadDescriptor::parse(&request.body[descriptor_offset..descriptor_offset + 24])
                .unwrap();
        let second = TypedPayloadDescriptor::parse(
            &request.body[descriptor_offset + 24..descriptor_offset + 48],
        )
        .unwrap();
        assert_eq!((first.offset, first.length), (0, 3));
        assert_eq!((second.offset, second.length), (3, 2));
        assert_eq!(&request.body[descriptor_offset + 48..], &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn tensor_builder_derives_section_descriptor_and_header_context() {
        let request = NnrpSubmitRequest::tensor(NnrpTensorSubmitInput {
            identity: identity(),
            policy: NnrpSubmitPolicy::default(),
            src_width: 64,
            src_height: 64,
            tile_width: 16,
            tile_height: 16,
            tile_ids: vec![0, 1],
            sections: vec![NnrpTensorSection {
                role_id: 1,
                default_codec_id: 2,
                dtype_id: 3,
                layout_id: 4,
                scale_policy: 0,
                element_count_per_tile: 16,
                tile_payloads: vec![vec![1, 2], vec![3]],
                codec_ids: Vec::new(),
                payload_stride_bytes: 0,
            }],
            camera_block: Vec::new(),
            input_profile: InputProfile::ChangedTilesLuma,
            tile_index_mode: TileIndexMode::DenseRange,
            tile_base_id: 0,
            references: NnrpSubmitObjectReferences::default(),
        })
        .unwrap();
        assert_eq!(request.header, identity().header);
        assert_eq!(request.metadata.tile_count, 2);
        assert_eq!(request.metadata.section_count, 1);
        assert_eq!(
            request.metadata.payload_kind_bitmap.0,
            PayloadKindBitmap::TENSOR
        );
        assert!(request.body.len() > BODY_REGION_PRELUDE_LEN);
    }

    #[test]
    fn defaults_and_encoded_payload_preserve_the_frozen_request_shape() {
        assert_eq!(NnrpSubmitHeaderContext::default().flags, HeaderFlags::NONE);
        assert_eq!(NnrpSubmitHeaderContext::default().trace_id, 0);
        assert_eq!(
            NnrpSubmitPolicy::default().budget_policy,
            BudgetPolicy::NONE
        );
        assert_eq!(
            NnrpSubmitPolicy::default().loss_tolerance_policy,
            LossTolerancePolicy::InheritSession
        );

        let request = NnrpSubmitRequest::token(NnrpTokenSubmitInput {
            identity: identity(),
            policy: NnrpSubmitPolicy::default(),
            chunks: vec![NnrpTokenChunk::partial(b"encoded".to_vec())],
        })
        .unwrap();
        let encoded = request.encoded_payload().unwrap();
        assert_eq!(
            &encoded[..72],
            request.metadata.to_bytes().unwrap().as_slice()
        );
        assert_eq!(&encoded[72..], request.body.as_slice());
    }

    #[test]
    fn request_builders_reject_invalid_identity_flags_and_typed_payload_shapes() {
        let mut invalid_identity = identity();
        invalid_identity.operation_id = 0;
        assert!(NnrpSubmitRequest::token(NnrpTokenSubmitInput {
            identity: invalid_identity,
            policy: NnrpSubmitPolicy::default(),
            chunks: vec![NnrpTokenChunk::partial(Vec::new())],
        })
        .is_err());

        let mut invalid_flags = identity();
        invalid_flags.header.flags = HeaderFlags(0x8000_0000);
        assert!(
            NnrpSubmitRequest::typed_payload(NnrpTypedPayloadSubmitInput {
                identity: invalid_flags,
                policy: NnrpSubmitPolicy::default(),
                frames: vec![NnrpTypedPayloadInputFrame {
                    profile_id: 41,
                    payload_kind: PayloadKind::AudioChunk,
                    descriptor_flags: 0,
                    schema_id: 42,
                    schema_version: 1,
                    stream_semantics: 0,
                    payload: Vec::new(),
                }],
            })
            .is_err()
        );

        assert!(
            NnrpSubmitRequest::typed_payload(NnrpTypedPayloadSubmitInput {
                identity: identity(),
                policy: NnrpSubmitPolicy::default(),
                frames: Vec::new(),
            })
            .is_err()
        );
        assert!(
            NnrpSubmitRequest::typed_payload(NnrpTypedPayloadSubmitInput {
                identity: identity(),
                policy: NnrpSubmitPolicy::default(),
                frames: vec![NnrpTypedPayloadInputFrame {
                    profile_id: 1,
                    payload_kind: PayloadKind::Tensor,
                    descriptor_flags: 0,
                    schema_id: 1,
                    schema_version: 1,
                    stream_semantics: 0,
                    payload: vec![1],
                }],
            })
            .is_err()
        );
    }

    #[test]
    fn tile_index_encodings_cover_every_frozen_mode_and_rejection() {
        assert_eq!(
            encode_tile_indices(&[4, 5, 6], TileIndexMode::DenseRange, 4).unwrap(),
            Vec::<u8>::new()
        );
        assert!(encode_tile_indices(&[4, 6], TileIndexMode::DenseRange, 4).is_err());
        assert_eq!(
            encode_tile_indices(&[1, 0x0203], TileIndexMode::RawU16, 0).unwrap(),
            vec![1, 0, 3, 2]
        );
        assert_eq!(
            encode_tile_indices(&[3, 8, 10], TileIndexMode::DeltaU16, 0).unwrap(),
            vec![3, 0, 5, 0, 2, 0]
        );
        assert!(encode_tile_indices(&[3, 3], TileIndexMode::DeltaU16, 0).is_err());
        assert_eq!(
            encode_tile_indices(&[0, 3, 9], TileIndexMode::Bitset, 0).unwrap(),
            vec![0b0000_1001, 0b0000_0010]
        );
        assert!(encode_tile_indices(&[3, 3], TileIndexMode::Bitset, 0).is_err());
    }

    #[test]
    fn tensor_section_encoding_enforces_order_counts_codecs_and_stride() {
        let mut mixed = tensor_section(1);
        mixed.codec_ids = vec![2, 7];
        mixed.payload_stride_bytes = 4;
        let encoded = encode_tensor_sections(&[mixed.clone()], 2).unwrap();
        let descriptor = TensorSectionDescriptor::parse(&encoded[0]).unwrap();
        assert_eq!(
            descriptor.section_flags,
            TensorSectionDescriptor::MIXED_CODEC | TensorSectionDescriptor::FIXED_STRIDE
        );

        assert!(encode_tensor_sections(&[tensor_section(2), tensor_section(1)], 2).is_err());
        let mut wrong_payload_count = tensor_section(1);
        wrong_payload_count.tile_payloads.pop();
        assert!(encode_tensor_sections(&[wrong_payload_count], 2).is_err());
        let mut wrong_codec_count = tensor_section(1);
        wrong_codec_count.codec_ids = vec![2];
        assert!(encode_tensor_sections(&[wrong_codec_count], 2).is_err());
        let mut oversized = tensor_section(1);
        oversized.payload_stride_bytes = 1;
        assert!(encode_tensor_sections(&[oversized], 2).is_err());
    }

    #[test]
    fn reference_helpers_validate_standard_slots_and_derive_all_mask_bits() {
        let camera = reference(CacheObjectKind::CameraBlock, 1);
        let tile_index = reference(CacheObjectKind::TileIndexBlock, 2);
        let section_table = reference(CacheObjectKind::TensorSectionTable, 3);
        let layout = reference(CacheObjectKind::PayloadLayoutTemplate, 4);
        assert_eq!(
            reference_mask(&[camera, tile_index, section_table, layout]),
            0x0f
        );
        assert_eq!(
            standard_references(&NnrpSubmitObjectReferences {
                camera: Some(camera),
                tile_index: Some(tile_index),
                tensor_section_table: Some(section_table),
            })
            .unwrap(),
            vec![camera, tile_index, section_table]
        );
        assert!(standard_references(&NnrpSubmitObjectReferences {
            camera: Some(tile_index),
            tile_index: None,
            tensor_section_table: None,
        })
        .is_err());
    }

    #[test]
    fn tensor_builder_emits_mixed_and_reference_only_submits() {
        let mixed = NnrpSubmitRequest::tensor(NnrpTensorSubmitInput {
            identity: identity(),
            policy: NnrpSubmitPolicy::default(),
            src_width: 32,
            src_height: 32,
            tile_width: 16,
            tile_height: 16,
            tile_ids: vec![2, 4],
            sections: vec![tensor_section(1)],
            camera_block: Vec::new(),
            input_profile: InputProfile::ChangedTilesLuma,
            tile_index_mode: TileIndexMode::RawU16,
            tile_base_id: 0,
            references: NnrpSubmitObjectReferences {
                camera: Some(reference(CacheObjectKind::CameraBlock, 1)),
                tile_index: None,
                tensor_section_table: None,
            },
        })
        .unwrap();
        assert_eq!(mixed.metadata.submit_mode, SubmitMode::Mixed);
        assert_eq!(mixed.metadata.camera_bytes, 0);
        assert_eq!(mixed.metadata.tile_index_bytes, 4);

        let reference_only = NnrpSubmitRequest::tensor(NnrpTensorSubmitInput {
            identity: identity(),
            policy: NnrpSubmitPolicy::default(),
            src_width: 0,
            src_height: 0,
            tile_width: 0,
            tile_height: 0,
            tile_ids: Vec::new(),
            sections: Vec::new(),
            camera_block: Vec::new(),
            input_profile: InputProfile::Unspecified,
            tile_index_mode: TileIndexMode::DenseRange,
            tile_base_id: 0,
            references: NnrpSubmitObjectReferences {
                camera: Some(reference(CacheObjectKind::CameraBlock, 1)),
                tile_index: Some(reference(CacheObjectKind::TileIndexBlock, 2)),
                tensor_section_table: Some(reference(CacheObjectKind::TensorSectionTable, 3)),
            },
        })
        .unwrap();
        assert_eq!(reference_only.metadata.submit_mode, SubmitMode::Reference);
        assert_eq!(reference_only.metadata.object_ref_mask, 0x07);
    }
}
