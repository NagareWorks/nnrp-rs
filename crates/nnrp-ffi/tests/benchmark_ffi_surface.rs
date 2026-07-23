#![cfg(feature = "benchmark-ffi")]

use nnrp_ffi::{
    nnrp_benchmark_client_submit_result_compact_batch, nnrp_benchmark_close_session,
    nnrp_benchmark_open_session, NnrpBenchmarkSessionOpenRequest, NnrpBufferView,
    NnrpClientSubmitResultBatchRequest, NnrpCompactResult, NnrpFfiStatus, NnrpFfiStatusCode,
    NnrpHandle, NnrpHandleKind,
};

#[test]
fn explicit_benchmark_feature_opens_session_and_runs_coarse_batch() {
    unsafe {
        let mut session = NnrpHandle::invalid();
        assert_eq!(
            nnrp_benchmark_open_session(
                NnrpBenchmarkSessionOpenRequest {
                    connection_id: 701,
                    requested_session_id: 702,
                    generation: 1,
                },
                &mut session,
            ),
            NnrpFfiStatus::ok()
        );
        assert_eq!(session.kind, NnrpHandleKind::Session as u32);

        let submit_payload = [1_u8, 2, 3];
        let result_payload = [4_u8, 5, 6];
        let mut result = NnrpCompactResult::none(NnrpFfiStatus::ok());
        let mut completed = 0_usize;
        assert_eq!(
            nnrp_benchmark_client_submit_result_compact_batch(
                NnrpClientSubmitResultBatchRequest {
                    session,
                    operation_id_start: 10_000,
                    frame_id_start: 20_000,
                    frame_id_stride: 1,
                    submit_payload: NnrpBufferView {
                        ptr: submit_payload.as_ptr(),
                        len: submit_payload.len(),
                    },
                    result_payload: NnrpBufferView {
                        ptr: result_payload.as_ptr(),
                        len: result_payload.len(),
                    },
                    max_events: 2,
                    iterations: 4_096,
                },
                &mut result,
                &mut completed,
            ),
            NnrpFfiStatus::ok()
        );

        assert_eq!(completed, 4_096);
        assert_eq!(result.status.status_code, NnrpFfiStatusCode::Ok as u32);
        assert_eq!(result.has_result, 1);
        assert_eq!(result.operation_id, 14_095);
        assert_eq!(result.frame_id, 24_095);
        assert_eq!(result.payload.len, result_payload.len());
        assert_eq!(nnrp_benchmark_close_session(session), NnrpFfiStatus::ok());
        assert_eq!(
            nnrp_benchmark_close_session(session),
            NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
        );
    }
}
