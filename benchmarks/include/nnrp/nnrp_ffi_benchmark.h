#ifndef NNRP_FFI_BENCHMARK_H
#define NNRP_FFI_BENCHMARK_H

#include "nnrp/nnrp_ffi.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct NnrpBenchmarkCompactResult {
  NnrpFfiStatus status;
  uint8_t has_result;
  uint32_t event_kind;
  uint32_t result_state;
  NnrpHandle operation;
  uint64_t operation_id;
  uint32_t frame_id;
  NnrpBufferView payload;
  NnrpFfiDiagnostic diagnostic;
} NnrpBenchmarkCompactResult;

typedef struct NnrpBenchmarkClientSubmitResultRequest {
  NnrpHandle session;
  uint64_t operation_id;
  uint32_t frame_id;
  NnrpBufferView submit_payload;
  NnrpBufferView result_payload;
  uintptr_t max_events;
} NnrpBenchmarkClientSubmitResultRequest;

typedef struct NnrpBenchmarkSessionOpenRequest {
  uint64_t connection_id;
  uint32_t requested_session_id;
  uint32_t generation;
} NnrpBenchmarkSessionOpenRequest;

typedef struct NnrpBenchmarkClientSubmitResultBatchRequest {
  NnrpHandle session;
  uint64_t operation_id_start;
  uint32_t frame_id_start;
  uint32_t frame_id_stride;
  NnrpBufferView submit_payload;
  NnrpBufferView result_payload;
  uintptr_t max_events;
  uintptr_t iterations;
} NnrpBenchmarkClientSubmitResultBatchRequest;

typedef struct NnrpBenchmarkClientRuntimeObjectLoopRequest {
  NnrpHandle session;
  uint64_t operation_id;
  uint32_t frame_id;
  NnrpBufferView submit_payload;
  NnrpRuntimeObjectDescriptor object_descriptor;
  NnrpBufferView object_metadata;
  NnrpCacheReferenceDescriptor cache_reference;
  NnrpBufferView cache_reference_metadata;
  NnrpProgressDescriptor progress;
  NnrpBufferView progress_body;
  NnrpPartialResultDescriptor partial_result;
  NnrpBufferView partial_body;
  NnrpObjectReleaseDescriptor object_release;
  NnrpBufferView release_diagnostics;
  NnrpBufferView result_payload;
  uintptr_t max_events;
} NnrpBenchmarkClientRuntimeObjectLoopRequest;

NnrpFfiStatus nnrp_benchmark_open_session(
    NnrpBenchmarkSessionOpenRequest request,
    NnrpHandle *out_session);
NnrpFfiStatus nnrp_benchmark_close_session(NnrpHandle session);
NnrpFfiStatus nnrp_benchmark_client_submit_result_compact(
    NnrpBenchmarkClientSubmitResultRequest request,
    NnrpBenchmarkCompactResult *out_result);
NnrpFfiStatus nnrp_benchmark_client_submit_result_compact_batch(
    NnrpBenchmarkClientSubmitResultBatchRequest request,
    NnrpBenchmarkCompactResult *out_last_result,
    uintptr_t *out_completed);
NnrpFfiStatus nnrp_benchmark_client_runtime_object_loop_compact(
    NnrpBenchmarkClientRuntimeObjectLoopRequest request,
    NnrpBenchmarkCompactResult *out_result);

#ifdef __cplusplus
}
#endif

#endif
