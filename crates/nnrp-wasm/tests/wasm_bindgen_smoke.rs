#![cfg(target_arch = "wasm32")]

use nnrp_core::MessageType;
use nnrp_wasm::{
    decode_websocket_binary_frame_batch_json, decode_websocket_binary_frame_json,
    encode_runtime_control_metadata_json, encode_websocket_binary_frame_json,
};
use serde_json::Value;
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
    let progress = r#"{"operation_id":42,"progress_sequence":7,"stage_code":8,"percent_x100":9000,"object_id":10,"body_bytes":3}"#;
    let tail = [30_u8, 31, 32];

    let encoded =
        encode_runtime_control_metadata_json(MessageType::Progress as u8, progress, &tail)
            .expect("wasm binding should encode progress metadata");

    assert_eq!(&encoded[encoded.len() - tail.len()..], tail);
}
