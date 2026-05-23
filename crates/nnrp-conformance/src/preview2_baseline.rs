use nnrp_core::{
    BackpressureLevel, BodyRegionPrelude, CacheAckMetadata, CacheInvalidateMetadata,
    CacheObjectKind, CachePutMetadata, CommonHeader, FlowScopeKind, FlowUpdateMetadata,
    FlowUpdateReason, FrameSubmitMetadata, MessageType, ObjectReferenceBlock,
    ObjectReferenceRegion, PayloadKindBitmap, ResultHintMetadata, ResultPushMetadata,
    SessionPatchAckMetadata, SubmitMode, TypedPayloadDescriptor, TypedPayloadRegion,
    CACHE_ACK_METADATA_LEN, CACHE_INVALIDATE_METADATA_LEN, CACHE_PUT_METADATA_LEN,
    CLIENT_HELLO_METADATA_LEN, FLOW_UPDATE_FLAG_CREDIT_VALID, FLOW_UPDATE_FLAG_RETRY_AFTER_VALID,
    FRAME_SUBMIT_METADATA_LEN, OBJECT_REFERENCE_BLOCK_LEN, PAYLOAD_KIND_KNOWN_MASK,
    RESULT_PUSH_METADATA_LEN, SESSION_PATCH_ACK_METADATA_LEN, STANDARD_PROFILE_TOKEN,
};
use nnrp_core::{ClientHelloMetadata, ResultHintReason};

pub fn execute_preview2_case(case_id: &str) -> Option<Result<(), String>> {
    let result = match case_id {
        "l0.header.fixed_shape.golden" => l0_header_fixed_shape(),
        "l0.control.client_hello.golden" => l0_client_hello(),
        "l0.control.session_patch_ack.golden" => l0_session_patch_ack(),
        "l0.flow_update.packet.golden" => l0_flow_update_packet(),
        "l0.result_hint.packet.golden" => l0_result_hint_packet(),
        "l0.frame_submit.metadata.golden" => l0_frame_submit_metadata(),
        "l0.result_push.metadata.golden" => l0_result_push_metadata(),
        "l0.body_region.prelude.golden" => l0_body_region_prelude(),
        "l0.object_reference.block.golden" => l0_object_reference_block(),
        "l0.typed_payload.descriptor.golden" => l0_preview2_typed_payload_descriptor(),
        "l0.typed_payload.frame_regions.golden" => l0_preview2_typed_payload_frame_regions(),
        "l1.flow_update.metadata.validation" => l1_flow_update_validation(),
        "l1.result_hint.metadata.validation" => l1_result_hint_validation(),
        "l1.cache.lifecycle.roundtrip" => l1_cache_lifecycle_roundtrip(),
        "l1.frame_submit.message.parse_emit" => l1_frame_submit_parse_emit(),
        "l1.result_push.message.parse_emit" => l1_result_push_parse_emit(),
        _ => return None,
    };
    Some(result)
}

fn l0_header_fixed_shape() -> Result<(), String> {
    round_trip_header(&hex_to_bytes(
        "4e4e525001001028210000003000000000100000070000000b0000000200000015cd5b0700000000",
    ))
}

fn l0_client_hello() -> Result<(), String> {
    round_trip_metadata(
        CLIENT_HELLO_METADATA_LEN,
        &hex_to_bytes("01010100010000000100000003000000030000002100000003000000010007000100020040000000000001007017640002000000000000006000000000000000"),
        ClientHelloMetadata::parse,
        |metadata, destination| metadata.write(destination),
    )
}

fn l0_session_patch_ack() -> Result<(), String> {
    round_trip_metadata(
        SESSION_PATCH_ACK_METADATA_LEN,
        &hex_to_bytes("010003001100000044000000000000000200000028230000680105000300000000000000010000000300000010000000"),
        SessionPatchAckMetadata::parse,
        |metadata, destination| metadata.write(destination),
    )
}

fn l0_flow_update_packet() -> Result<(), String> {
    let packet = hex_to_bytes("4e4e5250010017280000000020000000000000001500000000000000000006000d000000000000000104020000000100000000000000000000000000280000000500000003000000");
    let (header, metadata, body) = CommonHeader::parse_packet(&packet).map_err(to_string)?;
    if !body.is_empty() {
        return Err("FLOW_UPDATE golden packet must not carry a body".to_string());
    }
    let flow = FlowUpdateMetadata::parse(metadata).map_err(to_string)?;
    flow.validate_routing(&header).map_err(to_string)?;
    assert_packet_round_trip(&header, metadata, body, &packet)
}

fn l0_result_hint_packet() -> Result<(), String> {
    let packet = hex_to_bytes("4e4e525001001828000000001000000000000000150000002f010000000007000e000000000000000300000003000000030000003c000000");
    let (header, metadata, body) = CommonHeader::parse_packet(&packet).map_err(to_string)?;
    if header.message_type != MessageType::ResultHint || !body.is_empty() {
        return Err("RESULT_HINT golden packet has an invalid envelope".to_string());
    }
    let hint = ResultHintMetadata::parse(metadata).map_err(to_string)?;
    if hint.reason != ResultHintReason::BudgetExceeded {
        return Err("RESULT_HINT golden packet did not preserve budget_exceeded".to_string());
    }
    assert_packet_round_trip(&header, metadata, body, &packet)
}

fn l0_frame_submit_metadata() -> Result<(), String> {
    round_trip_metadata(
        FRAME_SUBMIT_METADATA_LEN,
        &hex_to_bytes("80026801200020005400020001020000640070170700000000000000c000000000000000000000000000000000000000000000000205ff0003000000290000001100000002000000"),
        FrameSubmitMetadata::parse,
        |metadata, destination| metadata.write(destination),
    )
}

fn l0_result_push_metadata() -> Result<(), String> {
    round_trip_metadata(
        RESULT_PUSH_METADATA_LEN,
        &hex_to_bytes("0000040001005400020000004b0302004e030000000000001000000000000000000000000000000000000000010100002900000035001f000300000003000000"),
        ResultPushMetadata::parse,
        |metadata, destination| metadata.write(destination),
    )
}

fn l0_body_region_prelude() -> Result<(), String> {
    round_trip_metadata(
        32,
        &hex_to_bytes("1800000010000000100000000e00000010000000050000000000000000000000"),
        BodyRegionPrelude::parse,
        |metadata, destination| metadata.write(destination),
    )
}

fn l0_object_reference_block() -> Result<(), String> {
    round_trip_metadata(
        OBJECT_REFERENCE_BLOCK_LEN,
        &hex_to_bytes("02000000070000004433221188776655"),
        ObjectReferenceBlock::parse,
        |metadata, destination| metadata.write(destination),
    )
}

fn l0_preview2_typed_payload_descriptor() -> Result<(), String> {
    let bytes = hex_to_bytes("10000300040000000700000000000000");
    let descriptor = Preview2TypedPayloadDescriptor::parse(&bytes)?;
    if descriptor.payload_kind != PayloadKindBitmap::STRUCTURED_EVENT
        || descriptor.profile_id != 3
        || descriptor.payload_offset != 4
        || descriptor.payload_length != 7
    {
        return Err("preview2 typed-payload descriptor fields changed".to_string());
    }
    if descriptor.to_bytes() != bytes {
        return Err("preview2 typed-payload descriptor did not round-trip".to_string());
    }
    Ok(())
}

fn l0_preview2_typed_payload_frame_regions() -> Result<(), String> {
    let descriptors = hex_to_bytes(
        "020001000000000003000000000000000400020003000000020000000000000008000300050000000500000000000000100004000a0000000300000000000000",
    );
    let payload = b"tokauvideoevt";
    validate_preview2_typed_payload_region(
        PayloadKindBitmap::TOKEN_CHUNK
            | PayloadKindBitmap::AUDIO_CHUNK
            | PayloadKindBitmap::VIDEO_CHUNK
            | PayloadKindBitmap::STRUCTURED_EVENT,
        4,
        &descriptors,
        payload,
    )
}

fn l1_flow_update_validation() -> Result<(), String> {
    let mut header = CommonHeader::new(MessageType::FlowUpdate, 32, 0);
    header.session_id = 21;
    let metadata = FlowUpdateMetadata {
        scope_kind: FlowScopeKind::Session,
        update_reason: FlowUpdateReason::Congestion,
        backpressure_level: BackpressureLevel::Hard,
        connection_credit: 0,
        session_credit: 1,
        operation_credit: 0,
        operation_id: 0,
        retry_after_ms: 40,
        credit_epoch: 5,
        flow_flags: FLOW_UPDATE_FLAG_CREDIT_VALID | FLOW_UPDATE_FLAG_RETRY_AFTER_VALID,
    };
    metadata.validate_routing(&header).map_err(to_string)?;

    let invalid = FlowUpdateMetadata {
        retry_after_ms: 1,
        flow_flags: FLOW_UPDATE_FLAG_CREDIT_VALID,
        ..metadata
    };
    if invalid.validate_routing(&header).is_ok() {
        return Err("FLOW_UPDATE accepted retry_after without retry_after_valid".to_string());
    }
    Ok(())
}

fn l1_result_hint_validation() -> Result<(), String> {
    let bytes = hex_to_bytes("02000000020000000200000014000000");
    let hint = ResultHintMetadata::parse(&bytes).map_err(to_string)?;
    if hint.to_bytes().map_err(to_string)?.as_slice() != bytes.as_slice() {
        return Err("RESULT_HINT metadata did not round-trip".to_string());
    }

    let mut bad = bytes;
    bad[8..12].copy_from_slice(&99u32.to_le_bytes());
    if ResultHintMetadata::parse(&bad).is_ok() {
        return Err("RESULT_HINT accepted an unknown reason code".to_string());
    }
    Ok(())
}

fn l1_cache_lifecycle_roundtrip() -> Result<(), String> {
    round_trip_metadata(
        CACHE_PUT_METADATA_LEN,
        &hex_to_bytes("01000000040302010807060501000000983a0000000800000300000003000000"),
        CachePutMetadata::parse,
        |metadata, destination| metadata.write(destination),
    )?;
    round_trip_metadata(
        CACHE_ACK_METADATA_LEN,
        &hex_to_bytes("01000000040302010807060500000000983a00000020000000000000"),
        CacheAckMetadata::parse,
        |metadata, destination| metadata.write(destination),
    )?;
    round_trip_metadata(
        CACHE_INVALIDATE_METADATA_LEN,
        &hex_to_bytes("0000000001000000040302010807060502000000"),
        CacheInvalidateMetadata::parse,
        |metadata, destination| metadata.write(destination),
    )
}

fn l1_frame_submit_parse_emit() -> Result<(), String> {
    let metadata = FrameSubmitMetadata::parse(&hex_to_bytes("80026801200020005400020001020000640070170700000000000000c000000000000000000000000000000000000000000000000205ff0003000000290000001100000002000000")).map_err(to_string)?;
    validate_submit_object_reference(metadata.submit_mode, metadata.object_ref_mask)?;

    let typed_descriptor = TypedPayloadDescriptor {
        profile_id: STANDARD_PROFILE_TOKEN,
        descriptor_flags: 0x0002,
        schema_id: 0x0000_1001,
        schema_version: 3,
        stream_semantics: 2,
        offset: 0,
        length: 3,
    };
    let region = TypedPayloadRegion::from_parts(
        PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
        vec![typed_descriptor],
        b"tok",
    )
    .map_err(to_string)?;
    if region.frame_views().map_err(to_string)?[0].payload != b"tok" {
        return Err("typed payload frame projection changed".to_string());
    }
    Ok(())
}

fn l1_result_push_parse_emit() -> Result<(), String> {
    let metadata = ResultPushMetadata::parse(&hex_to_bytes("0000040001005400020000004b0302004e030000000000001000000000000000000000000000000000000000010100002900000035001f000300000003000000")).map_err(to_string)?;
    if metadata.payload_frame_count != 3 || !metadata.payload_kind_bitmap.contains_tensor() {
        return Err("RESULT_PUSH preview2 metadata lost tensor payload bookkeeping".to_string());
    }
    let cache_ref = ObjectReferenceBlock {
        object_kind: CacheObjectKind::TileIndexBlock,
        ref_flags: 0,
        cache_namespace: 7,
        cache_key_hi: 0x1122_3344,
        cache_key_lo: 0x5566_7788,
    };
    ObjectReferenceRegion::from_blocks(vec![cache_ref])
        .validate_resolved_or_cache_miss(|block| block.cache_namespace == 7, 21, 303, 0)
        .map_err(|error| format!("unexpected cache miss error: {error:?}"))
}

fn validate_submit_object_reference(
    submit_mode: SubmitMode,
    object_ref_mask: u32,
) -> Result<(), String> {
    let camera = ObjectReferenceBlock {
        object_kind: CacheObjectKind::CameraBlock,
        ref_flags: 0,
        cache_namespace: 1,
        cache_key_hi: 1,
        cache_key_lo: 1,
    };
    let tile = ObjectReferenceBlock {
        object_kind: CacheObjectKind::TileIndexBlock,
        ref_flags: 0,
        cache_namespace: 1,
        cache_key_hi: 2,
        cache_key_lo: 2,
    };
    ObjectReferenceRegion::from_blocks(vec![camera, tile])
        .validate_submit_mask(submit_mode, object_ref_mask)
        .map_err(to_string)
}

fn round_trip_header(bytes: &[u8]) -> Result<(), String> {
    let header = CommonHeader::parse(bytes).map_err(to_string)?;
    if header.to_bytes().map_err(to_string)?.as_slice() != bytes {
        return Err("common header did not round-trip".to_string());
    }
    Ok(())
}

fn round_trip_metadata<T>(
    expected_len: usize,
    bytes: &[u8],
    parse: fn(&[u8]) -> Result<T, nnrp_core::NnrpError>,
    write: fn(&T, &mut [u8]) -> Result<(), nnrp_core::NnrpError>,
) -> Result<(), String> {
    if bytes.len() != expected_len {
        return Err(format!(
            "golden vector length changed: expected {expected_len}, got {}",
            bytes.len()
        ));
    }
    let metadata = parse(bytes).map_err(to_string)?;
    let mut output = vec![0u8; expected_len];
    write(&metadata, &mut output).map_err(to_string)?;
    if output.as_slice() != bytes {
        return Err("metadata did not round-trip".to_string());
    }
    Ok(())
}

fn assert_packet_round_trip(
    header: &CommonHeader,
    metadata: &[u8],
    body: &[u8],
    expected: &[u8],
) -> Result<(), String> {
    let mut output = Vec::with_capacity(expected.len());
    output.extend_from_slice(&header.to_bytes().map_err(to_string)?);
    output.extend_from_slice(metadata);
    output.extend_from_slice(body);
    if output != expected {
        return Err("packet did not round-trip".to_string());
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Preview2TypedPayloadDescriptor {
    payload_kind: u32,
    descriptor_flags: u8,
    profile_id: u16,
    payload_offset: u32,
    payload_length: u32,
}

impl Preview2TypedPayloadDescriptor {
    fn parse(source: &[u8]) -> Result<Self, String> {
        if source.len() != 16 {
            return Err(format!(
                "preview2 typed-payload descriptor must be 16 bytes, got {}",
                source.len()
            ));
        }
        let descriptor_flags = source[1];
        let reserved = read_u32(source, 12);
        if descriptor_flags != 0 || reserved != 0 {
            return Err(
                "preview2 typed-payload descriptor reserved fields must be zero".to_string(),
            );
        }
        let payload_kind = u32::from(source[0]);
        if payload_kind == 0
            || payload_kind & (payload_kind - 1) != 0
            || payload_kind & !PAYLOAD_KIND_KNOWN_MASK != 0
        {
            return Err("preview2 typed-payload descriptor payload_kind is invalid".to_string());
        }
        Ok(Self {
            payload_kind,
            descriptor_flags,
            profile_id: read_u16(source, 2),
            payload_offset: read_u32(source, 4),
            payload_length: read_u32(source, 8),
        })
    }

    fn to_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16);
        bytes.push(self.payload_kind as u8);
        bytes.push(self.descriptor_flags);
        bytes.extend_from_slice(&self.profile_id.to_le_bytes());
        bytes.extend_from_slice(&self.payload_offset.to_le_bytes());
        bytes.extend_from_slice(&self.payload_length.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes
    }
}

fn validate_preview2_typed_payload_region(
    payload_kind_bitmap: u32,
    payload_frame_count: u16,
    descriptor_region: &[u8],
    payload_region: &[u8],
) -> Result<(), String> {
    if descriptor_region.len() != usize::from(payload_frame_count) * 16 {
        return Err("preview2 descriptor bytes must match payload_frame_count * 16".to_string());
    }

    let mut next_offset = 0u32;
    for chunk in descriptor_region.chunks_exact(16) {
        let descriptor = Preview2TypedPayloadDescriptor::parse(chunk)?;
        if descriptor.payload_kind & !payload_kind_bitmap != 0 {
            return Err(
                "preview2 descriptor payload kind was not declared in metadata".to_string(),
            );
        }
        if descriptor.payload_offset != next_offset {
            return Err("preview2 typed-payload descriptors must be contiguous".to_string());
        }
        next_offset = descriptor
            .payload_offset
            .checked_add(descriptor.payload_length)
            .ok_or_else(|| "preview2 typed-payload range overflowed".to_string())?;
        if next_offset as usize > payload_region.len() {
            return Err("preview2 typed-payload range exceeds payload region".to_string());
        }
    }

    if next_offset as usize != payload_region.len() {
        return Err("preview2 payload region must be exactly covered".to_string());
    }
    Ok(())
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("valid hex"))
        .collect()
}

fn read_u16(source: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(source[offset..offset + 2].try_into().expect("slice length"))
}

fn read_u32(source: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(source[offset..offset + 4].try_into().expect("slice length"))
}

fn to_string(error: nnrp_core::NnrpError) -> String {
    error.to_string()
}
