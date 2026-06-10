#ifndef NNRP_FFI_H
#define NNRP_FFI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct NnrpProtocolVersion {
  uint8_t major;
  uint8_t wire_format;
} NnrpProtocolVersion;

#define NNRP_FFI_ABI_MAJOR 1
#define NNRP_FFI_ABI_MINOR 10
#define NNRP_FFI_ABI_PATCH 0

#define NNRP_TRANSPORT_SLOT_QUIC 0x00000001u
#define NNRP_TRANSPORT_SLOT_TCP 0x00000002u

#define NNRP_RUNTIME_FEATURE_PROTOCOL_CORE 0x0000000000000001ull
#define NNRP_RUNTIME_FEATURE_CLIENT_API 0x0000000000000002ull
#define NNRP_RUNTIME_FEATURE_SERVER_API 0x0000000000000004ull
#define NNRP_RUNTIME_FEATURE_EVENT_POLLING 0x0000000000000008ull
#define NNRP_RUNTIME_FEATURE_CALLBACK_DISPATCH 0x0000000000000010ull
#define NNRP_RUNTIME_FEATURE_CACHE_SCHEMA 0x0000000000000020ull
#define NNRP_RUNTIME_FEATURE_RECOVERY 0x0000000000000040ull
#define NNRP_RUNTIME_FEATURE_TYPED_PAYLOAD 0x0000000000000080ull
#define NNRP_RUNTIME_FEATURE_TRANSPORT_SLOTS 0x0000000000000100ull
#define NNRP_RUNTIME_FEATURE_BATCH_POLLING 0x0000000000000200ull
#define NNRP_RUNTIME_FEATURE_CACHE_LEASE_OPS 0x0000000000000400ull
#define NNRP_RUNTIME_FEATURE_SCHEMA_REGISTRY_HANDLES 0x0000000000000800ull
#define NNRP_RUNTIME_FEATURE_BUFFER_HANDLES 0x0000000000001000ull
#define NNRP_RUNTIME_FEATURE_EXECUTABLE_RESUME 0x0000000000002000ull
#define NNRP_RUNTIME_FEATURE_CLIENT_COMPLETION_HELPERS 0x0000000000004000ull
#define NNRP_RUNTIME_FEATURE_CLIENT_COARSE_RESULT_HELPERS 0x0000000000008000ull
#define NNRP_RUNTIME_FEATURE_CLIENT_COMPACT_RESULT_HELPERS 0x0000000000010000ull
#define NNRP_RUNTIME_FEATURE_PREVIEW4_CONTROL_EVENTS 0x0000000000020000ull
#define NNRP_RUNTIME_FEATURE_PREVIEW4_OBJECT_CACHE_EVENTS 0x0000000000040000ull

#define NNRP_RESULT_STATE_NONE 0u
#define NNRP_RESULT_STATE_COMPLETED 1u
#define NNRP_RESULT_STATE_PARTIAL 2u
#define NNRP_RESULT_STATE_DEGRADED 3u
#define NNRP_RESULT_STATE_STALE_REUSE 4u
#define NNRP_RESULT_STATE_CANCELLED 5u
#define NNRP_RESULT_STATE_FAILED 6u

#define NNRP_SESSION_RECOVERY_OUTCOME_FRESH 0u
#define NNRP_SESSION_RECOVERY_OUTCOME_RESUME_ENABLED 1u
#define NNRP_SESSION_RECOVERY_OUTCOME_RESUMED 2u
#define NNRP_SESSION_RECOVERY_OUTCOME_RESUME_REJECTED 3u

#define NNRP_SCHEMA_REGISTRY_ACTION_INSTALLED 0u
#define NNRP_SCHEMA_REGISTRY_ACTION_ALREADY_INSTALLED 1u
#define NNRP_SCHEMA_REGISTRY_ACTION_UPDATED 2u
#define NNRP_SCHEMA_REGISTRY_ACTION_INVALIDATED 3u

#define NNRP_CACHE_LEASE_OUTCOME_VALID 0u
#define NNRP_CACHE_LEASE_OUTCOME_MISS 1u
#define NNRP_CACHE_LEASE_OUTCOME_EXPIRED 2u
#define NNRP_CACHE_LEASE_OUTCOME_RELEASED 3u

typedef struct NnrpRuntimeCapabilities {
  uint16_t abi_major;
  uint16_t abi_minor;
  uint16_t abi_patch;
  uint16_t reserved0;
  NnrpProtocolVersion protocol_version;
  uint16_t sdk_major;
  uint16_t sdk_minor;
  uint16_t sdk_patch;
  uint16_t sdk_preview;
  uint16_t sdk_revision;
  uint16_t reserved1;
  uint32_t transport_slots;
  uint64_t feature_flags;
} NnrpRuntimeCapabilities;

typedef enum NnrpFfiStatusCode {
  NNRP_FFI_STATUS_OK = 0,
  NNRP_FFI_STATUS_INVALID_ARGUMENT = 1,
  NNRP_FFI_STATUS_INVALID_HANDLE = 2,
  NNRP_FFI_STATUS_INVALID_STATE = 3,
  NNRP_FFI_STATUS_PROTOCOL_ERROR = 4,
  NNRP_FFI_STATUS_WOULD_BLOCK = 5,
  NNRP_FFI_STATUS_CALLBACK_REJECTED = 6,
  NNRP_FFI_STATUS_INTERNAL_ERROR = 0xffff
} NnrpFfiStatusCode;

typedef enum NnrpErrorFamily {
  NNRP_ERROR_FAMILY_NONE = 0,
  NNRP_ERROR_FAMILY_SESSION = 1,
  NNRP_ERROR_FAMILY_CACHE = 2,
  NNRP_ERROR_FAMILY_SCHEMA = 3,
  NNRP_ERROR_FAMILY_TRANSPORT = 4,
  NNRP_ERROR_FAMILY_LIFECYCLE = 5,
  NNRP_ERROR_FAMILY_OPERATION = 6,
  NNRP_ERROR_FAMILY_CONTROL = 7,
  NNRP_ERROR_FAMILY_RUNTIME_OBJECT = 8,
  NNRP_ERROR_FAMILY_INTERNAL = 0xffff
} NnrpErrorFamily;

typedef enum NnrpHandleKind {
  NNRP_HANDLE_INVALID = 0,
  NNRP_HANDLE_CONNECTION = 1,
  NNRP_HANDLE_SESSION = 2,
  NNRP_HANDLE_OPERATION = 3,
  NNRP_HANDLE_EVENT_PUMP = 4,
  NNRP_HANDLE_BUFFER = 5,
  NNRP_HANDLE_SCHEMA_REGISTRY = 6,
  NNRP_HANDLE_CACHE_LEASE = 7,
  NNRP_HANDLE_OBJECT_DESCRIPTOR = 8,
  NNRP_HANDLE_CACHE_REFERENCE_DESCRIPTOR = 9
} NnrpHandleKind;

typedef enum NnrpEventKind {
  NNRP_EVENT_NONE = 0,
  NNRP_EVENT_CONNECTION_OPENED = 1,
  NNRP_EVENT_SESSION_OPENED = 2,
  NNRP_EVENT_SESSION_PATCHED = 3,
  NNRP_EVENT_SESSION_CLOSED = 4,
  NNRP_EVENT_SUBMIT_ACCEPTED = 5,
  NNRP_EVENT_RESULT_PUSHED = 6,
  NNRP_EVENT_RESULT_DROPPED = 7,
  NNRP_EVENT_FLOW_UPDATED = 8,
  NNRP_EVENT_CONTROL = 9,
  NNRP_EVENT_ERROR = 10,
  NNRP_EVENT_RESULT_HINT = 11
} NnrpEventKind;

typedef struct NnrpFfiStatus {
  uint32_t status_code;
  uint32_t error_family;
  uint32_t protocol_error_code;
  uint32_t detail_code;
} NnrpFfiStatus;

typedef struct NnrpFfiDiagnostic {
  NnrpFfiStatus status;
  uint64_t related_connection_id;
  uint32_t related_session_id;
  uint64_t related_operation_id;
  uint32_t related_frame_id;
} NnrpFfiDiagnostic;

typedef struct NnrpHandle {
  uint32_t kind;
  uint64_t id;
  uint32_t generation;
  uint32_t flags;
} NnrpHandle;

typedef struct NnrpBufferView {
  const uint8_t *ptr;
  uintptr_t len;
} NnrpBufferView;

typedef struct NnrpBufferViewMut {
  uint8_t *ptr;
  uintptr_t len;
} NnrpBufferViewMut;

typedef struct NnrpSchemaDescriptorHeader {
  uint32_t schema_id;
  uint32_t schema_version;
  uint16_t profile_id;
  uint16_t schema_flags;
  uint8_t min_version_major;
  uint8_t max_version_major;
  uint16_t reserved0;
  uint32_t body_bytes;
  uint16_t dependency_count;
  uint16_t default_stream_semantics;
  uint64_t schema_hash;
} NnrpSchemaDescriptorHeader;

typedef struct NnrpTypedPayloadDescriptor {
  uint16_t profile_id;
  uint16_t descriptor_flags;
  uint32_t schema_id;
  uint32_t schema_version;
  uint16_t stream_semantics;
  uint16_t reserved0;
  uint32_t offset;
  uint32_t length;
} NnrpTypedPayloadDescriptor;

typedef struct NnrpSessionRecoveryOutcome {
  uint32_t outcome_code;
  uint32_t resume_window_ms;
} NnrpSessionRecoveryOutcome;

typedef struct NnrpCacheObjectId {
  uint32_t cache_namespace;
  uint32_t cache_key_hi;
  uint32_t cache_key_lo;
  uint32_t object_kind;
} NnrpCacheObjectId;

typedef struct NnrpCacheLeaseRequest {
  NnrpHandle owner;
  NnrpCacheObjectId object_id;
  uint64_t expected_version;
  uint64_t now_ms;
  uint32_t ttl_ms;
} NnrpCacheLeaseRequest;

typedef struct NnrpCacheLeaseResult {
  uint32_t outcome_code;
  NnrpHandle lease_handle;
  NnrpCacheObjectId object_id;
  uint64_t object_version;
  uint64_t lease_id;
  uint64_t expires_at_ms;
} NnrpCacheLeaseResult;

typedef struct NnrpSessionResumeRequest {
  NnrpHandle connection;
  uint32_t requested_session_id;
  uint32_t generation;
  uint16_t profile_id;
  uint32_t schema_id;
  uint32_t schema_version;
  uint32_t resume_token_bytes;
} NnrpSessionResumeRequest;

typedef struct NnrpEvent {
  uint32_t kind;
  NnrpHandle connection;
  NnrpHandle session;
  NnrpHandle operation;
  uint32_t frame_id;
  NnrpBufferView payload;
  NnrpFfiDiagnostic diagnostic;
} NnrpEvent;

typedef uint32_t (*NnrpEventCallback)(void *user_data, const NnrpEvent *event);

typedef struct NnrpCallbackSink {
  void *user_data;
  NnrpEventCallback on_event;
} NnrpCallbackSink;

typedef struct NnrpPollResult {
  NnrpFfiStatus status;
  uint8_t has_event;
  NnrpEvent event;
} NnrpPollResult;

typedef struct NnrpCompactResult {
  NnrpFfiStatus status;
  uint8_t has_result;
  uint32_t event_kind;
  uint32_t result_state;
  NnrpHandle operation;
  uint64_t operation_id;
  uint32_t frame_id;
  NnrpBufferView payload;
  NnrpFfiDiagnostic diagnostic;
} NnrpCompactResult;

typedef struct NnrpConnectionBootstrap {
  uint64_t connection_id;
  uint32_t generation;
  uint32_t transport_id;
} NnrpConnectionBootstrap;

typedef struct NnrpClientConnectRequest {
  uint64_t connection_id;
  uint32_t generation;
  uint32_t transport_id;
} NnrpClientConnectRequest;

typedef struct NnrpServerBindRequest {
  uint64_t server_id;
  uint32_t generation;
  uint32_t transport_id;
} NnrpServerBindRequest;

typedef struct NnrpSessionOpenRequest {
  NnrpHandle connection;
  uint32_t requested_session_id;
  uint32_t generation;
  uint16_t profile_id;
  uint32_t schema_id;
  uint32_t schema_version;
} NnrpSessionOpenRequest;

typedef struct NnrpSubmitRequest {
  NnrpHandle session;
  uint64_t operation_id;
  uint32_t frame_id;
  NnrpBufferView payload;
} NnrpSubmitRequest;

typedef struct NnrpClientCancelRequest {
  NnrpHandle session;
  uint32_t frame_id;
} NnrpClientCancelRequest;

typedef struct NnrpServerAcceptRequest {
  NnrpHandle server;
  uint32_t session_id;
  uint32_t generation;
  uint16_t profile_id;
  uint32_t schema_id;
  uint32_t schema_version;
} NnrpServerAcceptRequest;

typedef struct NnrpServerReceiveSubmitRequest {
  NnrpHandle session;
  uint64_t operation_id;
  uint32_t frame_id;
  NnrpBufferView payload;
} NnrpServerReceiveSubmitRequest;

typedef struct NnrpServerSendResultRequest {
  NnrpHandle operation;
  NnrpBufferView payload;
} NnrpServerSendResultRequest;

typedef struct NnrpClientCompleteOperationRequest {
  NnrpHandle operation;
  NnrpBufferView payload;
} NnrpClientCompleteOperationRequest;

typedef struct NnrpClientDropOperationRequest {
  NnrpHandle operation;
} NnrpClientDropOperationRequest;

typedef struct NnrpClientSubmitResultRequest {
  NnrpHandle session;
  uint64_t operation_id;
  uint32_t frame_id;
  NnrpBufferView submit_payload;
  NnrpBufferView result_payload;
  uintptr_t max_events;
} NnrpClientSubmitResultRequest;

typedef struct NnrpClientSubmitResultBatchRequest {
  NnrpHandle session;
  uint64_t operation_id_start;
  uint32_t frame_id_start;
  uint32_t frame_id_stride;
  NnrpBufferView submit_payload;
  NnrpBufferView result_payload;
  uintptr_t max_events;
  uintptr_t iterations;
} NnrpClientSubmitResultBatchRequest;

typedef struct NnrpRuntimeObjectDescriptor {
  uint64_t object_id;
  uint16_t object_kind;
  uint8_t producer_role;
  uint8_t consumer_role;
  uint32_t session_id;
  uint64_t byte_size;
  uint32_t compute_cost_units;
  uint16_t memory_location_hint;
  uint16_t ownership_hint;
  uint32_t lifetime_hint_ms;
  uint32_t metadata_bytes;
} NnrpRuntimeObjectDescriptor;

typedef struct NnrpObjectReferenceDescriptor {
  uint64_t object_id;
  uint64_t operation_id;
  uint64_t object_version;
  uint64_t offset;
  uint64_t length;
  uint32_t flags;
  uint32_t metadata_bytes;
} NnrpObjectReferenceDescriptor;

typedef struct NnrpObjectReleaseDescriptor {
  uint64_t object_id;
  uint64_t operation_id;
  uint16_t release_reason;
  uint8_t source_role;
  uint8_t flags;
  uint32_t diagnostic_bytes;
} NnrpObjectReleaseDescriptor;

typedef struct NnrpObjectDeltaDescriptor {
  uint64_t object_id;
  uint64_t delta_sequence;
  uint64_t region_offset;
  uint32_t region_bytes;
  uint32_t delta_bytes;
  uint32_t flags;
  uint32_t metadata_bytes;
} NnrpObjectDeltaDescriptor;

typedef struct NnrpCacheReferenceDescriptor {
  uint64_t cache_key_hi;
  uint64_t cache_key_lo;
  uint16_t profile_id;
  uint16_t reuse_scope;
  uint64_t lease_id;
  uint64_t producer_trace_id;
  uint32_t expiration_hint_ms;
  uint32_t metadata_bytes;
  uint32_t flags;
} NnrpCacheReferenceDescriptor;

typedef struct NnrpCacheMissDescriptor {
  uint64_t cache_key_hi;
  uint64_t cache_key_lo;
  uint16_t miss_reason;
  uint16_t profile_id;
  uint32_t diagnostic_bytes;
} NnrpCacheMissDescriptor;

typedef struct NnrpProgressDescriptor {
  uint64_t operation_id;
  uint64_t progress_sequence;
  uint16_t stage_code;
  uint16_t percent_x100;
  uint64_t object_id;
  uint32_t body_bytes;
} NnrpProgressDescriptor;

typedef struct NnrpPartialResultDescriptor {
  uint64_t operation_id;
  uint64_t result_sequence;
  uint64_t object_id;
  uint64_t delta_sequence;
  uint32_t body_bytes;
  uint32_t flags;
} NnrpPartialResultDescriptor;

typedef struct NnrpClientRuntimeObjectLoopRequest {
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
} NnrpClientRuntimeObjectLoopRequest;

typedef struct NnrpServerFlowUpdateRequest {
  NnrpHandle session;
  uint32_t frame_id;
} NnrpServerFlowUpdateRequest;

typedef struct NnrpControlRequest {
  NnrpHandle handle;
  uint32_t control_code;
  NnrpBufferView payload;
} NnrpControlRequest;

NnrpProtocolVersion nnrp_current_protocol_version(void);
NnrpRuntimeCapabilities nnrp_runtime_capabilities(void);
NnrpFfiStatus nnrp_connection_bootstrap(NnrpConnectionBootstrap request, NnrpHandle *out_connection);
NnrpFfiStatus nnrp_client_connect(NnrpClientConnectRequest request, NnrpHandle *out_connection);
NnrpFfiStatus nnrp_session_open(NnrpSessionOpenRequest request, NnrpHandle *out_session);
NnrpFfiStatus nnrp_client_open_session(NnrpSessionOpenRequest request, NnrpHandle *out_session);
NnrpFfiStatus nnrp_client_resume_session(NnrpSessionResumeRequest request, NnrpHandle *out_session, NnrpSessionRecoveryOutcome *out_outcome);
NnrpFfiStatus nnrp_submit(NnrpSubmitRequest request, NnrpHandle *out_operation);
NnrpFfiStatus nnrp_client_submit(NnrpSubmitRequest request, NnrpHandle *out_operation);
NnrpFfiStatus nnrp_session_close(NnrpHandle session);
NnrpFfiStatus nnrp_client_close(NnrpHandle session);
NnrpFfiStatus nnrp_connection_close(NnrpHandle connection);
NnrpFfiStatus nnrp_client_close_connection(NnrpHandle connection);
NnrpFfiStatus nnrp_client_cancel(NnrpClientCancelRequest request);
NnrpFfiStatus nnrp_client_complete_operation(NnrpClientCompleteOperationRequest request);
NnrpFfiStatus nnrp_client_drop_operation(NnrpClientDropOperationRequest request);
NnrpFfiStatus nnrp_client_submit_result(NnrpClientSubmitResultRequest request, NnrpHandle *out_operation, NnrpPollResult *out_result);
NnrpFfiStatus nnrp_client_submit_result_compact(NnrpClientSubmitResultRequest request, NnrpCompactResult *out_result);
NnrpFfiStatus nnrp_client_submit_result_compact_batch(NnrpClientSubmitResultBatchRequest request, NnrpCompactResult *out_last_result, uintptr_t *out_completed);
NnrpFfiStatus nnrp_client_submit_runtime_object_loop_compact(NnrpClientRuntimeObjectLoopRequest request, NnrpCompactResult *out_result);
NnrpFfiStatus nnrp_client_send_flow_update(NnrpServerFlowUpdateRequest request);
NnrpFfiStatus nnrp_client_send_result_hint(NnrpControlRequest request);
NnrpFfiStatus nnrp_client_await_event(NnrpHandle connection, NnrpPollResult *out_result);
NnrpFfiStatus nnrp_client_await_events(NnrpHandle connection, NnrpEvent *out_events, uintptr_t event_capacity, uintptr_t *out_event_count);
NnrpFfiStatus nnrp_schema_descriptor_parse(NnrpBufferView source, NnrpSchemaDescriptorHeader *out_descriptor);
NnrpFfiStatus nnrp_schema_descriptor_write(NnrpSchemaDescriptorHeader descriptor, NnrpBufferViewMut destination);
NnrpFfiStatus nnrp_token_delta_schema_descriptor(NnrpSchemaDescriptorHeader *out_descriptor);
NnrpFfiStatus nnrp_typed_payload_descriptor_parse(NnrpBufferView source, NnrpTypedPayloadDescriptor *out_descriptor);
NnrpFfiStatus nnrp_typed_payload_descriptor_write(NnrpTypedPayloadDescriptor descriptor, NnrpBufferViewMut destination);
NnrpFfiStatus nnrp_typed_payload_validate_binding(const NnrpSchemaDescriptorHeader *schema_descriptors, uintptr_t schema_count, NnrpTypedPayloadDescriptor descriptor);
NnrpFfiStatus nnrp_schema_registry_create(NnrpHandle *out_registry);
NnrpFfiStatus nnrp_schema_registry_install(NnrpHandle registry_handle, NnrpSchemaDescriptorHeader descriptor, uint32_t *out_action);
NnrpFfiStatus nnrp_schema_registry_lookup(NnrpHandle registry_handle, uint32_t schema_id, uint32_t schema_version, NnrpSchemaDescriptorHeader *out_descriptor);
NnrpFfiStatus nnrp_schema_registry_invalidate(NnrpHandle registry_handle, uint32_t schema_id, uint32_t schema_version, uint32_t *out_action);
NnrpFfiStatus nnrp_schema_registry_validate_binding(NnrpHandle registry_handle, NnrpTypedPayloadDescriptor descriptor);
NnrpFfiStatus nnrp_schema_registry_release(NnrpHandle registry_handle);
NnrpFfiStatus nnrp_session_recovery_request_validate(NnrpBufferView session_open_metadata);
NnrpFfiStatus nnrp_session_recovery_ack_validate(NnrpBufferView session_open_metadata, NnrpBufferView session_open_ack_metadata, NnrpSessionRecoveryOutcome *out_outcome);
NnrpFfiStatus nnrp_migration_recovery_validate(NnrpBufferView session_migrate_metadata, NnrpBufferView session_migrate_ack_metadata);
NnrpFfiStatus nnrp_migration_should_replay_frame(NnrpBufferView session_migrate_ack_metadata, uint64_t frame_id, uint8_t *out_should_replay);
NnrpFfiStatus nnrp_buffer_acquire_copy(NnrpBufferView source, NnrpHandle *out_buffer, NnrpBufferView *out_view);
NnrpFfiStatus nnrp_buffer_view(NnrpHandle buffer, NnrpBufferView *out_view);
NnrpFfiStatus nnrp_buffer_release(NnrpHandle buffer);
NnrpFfiStatus nnrp_object_metadata_buffer_acquire_copy(NnrpBufferView source, NnrpHandle *out_buffer, NnrpBufferView *out_view);
NnrpFfiStatus nnrp_object_metadata_buffer_view(NnrpHandle buffer, NnrpBufferView *out_view);
NnrpFfiStatus nnrp_object_metadata_buffer_release(NnrpHandle buffer);
NnrpFfiStatus nnrp_object_descriptor_create(NnrpRuntimeObjectDescriptor descriptor, NnrpBufferView metadata, NnrpHandle *out_handle);
NnrpFfiStatus nnrp_object_descriptor_view(NnrpHandle handle, NnrpRuntimeObjectDescriptor *out_descriptor, NnrpBufferView *out_metadata);
NnrpFfiStatus nnrp_object_descriptor_metadata_snapshot(NnrpHandle handle, NnrpHandle *out_buffer, NnrpBufferView *out_view);
NnrpFfiStatus nnrp_object_descriptor_release(NnrpHandle handle);
NnrpFfiStatus nnrp_cache_reference_descriptor_create(NnrpCacheReferenceDescriptor descriptor, NnrpBufferView metadata, NnrpHandle *out_handle);
NnrpFfiStatus nnrp_cache_reference_descriptor_view(NnrpHandle handle, NnrpCacheReferenceDescriptor *out_descriptor, NnrpBufferView *out_metadata);
NnrpFfiStatus nnrp_cache_reference_descriptor_metadata_snapshot(NnrpHandle handle, NnrpHandle *out_buffer, NnrpBufferView *out_view);
NnrpFfiStatus nnrp_cache_reference_descriptor_release(NnrpHandle handle);
NnrpFfiStatus nnrp_cache_query(NnrpCacheLeaseRequest request, NnrpCacheLeaseResult *out_result);
NnrpFfiStatus nnrp_cache_touch(NnrpCacheLeaseRequest request, NnrpCacheLeaseResult *out_result);
NnrpFfiStatus nnrp_cache_prefetch(NnrpHandle owner, const NnrpCacheObjectId *objects, uintptr_t object_count, uint64_t now_ms, uint32_t ttl_ms, NnrpCacheLeaseResult *out_results);
NnrpFfiStatus nnrp_cache_release(NnrpHandle lease_handle, NnrpCacheLeaseResult *out_result);
NnrpFfiStatus nnrp_server_bind(NnrpServerBindRequest request, NnrpHandle *out_server);
NnrpFfiStatus nnrp_server_accept(NnrpServerAcceptRequest request, NnrpHandle *out_session);
NnrpFfiStatus nnrp_server_receive_submit(NnrpServerReceiveSubmitRequest request, NnrpHandle *out_operation);
NnrpFfiStatus nnrp_server_send_result(NnrpServerSendResultRequest request);
NnrpFfiStatus nnrp_server_send_flow_update(NnrpServerFlowUpdateRequest request);
NnrpFfiStatus nnrp_server_close(NnrpHandle session);
NnrpFfiStatus nnrp_control(NnrpControlRequest request);
NnrpFfiStatus nnrp_poll_empty(NnrpPollResult *out_result);
NnrpFfiStatus nnrp_dispatch_event(NnrpCallbackSink sink, const NnrpEvent *event);

#ifdef __cplusplus
}
#endif

#endif
