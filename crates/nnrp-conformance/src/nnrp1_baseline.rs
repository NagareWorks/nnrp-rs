use nnrp_core::{
    BackpressureLevel, BodyRegionPrelude, CacheAckMetadata, CacheInvalidateMetadata,
    CacheObjectKind, CachePutMetadata, CommonHeader, FlowScopeKind, FlowUpdateMetadata,
    FlowUpdateReason, FrameSubmitMetadata, InputProfile, MessageType, ObjectReferenceBlock,
    ObjectReferenceRegion, PayloadKindBitmap, ResultClass, ResultHintMetadata, ResultPushMetadata,
    SessionPatchAckMetadata, SubmitMode, TileIndexMode, TransportId, TransportProbeAckMetadata,
    TransportProbeMetadata, TypedPayloadDescriptor, TypedPayloadRegion, CACHE_ACK_METADATA_LEN,
    CACHE_INVALIDATE_METADATA_LEN, CACHE_PUT_METADATA_LEN, CLIENT_HELLO_METADATA_LEN,
    FLOW_UPDATE_FLAG_CREDIT_VALID, FLOW_UPDATE_FLAG_RETRY_AFTER_VALID, FRAME_SUBMIT_METADATA_LEN,
    OBJECT_REFERENCE_BLOCK_LEN, PAYLOAD_KIND_KNOWN_MASK, RESULT_PUSH_METADATA_LEN,
    SESSION_PATCH_ACK_METADATA_LEN, STANDARD_PROFILE_TOKEN,
};
use nnrp_core::{ClientHelloMetadata, ResultHintReason};
use nnrp_runtime::{NnrpClient, NnrpClientConfig, NnrpServerConfig, RuntimeError};
use nnrp_transport_provider::{
    select_transport_with_probe, ProbeSample, RemoteTransportSupport, TransportPolicy,
    TransportProviderDescriptor, TransportProviderKind,
};
use nnrp_transport_quic::{
    quic_client_config, quic_server_config, QuicClientEndpointConfig, QuicProvider,
    QuicServerEndpointConfig,
};
use nnrp_transport_tcp::TcpProvider;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Preview3FrameSubmitMetadata {
    bytes: [u8; FRAME_SUBMIT_METADATA_LEN],
    submit_mode: SubmitMode,
    object_ref_mask: u32,
}

impl Preview3FrameSubmitMetadata {
    fn parse(source: &[u8]) -> Result<Self, nnrp_core::NnrpError> {
        if source.len() < FRAME_SUBMIT_METADATA_LEN {
            return Err(nnrp_core::NnrpError::SourceTooShort {
                expected: FRAME_SUBMIT_METADATA_LEN,
                actual: source.len(),
            });
        }
        let mut bytes = [0u8; FRAME_SUBMIT_METADATA_LEN];
        bytes.copy_from_slice(&source[..FRAME_SUBMIT_METADATA_LEN]);
        Ok(Self {
            submit_mode: SubmitMode::try_from_u8(bytes[52])?,
            object_ref_mask: u32::from_le_bytes(bytes[56..60].try_into().expect("fixed range")),
            bytes,
        })
    }

    fn write(&self, destination: &mut [u8]) -> Result<(), nnrp_core::NnrpError> {
        if destination.len() < FRAME_SUBMIT_METADATA_LEN {
            return Err(nnrp_core::NnrpError::DestinationTooShort {
                expected: FRAME_SUBMIT_METADATA_LEN,
                actual: destination.len(),
            });
        }
        destination[..FRAME_SUBMIT_METADATA_LEN].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn execute_nnrp1_baseline_case(case_id: &str) -> Option<Result<(), String>> {
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
        "l0.typed_payload.descriptor.golden" => l0_baseline_typed_payload_descriptor(),
        "l0.typed_payload.frame_regions.golden" => l0_baseline_typed_payload_frame_regions(),
        "l1.flow_update.metadata.validation" => l1_flow_update_validation(),
        "l1.result_hint.metadata.validation" => l1_result_hint_validation(),
        "l1.cache.lifecycle.roundtrip" => l1_cache_lifecycle_roundtrip(),
        "l1.transport_probe.metadata.roundtrip" => l1_transport_probe_roundtrip(),
        "l1.frame_submit.message.parse_emit" => l1_frame_submit_parse_emit(),
        "l1.result_push.message.parse_emit" => l1_result_push_parse_emit(),
        "l1.result_push.object_reference.resolve" => l1_result_push_object_reference_resolve(),
        "l1.typed_payload.region.pack" => l1_typed_payload_region_pack(),
        "l3.transport.probe.selection" => l3_transport_probe_selection(),
        "l3.transport.tcp.session_smoke" => l3_transport_tcp_session_smoke(),
        "l3.transport.quic.session_smoke" => l3_transport_quic_session_smoke(),
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
        Preview3FrameSubmitMetadata::parse,
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
        &hex_to_bytes("1800000018000000100000000e00000010000000050000000000000000000000"),
        BodyRegionPrelude::parse,
        |metadata, destination| metadata.write(destination),
    )
}

fn l0_object_reference_block() -> Result<(), String> {
    round_trip_metadata(
        OBJECT_REFERENCE_BLOCK_LEN,
        &hex_to_bytes("020000000700000044332211000000008877665500000000"),
        ObjectReferenceBlock::parse,
        |metadata, destination| metadata.write(destination),
    )
}

fn l0_baseline_typed_payload_descriptor() -> Result<(), String> {
    let bytes = hex_to_bytes("10000300040000000700000000000000");
    let descriptor = BaselineTypedPayloadDescriptor::parse(&bytes)?;
    if descriptor.payload_kind != PayloadKindBitmap::STRUCTURED_EVENT
        || descriptor.profile_id != 3
        || descriptor.payload_offset != 4
        || descriptor.payload_length != 7
    {
        return Err("NNRP/1 baseline typed-payload descriptor fields changed".to_string());
    }
    if descriptor.to_bytes() != bytes {
        return Err("NNRP/1 baseline typed-payload descriptor did not round-trip".to_string());
    }
    Ok(())
}

fn l0_baseline_typed_payload_frame_regions() -> Result<(), String> {
    let descriptors = hex_to_bytes(
        "020001000000000003000000000000000400020003000000020000000000000008000300050000000500000000000000100004000a0000000300000000000000",
    );
    let payload = b"tokauvideoevt";
    validate_baseline_typed_payload_region(
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
        &hex_to_bytes(
            "010000000100000004030201000000000807060500000000983a0000000800000300000003000000",
        ),
        CachePutMetadata::parse,
        |metadata, destination| metadata.write(destination),
    )?;
    round_trip_metadata(
        CACHE_ACK_METADATA_LEN,
        &hex_to_bytes(
            "010000000000000004030201000000000807060500000000983a0000002000000000000000000000",
        ),
        CacheAckMetadata::parse,
        |metadata, destination| metadata.write(destination),
    )?;
    round_trip_metadata(
        CACHE_INVALIDATE_METADATA_LEN,
        &hex_to_bytes("0300000001000000040302010000000008070605000000000200000000000000"),
        CacheInvalidateMetadata::parse,
        |metadata, destination| metadata.write(destination),
    )
}

fn l1_transport_probe_roundtrip() -> Result<(), String> {
    let probe = TransportProbeMetadata {
        probe_id: 7,
        probe_payload_bytes: 1_200,
        client_send_ts_us: 100_000,
    };
    let parsed =
        TransportProbeMetadata::parse(&probe.to_bytes().map_err(to_string)?).map_err(to_string)?;
    if parsed != probe {
        return Err("TRANSPORT_PROBE metadata did not round-trip".to_string());
    }

    let ack = TransportProbeAckMetadata {
        probe_id: 7,
        server_recv_ts_us: 100_800,
    };
    let parsed =
        TransportProbeAckMetadata::parse(&ack.to_bytes().map_err(to_string)?).map_err(to_string)?;
    if parsed != ack {
        return Err("TRANSPORT_PROBE_ACK metadata did not round-trip".to_string());
    }
    Ok(())
}

fn l1_frame_submit_parse_emit() -> Result<(), String> {
    let metadata = Preview3FrameSubmitMetadata::parse(&hex_to_bytes("80026801200020005400020001020000640070170700000000000000c000000000000000000000000000000000000000000000000205ff0003000000290000001100000002000000")).map_err(to_string)?;
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
        return Err("RESULT_PUSH baseline metadata lost tensor payload bookkeeping".to_string());
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

fn l1_result_push_object_reference_resolve() -> Result<(), String> {
    let cache_ref = ObjectReferenceBlock {
        object_kind: CacheObjectKind::TileIndexBlock,
        ref_flags: 0,
        cache_namespace: 7,
        cache_key_hi: 0x1122_3344,
        cache_key_lo: 0x5566_7788,
    };
    let region = ObjectReferenceRegion::from_blocks(vec![cache_ref]);
    region
        .validate_resolved_or_cache_miss(|block| block.cache_namespace == 7, 21, 303, 0)
        .map_err(|error| format!("resolved cache-backed reference was rejected: {error:?}"))?;

    let miss = region
        .validate_resolved_or_cache_miss(|_| false, 21, 303, 0)
        .expect_err("unresolved cache-backed reference should return cache miss metadata");
    if miss.error_code != nnrp_core::CACHE_ERROR_MISS {
        return Err("object-reference cache miss returned the wrong error code".to_string());
    }
    Ok(())
}

fn l1_typed_payload_region_pack() -> Result<(), String> {
    validate_baseline_typed_payload_region(
        PayloadKindBitmap::TOKEN_CHUNK | PayloadKindBitmap::STRUCTURED_EVENT,
        2,
        &[
            descriptor(PayloadKindBitmap::TOKEN_CHUNK as u8, 0, 3),
            descriptor(PayloadKindBitmap::STRUCTURED_EVENT as u8, 3, 5),
        ]
        .concat(),
        b"tokevent",
    )
}

fn l3_transport_probe_selection() -> Result<(), String> {
    let providers = [
        TransportProviderDescriptor::available(
            TcpProvider::NAME,
            "1.0.0-preview.4",
            TransportId::Tcp,
            TransportProviderKind::PureRust,
        ),
        TransportProviderDescriptor::available(
            "nnrp-quic-native",
            "1.0.0-preview.4",
            TransportId::Quic,
            TransportProviderKind::NativeDynamic,
        ),
    ];
    let remote = RemoteTransportSupport::new([TransportId::Tcp, TransportId::Quic]);
    let samples = [
        ProbeSample::success(
            TransportId::Tcp,
            "nnrp.transport.tcp.native",
            10_000,
            1_500,
            512,
            512,
        ),
        ProbeSample::success(
            TransportId::Quic,
            "nnrp.transport.quic.native",
            10_000,
            800,
            512,
            512,
        ),
    ];
    let selection =
        select_transport_with_probe(&providers, &remote, TransportPolicy::Auto, None, &samples)
            .map_err(|error| error.to_string())?;
    if selection.selected.transport_id != TransportId::Quic {
        return Err("transport probe did not prefer the lower-latency QUIC sample".to_string());
    }

    let fallback_samples = [
        ProbeSample::success(
            TransportId::Tcp,
            "nnrp.transport.tcp.native",
            10_000,
            900,
            512,
            512,
        ),
        ProbeSample::failure(
            TransportId::Quic,
            "nnrp.transport.quic.native",
            10_000,
            true,
        ),
    ];
    let fallback = select_transport_with_probe(
        &providers,
        &remote,
        TransportPolicy::PreferQuic,
        None,
        &fallback_samples,
    )
    .map_err(|error| error.to_string())?;
    if fallback.selected.transport_id != TransportId::Tcp {
        return Err("transport probe did not fall back to TCP after QUIC failure".to_string());
    }
    Ok(())
}

fn l3_transport_tcp_session_smoke() -> Result<(), String> {
    run_tokio_smoke(tcp_session_smoke())
}

fn l3_transport_quic_session_smoke() -> Result<(), String> {
    run_tokio_smoke(quic_session_smoke())
}

fn run_tokio_smoke(
    future: impl std::future::Future<Output = Result<(), RuntimeError>>,
) -> Result<(), String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?
        .block_on(future)
        .map_err(|error| error.to_string())
}

async fn tcp_session_smoke() -> Result<(), RuntimeError> {
    let server =
        nnrp_runtime::NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;
    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        let submit = session.receive_submit().await?;
        session
            .send_result(submit.frame_id, token_result(), b"delta".to_vec())
            .await?;
        let close = session.receive_close().await?;
        session.ack_close(&close).await?;
        session.close().await
    });

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
    let mut session = client.open_session().await?;
    let frame_id = session.submit(token_submit(1), b"prompt".to_vec()).await?;
    let result = session.await_result().await?;
    if result.frame_id != frame_id || result.body != b"delta" {
        return Err(RuntimeError::UnexpectedMessage(
            "TCP smoke result did not preserve frame id and body",
        ));
    }
    session.close().await?;
    server_task.await.expect("server task should join")?;
    Ok(())
}

async fn quic_session_smoke() -> Result<(), RuntimeError> {
    let (endpoint_config, certificate) =
        QuicServerEndpointConfig::self_signed_localhost("127.0.0.1:0".parse().unwrap())?;
    let server = QuicProvider::bind(
        endpoint_config,
        quic_server_config(NnrpServerConfig::default()),
    )
    .await?;
    let addr = server.local_addr()?;
    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        let submit = session.receive_submit().await?;
        session
            .send_result(submit.frame_id, token_result(), b"delta".to_vec())
            .await?;
        let close = session.receive_close().await?;
        session.ack_close(&close).await?;
        session.close().await
    });

    let endpoint_config =
        QuicClientEndpointConfig::localhost_with_root_certificate(certificate.certificate_der);
    let client = QuicProvider::connect_addr(
        addr,
        endpoint_config,
        quic_client_config(NnrpClientConfig::default()),
    )
    .await?;
    let mut session = client.open_session().await?;
    let frame_id = session.submit(token_submit(1), b"prompt".to_vec()).await?;
    let result = session.await_result().await?;
    if result.frame_id != frame_id || result.body != b"delta" {
        return Err(RuntimeError::UnexpectedMessage(
            "QUIC smoke result did not preserve frame id and body",
        ));
    }
    session.close().await?;
    server_task.await.expect("server task should join")?;
    Ok(())
}

fn token_submit(operation_id: u64) -> FrameSubmitMetadata {
    FrameSubmitMetadata {
        src_width: 0,
        src_height: 0,
        tile_width: 0,
        tile_height: 0,
        tile_count: 0,
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
        operation_id,
        submit_mode: SubmitMode::Inline,
        budget_policy: 0,
        loss_tolerance_policy: 0,
        payload_frame_count: 1,
        object_ref_mask: 0,
        dependency_frame_id: 0,
        payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
    }
}

fn token_result() -> ResultPushMetadata {
    ResultPushMetadata {
        status_code: 0,
        result_flags: 0,
        section_count: 0,
        tile_count: 0,
        active_profile_id: STANDARD_PROFILE_TOKEN,
        inference_ms: 1,
        queue_ms: 0,
        server_total_ms: 1,
        tile_base_id: 0,
        tile_index_bytes: 0,
        result_class: ResultClass::Complete,
        applied_budget_policy: 0,
        reused_frame_id: 0,
        covered_tile_count: 0,
        dropped_tile_count: 0,
        payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
        payload_frame_count: 1,
    }
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
struct BaselineTypedPayloadDescriptor {
    payload_kind: u32,
    descriptor_flags: u8,
    profile_id: u16,
    payload_offset: u32,
    payload_length: u32,
}

impl BaselineTypedPayloadDescriptor {
    fn parse(source: &[u8]) -> Result<Self, String> {
        if source.len() != 16 {
            return Err(format!(
                "NNRP/1 baseline typed-payload descriptor must be 16 bytes, got {}",
                source.len()
            ));
        }
        let descriptor_flags = source[1];
        let reserved = read_u32(source, 12);
        if descriptor_flags != 0 || reserved != 0 {
            return Err(
                "NNRP/1 baseline typed-payload descriptor reserved fields must be zero".to_string(),
            );
        }
        let payload_kind = u32::from(source[0]);
        if payload_kind == 0
            || payload_kind & (payload_kind - 1) != 0
            || payload_kind & !PAYLOAD_KIND_KNOWN_MASK != 0
        {
            return Err(
                "NNRP/1 baseline typed-payload descriptor payload_kind is invalid".to_string(),
            );
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

fn validate_baseline_typed_payload_region(
    payload_kind_bitmap: u32,
    payload_frame_count: u16,
    descriptor_region: &[u8],
    payload_region: &[u8],
) -> Result<(), String> {
    if descriptor_region.len() != usize::from(payload_frame_count) * 16 {
        return Err(
            "NNRP/1 baseline descriptor bytes must match payload_frame_count * 16".to_string(),
        );
    }

    let mut next_offset = 0u32;
    for chunk in descriptor_region.chunks_exact(16) {
        let descriptor = BaselineTypedPayloadDescriptor::parse(chunk)?;
        if descriptor.payload_kind & !payload_kind_bitmap != 0 {
            return Err(
                "NNRP/1 baseline descriptor payload kind was not declared in metadata".to_string(),
            );
        }
        if descriptor.payload_offset != next_offset {
            return Err("NNRP/1 baseline typed-payload descriptors must be contiguous".to_string());
        }
        next_offset = descriptor
            .payload_offset
            .checked_add(descriptor.payload_length)
            .ok_or_else(|| "NNRP/1 baseline typed-payload range overflowed".to_string())?;
        if next_offset as usize > payload_region.len() {
            return Err("NNRP/1 baseline typed-payload range exceeds payload region".to_string());
        }
    }

    if next_offset as usize != payload_region.len() {
        return Err("NNRP/1 baseline payload region must be exactly covered".to_string());
    }
    Ok(())
}

fn descriptor(payload_kind: u8, offset: u32, length: u32) -> Vec<u8> {
    let descriptor = BaselineTypedPayloadDescriptor {
        payload_kind: u32::from(payload_kind),
        descriptor_flags: 0,
        profile_id: STANDARD_PROFILE_TOKEN,
        payload_offset: offset,
        payload_length: length,
    };
    descriptor.to_bytes()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_typed_payload_descriptor_round_trips() {
        l0_baseline_typed_payload_descriptor().expect("baseline descriptor should match fixture");
        l0_baseline_typed_payload_frame_regions()
            .expect("baseline typed-payload region should match fixture");
    }

    #[test]
    fn preview3_submit_fixture_rejects_short_source_and_destination() {
        assert_eq!(
            Preview3FrameSubmitMetadata::parse(&[0; FRAME_SUBMIT_METADATA_LEN - 1]),
            Err(nnrp_core::NnrpError::SourceTooShort {
                expected: FRAME_SUBMIT_METADATA_LEN,
                actual: FRAME_SUBMIT_METADATA_LEN - 1,
            })
        );

        let metadata = Preview3FrameSubmitMetadata::parse(&[0; FRAME_SUBMIT_METADATA_LEN])
            .expect("zeroed Preview3 submit metadata should parse");
        assert_eq!(
            metadata.write(&mut [0; FRAME_SUBMIT_METADATA_LEN - 1]),
            Err(nnrp_core::NnrpError::DestinationTooShort {
                expected: FRAME_SUBMIT_METADATA_LEN,
                actual: FRAME_SUBMIT_METADATA_LEN - 1,
            })
        );
    }

    #[test]
    fn baseline_typed_payload_descriptor_rejects_invalid_shape() {
        let short = [PayloadKindBitmap::TOKEN_CHUNK as u8; 15];
        assert!(BaselineTypedPayloadDescriptor::parse(&short)
            .expect_err("short descriptor should be rejected")
            .contains("16 bytes"));

        let mut reserved = descriptor(PayloadKindBitmap::TOKEN_CHUNK as u8, 0, 3);
        reserved[1] = 1;
        assert!(BaselineTypedPayloadDescriptor::parse(&reserved)
            .expect_err("reserved descriptor flags should be rejected")
            .contains("reserved fields"));

        let invalid_payload_kind = descriptor(3, 0, 3);
        assert!(BaselineTypedPayloadDescriptor::parse(&invalid_payload_kind)
            .expect_err("multi-bit payload kind should be rejected")
            .contains("payload_kind is invalid"));
    }

    #[test]
    fn baseline_typed_payload_region_rejects_invalid_descriptor_layouts() {
        assert!(validate_baseline_typed_payload_region(
            PayloadKindBitmap::TOKEN_CHUNK,
            1,
            &[0; 15],
            b""
        )
        .expect_err("descriptor byte count must match frame count")
        .contains("payload_frame_count"));

        assert!(validate_baseline_typed_payload_region(
            PayloadKindBitmap::TOKEN_CHUNK,
            1,
            &descriptor(PayloadKindBitmap::VIDEO_CHUNK as u8, 0, 3),
            b"tok",
        )
        .expect_err("undeclared payload kind should be rejected")
        .contains("not declared"));

        assert!(validate_baseline_typed_payload_region(
            PayloadKindBitmap::TOKEN_CHUNK,
            1,
            &descriptor(PayloadKindBitmap::TOKEN_CHUNK as u8, 1, 2),
            b"ok",
        )
        .expect_err("non-contiguous descriptor should be rejected")
        .contains("contiguous"));

        assert!(validate_baseline_typed_payload_region(
            PayloadKindBitmap::TOKEN_CHUNK,
            1,
            &descriptor(PayloadKindBitmap::TOKEN_CHUNK as u8, 0, 4),
            b"tok",
        )
        .expect_err("descriptor range must fit payload region")
        .contains("exceeds"));

        assert!(validate_baseline_typed_payload_region(
            PayloadKindBitmap::TOKEN_CHUNK,
            1,
            &descriptor(PayloadKindBitmap::TOKEN_CHUNK as u8, 0, 2),
            b"tok",
        )
        .expect_err("descriptor coverage must be exact")
        .contains("exactly covered"));
    }

    #[test]
    fn preview2_optional_baseline_cases_execute() {
        for case_id in [
            "l1.transport_probe.metadata.roundtrip",
            "l1.result_push.object_reference.resolve",
            "l1.typed_payload.region.pack",
            "l3.transport.probe.selection",
            "l3.transport.tcp.session_smoke",
            "l3.transport.quic.session_smoke",
        ] {
            assert_eq!(execute_nnrp1_baseline_case(case_id), Some(Ok(())));
        }
    }
}
