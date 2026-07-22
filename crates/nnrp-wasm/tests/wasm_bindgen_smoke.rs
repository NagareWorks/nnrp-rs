#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use js_sys::{Function, Promise, Uint8Array};
use nnrp_core::{
    BudgetMetadata, CommonHeader, FrameSubmitMetadata, InputProfile, MessageType,
    PayloadKindBitmap, ResultClass, ResultPushMetadata, SessionCloseAckMetadata,
    SessionCloseMetadata, SessionCloseStatus, SessionOpenAckMetadata, SessionOpenMetadata,
    SessionStatus, SubmitMode, TileIndexMode, RESULT_PUSH_METADATA_LEN,
    SESSION_CLOSE_ACK_METADATA_LEN, SESSION_ERROR_NONE, SESSION_OPEN_ACK_METADATA_LEN,
    STANDARD_PROFILE_TOKEN, TOKEN_DELTA_SCHEMA_ID, TOKEN_DELTA_SCHEMA_VERSION,
};
use nnrp_runtime::RuntimePacket;
use nnrp_wasm::{
    decode_runtime_control_metadata_json, decode_websocket_binary_frame_batch_json,
    decode_websocket_binary_frame_json, encode_runtime_control_metadata_json,
    encode_websocket_binary_frame_json, open_browser_client_role,
};
use serde_json::Value;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn wasm_bindgen_websocket_frame_codec_round_trips() {
    let header = format!(
        r#"{{"message_type":{},"session_id":9,"frame_id":10,"view_id":11,"route_id":12,"trace_id":13}}"#,
        MessageType::FrameSubmit as u8
    );
    let metadata = [1_u8, 3, 5, 7];
    let body = [2_u8, 4, 6];

    let frame = encode_websocket_binary_frame_json(&header, &metadata, &body)
        .expect("wasm binding should encode a websocket frame");
    let decoded = decode_websocket_binary_frame_json(&frame)
        .expect("wasm binding should decode a websocket frame");
    let decoded: Value = serde_json::from_str(&decoded).expect("decoded frame should be JSON");

    assert_eq!(
        decoded["header"]["message_type"],
        MessageType::FrameSubmit as u8
    );
    assert_eq!(decoded["header"]["session_id"], 9);
    assert_eq!(decoded["header"]["frame_id"], 10);
    assert_eq!(decoded["header"]["view_id"], 11);
    assert_eq!(decoded["header"]["route_id"], 12);
    assert_eq!(decoded["header"]["trace_id"], 13);
    assert_eq!(decoded["metadata_len"], metadata.len());
    assert_eq!(decoded["body_len"], body.len());
}

#[wasm_bindgen_test]
fn wasm_bindgen_websocket_frame_batch_reports_offsets_and_limits() {
    let progress_header = format!(r#"{{"message_type":{}}}"#, MessageType::Progress as u8);
    let partial_header = format!(r#"{{"message_type":{}}}"#, MessageType::PartialResult as u8);

    let first = encode_websocket_binary_frame_json(&progress_header, &[10, 11], &[12])
        .expect("first wasm frame should encode");
    let second = encode_websocket_binary_frame_json(&partial_header, &[20], &[21, 22])
        .expect("second wasm frame should encode");

    let mut batch = first.clone();
    batch.extend_from_slice(&second);

    let decoded = decode_websocket_binary_frame_batch_json(&batch, 0)
        .expect("wasm binding should decode an unlimited frame batch");
    let decoded: Value = serde_json::from_str(&decoded).expect("batch output should be JSON");
    let frames = decoded["frames"]
        .as_array()
        .expect("frames should be an array");

    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0]["frame_offset"], 0);
    assert_eq!(frames[1]["frame_offset"], first.len());
    assert_eq!(decoded["consumed_len"], batch.len());
    assert_eq!(decoded["remaining_len"], 0);

    let limited = decode_websocket_binary_frame_batch_json(&batch, 1)
        .expect("wasm binding should respect the frame limit");
    let limited: Value = serde_json::from_str(&limited).expect("limited batch should be JSON");
    assert_eq!(limited["frames"].as_array().unwrap().len(), 1);
    assert_eq!(limited["consumed_len"], first.len());
    assert_eq!(limited["remaining_len"], second.len());
}

#[wasm_bindgen_test]
fn wasm_bindgen_runtime_control_metadata_encodes_progress_tail() {
    let progress = r#"{"operation_id":"42","progress_sequence":"7","stage_code":8,"percent_x100":9000,"object_id":"10","body_bytes":3}"#;
    let tail = [30_u8, 31, 32];

    let encoded =
        encode_runtime_control_metadata_json(MessageType::Progress as u8, progress, &tail)
            .expect("wasm binding should encode progress metadata");
    let decoded = decode_runtime_control_metadata_json(MessageType::Progress as u8, &encoded)
        .expect("wasm binding should decode progress metadata");
    let decoded: Value = serde_json::from_str(&decoded).expect("decoded metadata should be JSON");

    assert_eq!(&encoded[encoded.len() - tail.len()..], tail);
    assert_eq!(decoded["metadata"]["operation_id"], "42");
    assert_eq!(decoded["metadata"]["progress_sequence"], "7");
}

#[wasm_bindgen_test(async)]
async fn wasm_bindgen_browser_role_runs_real_session_submit_and_close() {
    let responses = Rc::new(RefCell::new(VecDeque::<Vec<u8>>::new()));
    let send_responses = Rc::clone(&responses);
    let send = Closure::wrap(Box::new(move |packet: Uint8Array| -> Promise {
        if let Some(response) = browser_role_response(&packet.to_vec()) {
            send_responses.borrow_mut().push_back(response);
        }
        Promise::resolve(&JsValue::UNDEFINED)
    }) as Box<dyn FnMut(Uint8Array) -> Promise>);

    let receive_responses = Rc::clone(&responses);
    let receive = Closure::wrap(Box::new(move || -> Promise {
        match receive_responses.borrow_mut().pop_front() {
            Some(packet) => {
                let packet: JsValue = Uint8Array::from(packet.as_slice()).into();
                Promise::resolve(&packet)
            }
            None => Promise::reject(&JsValue::from_str("no scripted browser response")),
        }
    }) as Box<dyn FnMut() -> Promise>);

    let close_count = Rc::new(RefCell::new(0_u32));
    let observed_close_count = Rc::clone(&close_count);
    let close = Closure::wrap(Box::new(move || -> Promise {
        *observed_close_count.borrow_mut() += 1;
        Promise::resolve(&JsValue::UNDEFINED)
    }) as Box<dyn FnMut() -> Promise>);

    let config = serde_json::json!({
        "requestedSessionId": 7,
        "profileId": STANDARD_PROFILE_TOKEN,
        "schemaId": TOKEN_DELTA_SCHEMA_ID,
        "schemaVersion": TOKEN_DELTA_SCHEMA_VERSION,
        "priorityClass": 1,
        "defaultDeadlineMs": 500,
        "maxInFlightOperations": 4,
        "leaseTtlHintMs": 30_000,
        "maxPacketBytes": 64 * 1024 * 1024,
    });
    let mut role = open_browser_client_role(
        send.as_ref().unchecked_ref::<Function>().clone(),
        receive.as_ref().unchecked_ref::<Function>().clone(),
        close.as_ref().unchecked_ref::<Function>().clone(),
        &config.to_string(),
    )
    .await
    .expect("browser role should open a real NNRP session");

    let submit = token_submit(42);
    let mut payload = Vec::from(submit.to_bytes().expect("submit metadata should encode"));
    payload.extend_from_slice(b"prompt");
    assert_eq!(
        role.submit_no_wait(9, &payload)
            .await
            .expect("browser role should submit through the Rust runtime"),
        9
    );

    let budget = BudgetMetadata {
        operation_id: 42,
        compute_budget_units: 100,
        memory_budget_bytes: 200,
        bandwidth_budget_bytes: 300,
        token_budget: 400,
        flags: 0,
    };
    role.send_runtime_frame(
        MessageType::BudgetUpdate as u8,
        9,
        &budget.to_bytes().expect("budget metadata should encode"),
    )
    .await
    .expect("browser role should route runtime controls through the Rust session");

    let event = role
        .await_event()
        .await
        .expect("browser role should validate and return the result event");
    assert_eq!(event.message_type(), MessageType::ResultPush as u8);
    assert_eq!(event.session_id(), 7);
    assert_eq!(event.frame_id(), 9);
    assert_eq!(event.metadata().length(), RESULT_PUSH_METADATA_LEN as u32);
    assert_eq!(event.body().to_vec(), b"answer");

    role.close()
        .await
        .expect("browser role should close the session and carrier");
    role.close()
        .await
        .expect("browser role close should be idempotent");
    assert_eq!(*close_count.borrow(), 1);
}

fn browser_role_response(packet: &[u8]) -> Option<Vec<u8>> {
    let (header, metadata, _) =
        CommonHeader::parse_packet(packet).expect("browser carrier should receive a valid packet");
    match header.message_type {
        MessageType::SessionOpen => {
            let open = SessionOpenMetadata::parse(metadata).expect("session open should parse");
            let ack = SessionOpenAckMetadata {
                session_id: open.requested_session_id,
                accepted_profile_id: open.profile_id,
                accepted_priority_class: open.priority_class,
                session_status: SessionStatus::Opened,
                schema_id: open.schema_id,
                schema_version: open.schema_version,
                granted_operation_credit: open.max_in_flight_operations,
                max_in_flight_operations: open.max_in_flight_operations,
                lease_ttl_ms: open.lease_ttl_hint_ms,
                resume_window_ms: 0,
                resume_token_bytes: 0,
                session_extension_bytes: 0,
                server_session_tag: open.client_session_tag,
                route_scope_id: 0,
                session_error_code: SESSION_ERROR_NONE,
                session_flags_ack: 0,
            };
            Some(response_packet(
                MessageType::SessionOpenAck,
                ack.session_id,
                0,
                ack.to_bytes().expect("session ack should encode").to_vec(),
                Vec::new(),
                SESSION_OPEN_ACK_METADATA_LEN,
            ))
        }
        MessageType::FrameSubmit => {
            FrameSubmitMetadata::parse(metadata).expect("frame submit should parse");
            let result = ResultPushMetadata {
                status_code: 200,
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
            };
            Some(response_packet(
                MessageType::ResultPush,
                header.session_id,
                header.frame_id,
                result.to_bytes().expect("result should encode").to_vec(),
                b"answer".to_vec(),
                RESULT_PUSH_METADATA_LEN,
            ))
        }
        MessageType::BudgetUpdate => {
            assert_eq!(header.session_id, 7);
            assert_eq!(header.frame_id, 9);
            let budget = BudgetMetadata::parse(metadata).expect("budget update should parse");
            assert_eq!(budget.operation_id, 42);
            None
        }
        MessageType::SessionClose => {
            let close = SessionCloseMetadata::parse(metadata).expect("session close should parse");
            let ack = SessionCloseAckMetadata {
                close_status: SessionCloseStatus::Closed,
                last_operation_id: close.last_operation_id,
                session_error_code: SESSION_ERROR_NONE,
            };
            Some(response_packet(
                MessageType::SessionCloseAck,
                header.session_id,
                0,
                ack.to_bytes().expect("close ack should encode").to_vec(),
                Vec::new(),
                SESSION_CLOSE_ACK_METADATA_LEN,
            ))
        }
        message_type => panic!("unexpected browser role packet: {message_type:?}"),
    }
}

fn response_packet(
    message_type: MessageType,
    session_id: u32,
    frame_id: u32,
    metadata: Vec<u8>,
    body: Vec<u8>,
    expected_metadata_len: usize,
) -> Vec<u8> {
    assert_eq!(metadata.len(), expected_metadata_len);
    let mut header = CommonHeader::new(message_type, metadata.len() as u32, body.len() as u32);
    header.session_id = session_id;
    header.frame_id = frame_id;
    RuntimePacket::new(header, metadata, body)
        .expect("response packet should be valid")
        .to_bytes()
        .expect("response packet should encode")
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
        latency_budget_ms: 25,
        target_fps_x100: 0,
        retry_of_frame: 0,
        tile_base_id: 0,
        camera_bytes: 0,
        tile_index_bytes: 0,
        operation_id,
        submit_mode: SubmitMode::Inline,
        budget_policy: 0,
        loss_tolerance_policy: 0,
        object_ref_mask: 0,
        dependency_frame_id: 0,
        payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
        payload_frame_count: 1,
    }
}
