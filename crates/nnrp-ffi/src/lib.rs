use core::ffi::c_void;
use std::collections::BTreeMap;
#[cfg(any(test, feature = "benchmark-ffi"))]
use std::collections::VecDeque;
#[cfg(not(test))]
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use nnrp_core::{
    should_replay_frame_after_migration, token_delta_schema_descriptor,
    validate_control_request_semantics, validate_migration_recovery,
    validate_partial_result_semantics, validate_pressure_semantics, validate_progress_semantics,
    validate_result_drop_reason_semantics, validate_scheduling_semantics,
    validate_session_recovery_ack, validate_session_recovery_request,
    validate_trace_context_semantics, BudgetMetadata, CacheInvalidateMetadata, CacheLease,
    CacheLeaseOwnerScope, CacheMissMetadata, CacheMissReason, CacheObjectId, CacheObjectKind,
    CacheReferenceMetadata, CacheReuseScope, CacheValidationFailure, CapabilityMetadata,
    ControlRequestMetadata, ErrorScope, FlowUpdateMetadata, MemoryLocationHint, MessageType,
    NnrpError, ObjectDeltaMetadata, ObjectDescriptorMetadata, ObjectReferenceMetadata,
    ObjectReleaseMetadata, ObjectReleaseReason, OwnershipHint, PartialResultMetadata,
    PressureMetadata, ProgressMetadata, ProtocolVersion, RecoverableErrorMetadata,
    ResultDropReasonMetadata, ResultHintMetadata, RetryAfterMetadata, RouteHintMetadata,
    RuntimeObjectKind, RuntimeRole, SchedulingMetadata, SchemaDescriptorHeader, SchemaRegistry,
    SchemaRegistryAction, SchemaRegistryFailure, SessionMigrateAckMetadata, SessionMigrateMetadata,
    SessionOpenAckMetadata, SessionOpenMetadata,
    SessionRecoveryOutcome as CoreSessionRecoveryOutcome, SupersedeMetadata, TraceContextMetadata,
    TransportId, TypedPayloadDescriptor, OBJECT_DELTA_METADATA_LEN, SESSION_ERROR_NONE,
    SESSION_ERROR_PROFILE_UNSUPPORTED, SESSION_ERROR_RESUME_REJECTED,
    SESSION_ERROR_SCHEMA_UNSUPPORTED, SESSION_FLAG_ALLOW_RESUME,
};
#[cfg(not(test))]
use nnrp_core::{
    FrameSubmitMetadata, ResultPushMetadata, FRAME_SUBMIT_METADATA_LEN, RESULT_PUSH_METADATA_LEN,
};
#[cfg(not(test))]
use nnrp_runtime::{
    BoxedFramedTransport, NnrpClient, NnrpClientConfig, NnrpClientEvent, NnrpClientSession,
    NnrpServer, NnrpServerConfig, NnrpServerEvent, NnrpServerSession, RuntimeTransportKind,
};
#[cfg(not(test))]
use tokio::sync::Mutex as AsyncMutex;

mod transport;
mod transport_exports;
mod sdk_version {
    include!(concat!(env!("OUT_DIR"), "/sdk_version.rs"));
}
pub use transport::*;

use sdk_version::{SDK_MAJOR, SDK_MINOR, SDK_PATCH, SDK_PREVIEW, SDK_REVISION};

pub const NNRP_FFI_ABI_MAJOR: u16 = 3;
pub const NNRP_FFI_ABI_MINOR: u16 = 0;
pub const NNRP_FFI_ABI_PATCH: u16 = 0;

pub const NNRP_TRANSPORT_SLOT_QUIC: u32 = 0x0000_0001;
pub const NNRP_TRANSPORT_SLOT_TCP: u32 = 0x0000_0002;
pub const NNRP_TRANSPORT_SLOT_IPC: u32 = 0x0000_0004;
pub const NNRP_TRANSPORT_SLOT_WEBSOCKET: u32 = 0x0000_0008;

#[cfg(not(any(
    feature = "transport-tcp",
    feature = "transport-quic",
    feature = "transport-ipc",
    feature = "transport-websocket"
)))]
compile_error!("nnrp-ffi must be built with at least one transport feature enabled.");

pub const NNRP_RUNTIME_FEATURE_PROTOCOL_CORE: u64 = 0x0000_0000_0000_0001;
pub const NNRP_RUNTIME_FEATURE_CLIENT_API: u64 = 0x0000_0000_0000_0002;
pub const NNRP_RUNTIME_FEATURE_SERVER_API: u64 = 0x0000_0000_0000_0004;
pub const NNRP_RUNTIME_FEATURE_EVENT_POLLING: u64 = 0x0000_0000_0000_0008;
pub const NNRP_RUNTIME_FEATURE_CALLBACK_DISPATCH: u64 = 0x0000_0000_0000_0010;
pub const NNRP_RUNTIME_FEATURE_CACHE_SCHEMA: u64 = 0x0000_0000_0000_0020;
pub const NNRP_RUNTIME_FEATURE_RECOVERY: u64 = 0x0000_0000_0000_0040;
pub const NNRP_RUNTIME_FEATURE_TYPED_PAYLOAD: u64 = 0x0000_0000_0000_0080;
pub const NNRP_RUNTIME_FEATURE_TRANSPORT_SLOTS: u64 = 0x0000_0000_0000_0100;
pub const NNRP_RUNTIME_FEATURE_BATCH_POLLING: u64 = 0x0000_0000_0000_0200;
pub const NNRP_RUNTIME_FEATURE_CACHE_LEASE_OPS: u64 = 0x0000_0000_0000_0400;
pub const NNRP_RUNTIME_FEATURE_SCHEMA_REGISTRY_HANDLES: u64 = 0x0000_0000_0000_0800;
pub const NNRP_RUNTIME_FEATURE_BUFFER_HANDLES: u64 = 0x0000_0000_0000_1000;
pub const NNRP_RUNTIME_FEATURE_EXECUTABLE_RESUME: u64 = 0x0000_0000_0000_2000;
pub const NNRP_RUNTIME_FEATURE_PREVIEW4_CONTROL_EVENTS: u64 = 0x0000_0000_0002_0000;
pub const NNRP_RUNTIME_FEATURE_PREVIEW4_OBJECT_CACHE_EVENTS: u64 = 0x0000_0000_0004_0000;
pub const NNRP_RUNTIME_FEATURE_PREVIEW4_RUNTIME_FRAME_SEND: u64 = 0x0000_0000_0008_0000;
pub const NNRP_RUNTIME_FEATURE_TRANSPORT_FRAMED_IO: u64 = 0x0000_0000_0010_0000;

pub const NNRP_RESULT_STATE_NONE: u32 = 0;
pub const NNRP_RESULT_STATE_COMPLETED: u32 = 1;
pub const NNRP_RESULT_STATE_PARTIAL: u32 = 2;
pub const NNRP_RESULT_STATE_DEGRADED: u32 = 3;
pub const NNRP_RESULT_STATE_STALE_REUSE: u32 = 4;
pub const NNRP_RESULT_STATE_CANCELLED: u32 = 5;
pub const NNRP_RESULT_STATE_FAILED: u32 = 6;

pub const NNRP_SESSION_RECOVERY_OUTCOME_FRESH: u32 = 0;
pub const NNRP_SESSION_RECOVERY_OUTCOME_RESUME_ENABLED: u32 = 1;
pub const NNRP_SESSION_RECOVERY_OUTCOME_RESUMED: u32 = 2;
pub const NNRP_SESSION_RECOVERY_OUTCOME_RESUME_REJECTED: u32 = 3;

pub const NNRP_SCHEMA_REGISTRY_ACTION_INSTALLED: u32 = 0;
pub const NNRP_SCHEMA_REGISTRY_ACTION_ALREADY_INSTALLED: u32 = 1;
pub const NNRP_SCHEMA_REGISTRY_ACTION_UPDATED: u32 = 2;
pub const NNRP_SCHEMA_REGISTRY_ACTION_INVALIDATED: u32 = 3;

pub const NNRP_CACHE_LEASE_OUTCOME_VALID: u32 = 0;
pub const NNRP_CACHE_LEASE_OUTCOME_MISS: u32 = 1;
pub const NNRP_CACHE_LEASE_OUTCOME_EXPIRED: u32 = 2;
pub const NNRP_CACHE_LEASE_OUTCOME_RELEASED: u32 = 3;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpProtocolVersion {
    pub major: u8,
    pub wire_format: u8,
}

impl From<ProtocolVersion> for NnrpProtocolVersion {
    fn from(value: ProtocolVersion) -> Self {
        Self {
            major: value.major,
            wire_format: value.wire_format,
        }
    }
}

pub fn current_protocol_version() -> NnrpProtocolVersion {
    ProtocolVersion::CURRENT.into()
}

#[no_mangle]
pub extern "C" fn nnrp_current_protocol_version() -> NnrpProtocolVersion {
    current_protocol_version()
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpRuntimeCapabilities {
    pub abi_major: u16,
    pub abi_minor: u16,
    pub abi_patch: u16,
    pub reserved0: u16,
    pub protocol_version: NnrpProtocolVersion,
    pub sdk_major: u16,
    pub sdk_minor: u16,
    pub sdk_patch: u16,
    pub sdk_preview: u16,
    pub sdk_revision: u16,
    pub reserved1: u16,
    pub transport_slots: u32,
    pub feature_flags: u64,
}

pub fn runtime_capabilities() -> NnrpRuntimeCapabilities {
    NnrpRuntimeCapabilities {
        abi_major: NNRP_FFI_ABI_MAJOR,
        abi_minor: NNRP_FFI_ABI_MINOR,
        abi_patch: NNRP_FFI_ABI_PATCH,
        reserved0: 0,
        protocol_version: current_protocol_version(),
        sdk_major: SDK_MAJOR,
        sdk_minor: SDK_MINOR,
        sdk_patch: SDK_PATCH,
        sdk_preview: SDK_PREVIEW,
        sdk_revision: SDK_REVISION,
        reserved1: 0,
        transport_slots: enabled_transport_slots(),
        feature_flags: NNRP_RUNTIME_FEATURE_PROTOCOL_CORE
            | NNRP_RUNTIME_FEATURE_CLIENT_API
            | NNRP_RUNTIME_FEATURE_SERVER_API
            | NNRP_RUNTIME_FEATURE_EVENT_POLLING
            | NNRP_RUNTIME_FEATURE_CALLBACK_DISPATCH
            | NNRP_RUNTIME_FEATURE_CACHE_SCHEMA
            | NNRP_RUNTIME_FEATURE_RECOVERY
            | NNRP_RUNTIME_FEATURE_TYPED_PAYLOAD
            | NNRP_RUNTIME_FEATURE_TRANSPORT_SLOTS
            | NNRP_RUNTIME_FEATURE_BATCH_POLLING
            | NNRP_RUNTIME_FEATURE_CACHE_LEASE_OPS
            | NNRP_RUNTIME_FEATURE_SCHEMA_REGISTRY_HANDLES
            | NNRP_RUNTIME_FEATURE_BUFFER_HANDLES
            | NNRP_RUNTIME_FEATURE_EXECUTABLE_RESUME
            | NNRP_RUNTIME_FEATURE_PREVIEW4_CONTROL_EVENTS
            | NNRP_RUNTIME_FEATURE_PREVIEW4_OBJECT_CACHE_EVENTS
            | NNRP_RUNTIME_FEATURE_PREVIEW4_RUNTIME_FRAME_SEND
            | NNRP_RUNTIME_FEATURE_TRANSPORT_FRAMED_IO,
    }
}

const fn transport_slot_bit(transport_id: TransportId) -> u32 {
    match transport_id {
        TransportId::Quic => NNRP_TRANSPORT_SLOT_QUIC,
        TransportId::Tcp => NNRP_TRANSPORT_SLOT_TCP,
        TransportId::Ipc => NNRP_TRANSPORT_SLOT_IPC,
        TransportId::WebSocket => NNRP_TRANSPORT_SLOT_WEBSOCKET,
        TransportId::Unspecified => 0,
    }
}

#[no_mangle]
pub extern "C" fn nnrp_runtime_capabilities() -> NnrpRuntimeCapabilities {
    runtime_capabilities()
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NnrpFfiStatusCode {
    Ok = 0,
    InvalidArgument = 1,
    InvalidHandle = 2,
    InvalidState = 3,
    ProtocolError = 4,
    WouldBlock = 5,
    CallbackRejected = 6,
    InternalError = 0xffff,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NnrpErrorFamily {
    None = 0,
    Session = 1,
    Cache = 2,
    Schema = 3,
    Transport = 4,
    Lifecycle = 5,
    Operation = 6,
    Control = 7,
    RuntimeObject = 8,
    Internal = 0xffff,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpFfiStatus {
    pub status_code: u32,
    pub error_family: u32,
    pub protocol_error_code: u32,
    pub detail_code: u32,
}

impl NnrpFfiStatus {
    pub const fn ok() -> Self {
        Self {
            status_code: NnrpFfiStatusCode::Ok as u32,
            error_family: NnrpErrorFamily::None as u32,
            protocol_error_code: 0,
            detail_code: 0,
        }
    }

    pub const fn invalid_argument(detail_code: u32) -> Self {
        Self {
            status_code: NnrpFfiStatusCode::InvalidArgument as u32,
            error_family: NnrpErrorFamily::None as u32,
            protocol_error_code: 0,
            detail_code,
        }
    }

    pub const fn invalid_handle(detail_code: u32) -> Self {
        Self {
            status_code: NnrpFfiStatusCode::InvalidHandle as u32,
            error_family: NnrpErrorFamily::Lifecycle as u32,
            protocol_error_code: 0,
            detail_code,
        }
    }

    pub const fn invalid_state(detail_code: u32) -> Self {
        Self {
            status_code: NnrpFfiStatusCode::InvalidState as u32,
            error_family: NnrpErrorFamily::Lifecycle as u32,
            protocol_error_code: 0,
            detail_code,
        }
    }

    pub const fn protocol(error_family: NnrpErrorFamily, protocol_error_code: u32) -> Self {
        Self {
            status_code: NnrpFfiStatusCode::ProtocolError as u32,
            error_family: error_family as u32,
            protocol_error_code,
            detail_code: 0,
        }
    }

    pub fn from_core_error(error: &NnrpError) -> Self {
        match error {
            NnrpError::InvalidProtocolCombination { .. }
            | NnrpError::InvalidOperationRelationship { .. }
            | NnrpError::InvalidOperationTransition { .. } => Self {
                status_code: NnrpFfiStatusCode::ProtocolError as u32,
                error_family: NnrpErrorFamily::Lifecycle as u32,
                protocol_error_code: 0,
                detail_code: 0,
            },
            NnrpError::UnknownEnumValue { enum_name, .. } => Self::protocol(
                ffi_error_family_for_enum(enum_name).unwrap_or(NnrpErrorFamily::Transport),
                0,
            ),
            NnrpError::NonZeroReservedField { field } => Self::protocol(
                ffi_error_family_for_named_field(field).unwrap_or(NnrpErrorFamily::Transport),
                0,
            ),
            NnrpError::DeclaredLengthMismatch { field, .. } => Self::protocol(
                ffi_error_family_for_named_field(field).unwrap_or(NnrpErrorFamily::Transport),
                0,
            ),
            NnrpError::ReservedBitsSet { .. }
            | NnrpError::UnsupportedWireFormat(_)
            | NnrpError::UnsupportedVersionMajor(_)
            | NnrpError::UnknownMessageType(_)
            | NnrpError::InvalidMagic
            | NnrpError::InvalidHeaderLength(_)
            | NnrpError::PacketLengthMismatch { .. }
            | NnrpError::MessageLengthOverflow => Self {
                status_code: NnrpFfiStatusCode::ProtocolError as u32,
                error_family: NnrpErrorFamily::Transport as u32,
                protocol_error_code: 0,
                detail_code: 0,
            },
            NnrpError::SourceTooShort { .. } | NnrpError::DestinationTooShort { .. } => {
                Self::invalid_argument(0)
            }
            NnrpError::ConnectionNotOpen
            | NnrpError::ConnectionAlreadyClosed
            | NnrpError::SessionAlreadyExists(_)
            | NnrpError::UnknownSession(_)
            | NnrpError::SessionNotOpen(_)
            | NnrpError::OperationAlreadyExists(_)
            | NnrpError::UnknownOperation(_) => Self {
                status_code: NnrpFfiStatusCode::InvalidState as u32,
                error_family: NnrpErrorFamily::Lifecycle as u32,
                protocol_error_code: 0,
                detail_code: 0,
            },
        }
    }
}

fn ffi_error_family_for_enum(enum_name: &str) -> Option<NnrpErrorFamily> {
    match enum_name {
        "result_hint_budget_policy"
        | "result_hint_congestion_state"
        | "result_hint_reason"
        | "session_patch_ack_status"
        | "session_patch_reject_reason"
        | "error_scope"
        | "pressure_level" => Some(NnrpErrorFamily::Control),
        "runtime_object_kind"
        | "runtime_role"
        | "memory_location_hint"
        | "ownership_hint"
        | "object_release_reason"
        | "cache_reuse_scope"
        | "cache_miss_reason" => Some(NnrpErrorFamily::RuntimeObject),
        "cache_object_kind" | "cache_ack_status" | "cache_invalidate_scope" => {
            Some(NnrpErrorFamily::Cache)
        }
        "transport_id" => Some(NnrpErrorFamily::Transport),
        _ => None,
    }
}

fn ffi_error_family_for_named_field(field: &str) -> Option<NnrpErrorFamily> {
    if field.starts_with("control_")
        || field.starts_with("result_hint")
        || field.starts_with("session_patch")
        || field.starts_with("scheduling")
        || field.starts_with("partial_result")
        || field.starts_with("pressure")
        || field.starts_with("capability")
        || field.starts_with("route_hint")
        || field.starts_with("trace_context")
        || field.starts_with("drop_reason")
        || field.starts_with("supersede")
    {
        return Some(NnrpErrorFamily::Control);
    }
    if field.starts_with("object_")
        || field.starts_with("cache_reference")
        || field.starts_with("cache_miss")
    {
        return Some(NnrpErrorFamily::RuntimeObject);
    }
    if field.starts_with("cache_") {
        return Some(NnrpErrorFamily::Cache);
    }
    None
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpFfiDiagnostic {
    pub status: NnrpFfiStatus,
    pub related_connection_id: u64,
    pub related_session_id: u32,
    pub related_operation_id: u64,
    pub related_frame_id: u32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NnrpHandleKind {
    Invalid = 0,
    Connection = 1,
    Session = 2,
    Operation = 3,
    EventPump = 4,
    Buffer = 5,
    SchemaRegistry = 6,
    CacheLease = 7,
    ObjectDescriptor = 8,
    CacheReferenceDescriptor = 9,
    TransportConnection = 10,
    TransportListener = 11,
    TransportSecurityConfig = 12,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpHandle {
    pub kind: u32,
    pub id: u64,
    pub generation: u32,
    pub flags: u32,
}

impl NnrpHandle {
    pub const fn invalid() -> Self {
        Self {
            kind: NnrpHandleKind::Invalid as u32,
            id: 0,
            generation: 0,
            flags: 0,
        }
    }

    pub const fn new(kind: NnrpHandleKind, id: u64, generation: u32) -> Self {
        Self {
            kind: kind as u32,
            id,
            generation,
            flags: 0,
        }
    }

    pub fn validate_kind(self, kind: NnrpHandleKind) -> Result<(), NnrpFfiStatus> {
        if self.kind != kind as u32 || self.id == 0 || self.generation == 0 {
            return Err(NnrpFfiStatus::invalid_handle(kind as u32));
        }

        Ok(())
    }
}

#[allow(dead_code)]
enum NnrpFfiResource {
    Connection {
        transport_id: u32,
        role: NnrpFfiConnectionRole,
        #[cfg(not(test))]
        runtime: NnrpFfiConnectionRuntime,
    },
    Session {
        connection: NnrpHandle,
        profile_id: u16,
        schema_id: u32,
        schema_version: u32,
        #[cfg(not(test))]
        runtime: NnrpFfiSessionRuntime,
    },
    Operation {
        session: NnrpHandle,
        operation_id: u64,
        frame_id: u32,
        payload_len: usize,
    },
    SchemaRegistry {
        registry: SchemaRegistry,
    },
    Buffer {
        bytes: Vec<u8>,
    },
    ObjectDescriptor {
        descriptor: ObjectDescriptorMetadata,
        metadata: Vec<u8>,
    },
    CacheReferenceDescriptor {
        descriptor: CacheReferenceMetadata,
        metadata: Vec<u8>,
    },
    CacheLease {
        owner: NnrpHandle,
        lease: CacheLease,
        released: bool,
    },
}

#[cfg(not(test))]
enum NnrpFfiConnectionRuntime {
    Client {
        carrier: Option<BoxedFramedTransport>,
    },
    Server(Arc<NnrpServer>),
}

#[cfg(not(test))]
#[allow(dead_code)]
enum NnrpFfiSessionRuntime {
    Client(Arc<AsyncMutex<NnrpClientSession>>),
    Server(Arc<AsyncMutex<NnrpServerSession>>),
}

#[cfg(not(test))]
enum NnrpFfiRoleSession {
    Client(Arc<AsyncMutex<NnrpClientSession>>),
    Server(Arc<AsyncMutex<NnrpServerSession>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NnrpFfiConnectionRole {
    Client,
    Server,
}

struct NnrpFfiResourceEntry {
    generation: u32,
    resource: NnrpFfiResource,
}

#[derive(Default)]
struct NnrpFfiHandleStore {
    entries: BTreeMap<(u32, u64), NnrpFfiResourceEntry>,
    next_ids: BTreeMap<u32, u64>,
    #[cfg(any(test, feature = "benchmark-ffi"))]
    events: VecDeque<NnrpQueuedEvent>,
}

#[cfg(any(test, feature = "benchmark-ffi"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NnrpQueuedEvent {
    kind: u32,
    message_type: u32,
    connection: NnrpHandle,
    session: NnrpHandle,
    operation: NnrpHandle,
    frame_id: u32,
    payload_owner: NnrpHandle,
}

#[cfg(any(test, feature = "benchmark-ffi"))]
impl NnrpQueuedEvent {
    const fn plain(
        kind: NnrpEventKind,
        connection: NnrpHandle,
        session: NnrpHandle,
        operation: NnrpHandle,
        frame_id: u32,
    ) -> Self {
        Self {
            kind: kind as u32,
            message_type: 0,
            connection,
            session,
            operation,
            frame_id,
            payload_owner: NnrpHandle::invalid(),
        }
    }

    fn into_event(self, payload: NnrpBufferView) -> NnrpEvent {
        NnrpEvent {
            kind: self.kind,
            message_type: self.message_type,
            connection: self.connection,
            session: self.session,
            operation: self.operation,
            frame_id: self.frame_id,
            payload_owner: self.payload_owner,
            payload,
            ..NnrpEvent::none()
        }
    }
}

impl NnrpFfiHandleStore {
    fn insert(
        &mut self,
        handle: NnrpHandle,
        resource: NnrpFfiResource,
    ) -> Result<(), NnrpFfiStatus> {
        handle.validate_kind(match handle.kind {
            value if value == NnrpHandleKind::Connection as u32 => NnrpHandleKind::Connection,
            value if value == NnrpHandleKind::Session as u32 => NnrpHandleKind::Session,
            value if value == NnrpHandleKind::Operation as u32 => NnrpHandleKind::Operation,
            value if value == NnrpHandleKind::Buffer as u32 => NnrpHandleKind::Buffer,
            value if value == NnrpHandleKind::SchemaRegistry as u32 => {
                NnrpHandleKind::SchemaRegistry
            }
            value if value == NnrpHandleKind::CacheLease as u32 => NnrpHandleKind::CacheLease,
            value if value == NnrpHandleKind::ObjectDescriptor as u32 => {
                NnrpHandleKind::ObjectDescriptor
            }
            value if value == NnrpHandleKind::CacheReferenceDescriptor as u32 => {
                NnrpHandleKind::CacheReferenceDescriptor
            }
            _ => return Err(NnrpFfiStatus::invalid_handle(handle.kind)),
        })?;
        self.entries.insert(
            (handle.kind, handle.id),
            NnrpFfiResourceEntry {
                generation: handle.generation,
                resource,
            },
        );
        Ok(())
    }

    fn get(
        &self,
        handle: NnrpHandle,
        kind: NnrpHandleKind,
    ) -> Result<&NnrpFfiResource, NnrpFfiStatus> {
        handle.validate_kind(kind)?;
        let Some(entry) = self.entries.get(&(handle.kind, handle.id)) else {
            return Err(NnrpFfiStatus::invalid_handle(kind as u32));
        };
        if entry.generation != handle.generation {
            return Err(NnrpFfiStatus::invalid_handle(kind as u32));
        }
        Ok(&entry.resource)
    }

    fn remove(&mut self, handle: NnrpHandle, kind: NnrpHandleKind) -> Result<(), NnrpFfiStatus> {
        self.get(handle, kind)?;
        self.entries.remove(&(handle.kind, handle.id));
        Ok(())
    }

    fn get_mut(
        &mut self,
        handle: NnrpHandle,
        kind: NnrpHandleKind,
    ) -> Result<&mut NnrpFfiResource, NnrpFfiStatus> {
        handle.validate_kind(kind)?;
        let Some(entry) = self.entries.get_mut(&(handle.kind, handle.id)) else {
            return Err(NnrpFfiStatus::invalid_handle(kind as u32));
        };
        if entry.generation != handle.generation {
            return Err(NnrpFfiStatus::invalid_handle(kind as u32));
        }
        Ok(&mut entry.resource)
    }

    fn close_connection(&mut self, connection: NnrpHandle) -> Result<(), NnrpFfiStatus> {
        self.get(connection, NnrpHandleKind::Connection)?;

        let sessions: Vec<NnrpHandle> = self
            .entries
            .iter()
            .filter_map(|((kind, id), entry)| match &entry.resource {
                NnrpFfiResource::Session {
                    connection: owner, ..
                } if *owner == connection => Some(NnrpHandle {
                    kind: *kind,
                    id: *id,
                    generation: entry.generation,
                    flags: 0,
                }),
                _ => None,
            })
            .collect();
        let operations: Vec<NnrpHandle> = self
            .entries
            .iter()
            .filter_map(|((kind, id), entry)| match &entry.resource {
                NnrpFfiResource::Operation { session, .. }
                    if sessions.iter().any(|owned| owned == session) =>
                {
                    Some(NnrpHandle {
                        kind: *kind,
                        id: *id,
                        generation: entry.generation,
                        flags: 0,
                    })
                }
                _ => None,
            })
            .collect();

        for operation in &operations {
            self.entries.remove(&(operation.kind, operation.id));
        }
        for session in &sessions {
            self.entries.remove(&(session.kind, session.id));
        }
        self.entries.retain(|_, entry| match &entry.resource {
            NnrpFfiResource::CacheLease { owner, .. } => {
                *owner != connection
                    && !sessions.iter().any(|session| session == owner)
                    && !operations.iter().any(|operation| operation == owner)
            }
            _ => true,
        });
        self.entries.remove(&(connection.kind, connection.id));
        #[cfg(any(test, feature = "benchmark-ffi"))]
        {
            let payload_owners: Vec<NnrpHandle> = self
                .events
                .iter()
                .filter_map(|event| {
                    (event.connection == connection
                        && event.payload_owner.kind == NnrpHandleKind::Buffer as u32)
                        .then_some(event.payload_owner)
                })
                .collect();
            self.events.retain(|event| event.connection != connection);
            for owner in payload_owners {
                self.entries.remove(&(owner.kind, owner.id));
            }
        }
        Ok(())
    }

    fn close_session(&mut self, session: NnrpHandle) -> Result<(), NnrpFfiStatus> {
        self.get(session, NnrpHandleKind::Session)?;

        let operations: Vec<NnrpHandle> = self
            .entries
            .iter()
            .filter_map(|((kind, id), entry)| match &entry.resource {
                NnrpFfiResource::Operation { session: owner, .. } if *owner == session => {
                    Some(NnrpHandle {
                        kind: *kind,
                        id: *id,
                        generation: entry.generation,
                        flags: 0,
                    })
                }
                _ => None,
            })
            .collect();

        for operation in &operations {
            self.entries.remove(&(operation.kind, operation.id));
        }
        self.entries.retain(|_, entry| match &entry.resource {
            NnrpFfiResource::CacheLease { owner, .. } => {
                *owner != session && !operations.iter().any(|operation| operation == owner)
            }
            _ => true,
        });
        self.entries.remove(&(session.kind, session.id));
        #[cfg(any(test, feature = "benchmark-ffi"))]
        {
            let payload_owners: Vec<NnrpHandle> = self
                .events
                .iter()
                .filter_map(|event| {
                    (event.session == session
                        && event.payload_owner.kind == NnrpHandleKind::Buffer as u32)
                        .then_some(event.payload_owner)
                })
                .collect();
            self.events.retain(|event| event.session != session);
            for owner in payload_owners {
                self.entries.remove(&(owner.kind, owner.id));
            }
        }
        Ok(())
    }

    #[cfg(any(test, feature = "benchmark-ffi"))]
    fn push_event(&mut self, event: NnrpQueuedEvent) {
        self.events.push_back(event);
    }

    #[cfg(any(test, feature = "benchmark-ffi"))]
    fn poll_event(&mut self, connection: NnrpHandle) -> Result<Option<NnrpEvent>, NnrpFfiStatus> {
        self.get(connection, NnrpHandleKind::Connection)?;
        let Some(index) = self
            .events
            .iter()
            .position(|event| event.connection == connection)
        else {
            return Ok(None);
        };
        let Some(event) = self.events.remove(index) else {
            return Ok(None);
        };
        let payload = if event.payload_owner.kind == NnrpHandleKind::Buffer as u32 {
            match self.get(event.payload_owner, NnrpHandleKind::Buffer)? {
                NnrpFfiResource::Buffer { bytes } => NnrpBufferView {
                    ptr: bytes.as_ptr(),
                    len: bytes.len(),
                },
                _ => return Err(NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)),
            }
        } else {
            NnrpBufferView::empty()
        };
        Ok(Some(event.into_event(payload)))
    }

    fn get_connection_role(
        &self,
        connection: NnrpHandle,
    ) -> Result<NnrpFfiConnectionRole, NnrpFfiStatus> {
        match self.get(connection, NnrpHandleKind::Connection)? {
            NnrpFfiResource::Connection { role, .. } => Ok(*role),
            _ => Err(NnrpFfiStatus::invalid_handle(
                NnrpHandleKind::Connection as u32,
            )),
        }
    }
}

static HANDLE_STORE: OnceLock<Mutex<NnrpFfiHandleStore>> = OnceLock::new();

fn handle_store() -> MutexGuard<'static, NnrpFfiHandleStore> {
    HANDLE_STORE
        .get_or_init(|| Mutex::new(NnrpFfiHandleStore::default()))
        .lock()
        .expect("FFI handle store lock should not be poisoned")
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpBufferView {
    pub ptr: *const u8,
    pub len: usize,
}

impl NnrpBufferView {
    pub const fn empty() -> Self {
        Self {
            ptr: core::ptr::null(),
            len: 0,
        }
    }

    pub fn validate(self) -> Result<(), NnrpFfiStatus> {
        if self.len > 0 && self.ptr.is_null() {
            return Err(NnrpFfiStatus::invalid_argument(1));
        }

        Ok(())
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpBufferViewMut {
    pub ptr: *mut u8,
    pub len: usize,
}

impl NnrpBufferViewMut {
    pub fn validate(self) -> Result<(), NnrpFfiStatus> {
        if self.len > 0 && self.ptr.is_null() {
            return Err(NnrpFfiStatus::invalid_argument(2));
        }

        Ok(())
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpSchemaDescriptorHeader {
    pub schema_id: u32,
    pub schema_version: u32,
    pub profile_id: u16,
    pub schema_flags: u16,
    pub min_version_major: u8,
    pub max_version_major: u8,
    pub reserved0: u16,
    pub body_bytes: u32,
    pub dependency_count: u16,
    pub default_stream_semantics: u16,
    pub schema_hash: u64,
}

impl From<SchemaDescriptorHeader> for NnrpSchemaDescriptorHeader {
    fn from(value: SchemaDescriptorHeader) -> Self {
        Self {
            schema_id: value.schema_id,
            schema_version: value.schema_version,
            profile_id: value.profile_id,
            schema_flags: value.schema_flags,
            min_version_major: value.min_version_major,
            max_version_major: value.max_version_major,
            reserved0: 0,
            body_bytes: value.body_bytes,
            dependency_count: value.dependency_count,
            default_stream_semantics: value.default_stream_semantics,
            schema_hash: value.schema_hash,
        }
    }
}

impl From<NnrpSchemaDescriptorHeader> for SchemaDescriptorHeader {
    fn from(value: NnrpSchemaDescriptorHeader) -> Self {
        Self {
            schema_id: value.schema_id,
            schema_version: value.schema_version,
            profile_id: value.profile_id,
            schema_flags: value.schema_flags,
            min_version_major: value.min_version_major,
            max_version_major: value.max_version_major,
            body_bytes: value.body_bytes,
            dependency_count: value.dependency_count,
            default_stream_semantics: value.default_stream_semantics,
            schema_hash: value.schema_hash,
        }
    }
}

impl NnrpSchemaDescriptorHeader {
    fn to_core(self) -> Result<SchemaDescriptorHeader, NnrpFfiStatus> {
        if self.reserved0 != 0 {
            return Err(NnrpFfiStatus::from_core_error(
                &NnrpError::NonZeroReservedField {
                    field: "schema_descriptor.reserved0",
                },
            ));
        }

        Ok(self.into())
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpTypedPayloadDescriptor {
    pub profile_id: u16,
    pub descriptor_flags: u16,
    pub schema_id: u32,
    pub schema_version: u32,
    pub stream_semantics: u16,
    pub reserved0: u16,
    pub offset: u32,
    pub length: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpCacheObjectId {
    pub cache_namespace: u32,
    pub object_kind: u32,
    pub cache_key_hi: u64,
    pub cache_key_lo: u64,
}

impl NnrpCacheObjectId {
    fn to_core(self) -> Result<CacheObjectId, NnrpFfiStatus> {
        Ok(CacheObjectId {
            cache_namespace: self.cache_namespace,
            cache_key_hi: self.cache_key_hi,
            cache_key_lo: self.cache_key_lo,
            object_kind: CacheObjectKind::try_from_u32(self.object_kind)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
        })
    }
}

impl From<CacheObjectId> for NnrpCacheObjectId {
    fn from(value: CacheObjectId) -> Self {
        Self {
            cache_namespace: value.cache_namespace,
            cache_key_hi: value.cache_key_hi,
            cache_key_lo: value.cache_key_lo,
            object_kind: value.object_kind as u32,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpCacheLeaseRequest {
    pub owner: NnrpHandle,
    pub object_id: NnrpCacheObjectId,
    pub expected_version: u64,
    pub now_ms: u64,
    pub ttl_ms: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpCacheLeaseResult {
    pub outcome_code: u32,
    pub lease_handle: NnrpHandle,
    pub object_id: NnrpCacheObjectId,
    pub object_version: u64,
    pub lease_id: u64,
    pub expires_at_ms: u64,
}

impl NnrpCacheLeaseResult {
    fn miss(object_id: CacheObjectId) -> Self {
        Self {
            outcome_code: NNRP_CACHE_LEASE_OUTCOME_MISS,
            lease_handle: NnrpHandle::invalid(),
            object_id: object_id.into(),
            object_version: 0,
            lease_id: 0,
            expires_at_ms: 0,
        }
    }

    fn from_lease(outcome_code: u32, lease_handle: NnrpHandle, lease: CacheLease) -> Self {
        Self {
            outcome_code,
            lease_handle,
            object_id: lease.object_id.into(),
            object_version: lease.object_version,
            lease_id: lease.lease_id,
            expires_at_ms: lease.expires_at_ms(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpRuntimeObjectDescriptor {
    pub object_id: u64,
    pub object_kind: u16,
    pub producer_role: u8,
    pub consumer_role: u8,
    pub session_id: u32,
    pub byte_size: u64,
    pub compute_cost_units: u32,
    pub memory_location_hint: u16,
    pub ownership_hint: u16,
    pub lifetime_hint_ms: u32,
    pub metadata_bytes: u32,
}

impl NnrpRuntimeObjectDescriptor {
    pub fn to_core(self) -> Result<ObjectDescriptorMetadata, NnrpFfiStatus> {
        Ok(ObjectDescriptorMetadata {
            object_id: self.object_id,
            object_kind: RuntimeObjectKind::try_from_u16(self.object_kind)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            producer_role: RuntimeRole::try_from_u8(self.producer_role)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            consumer_role: RuntimeRole::try_from_u8(self.consumer_role)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            session_id: self.session_id,
            byte_size: self.byte_size,
            compute_cost_units: self.compute_cost_units,
            memory_location_hint: MemoryLocationHint::try_from_u16(self.memory_location_hint)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            ownership_hint: OwnershipHint::try_from_u16(self.ownership_hint)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            lifetime_hint_ms: self.lifetime_hint_ms,
            metadata_bytes: self.metadata_bytes,
        })
    }
}

impl From<ObjectDescriptorMetadata> for NnrpRuntimeObjectDescriptor {
    fn from(value: ObjectDescriptorMetadata) -> Self {
        Self {
            object_id: value.object_id,
            object_kind: value.object_kind as u16,
            producer_role: value.producer_role as u8,
            consumer_role: value.consumer_role as u8,
            session_id: value.session_id,
            byte_size: value.byte_size,
            compute_cost_units: value.compute_cost_units,
            memory_location_hint: value.memory_location_hint as u16,
            ownership_hint: value.ownership_hint as u16,
            lifetime_hint_ms: value.lifetime_hint_ms,
            metadata_bytes: value.metadata_bytes,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpObjectReferenceDescriptor {
    pub object_id: u64,
    pub operation_id: u64,
    pub object_version: u64,
    pub offset: u64,
    pub length: u64,
    pub flags: u32,
    pub metadata_bytes: u32,
}

impl From<ObjectReferenceMetadata> for NnrpObjectReferenceDescriptor {
    fn from(value: ObjectReferenceMetadata) -> Self {
        Self {
            object_id: value.object_id,
            operation_id: value.operation_id,
            object_version: value.object_version,
            offset: value.offset,
            length: value.length,
            flags: value.flags,
            metadata_bytes: value.metadata_bytes,
        }
    }
}

impl From<NnrpObjectReferenceDescriptor> for ObjectReferenceMetadata {
    fn from(value: NnrpObjectReferenceDescriptor) -> Self {
        Self {
            object_id: value.object_id,
            operation_id: value.operation_id,
            object_version: value.object_version,
            offset: value.offset,
            length: value.length,
            flags: value.flags,
            metadata_bytes: value.metadata_bytes,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpObjectReleaseDescriptor {
    pub object_id: u64,
    pub operation_id: u64,
    pub release_reason: u16,
    pub source_role: u8,
    pub flags: u8,
    pub diagnostic_bytes: u32,
}

impl NnrpObjectReleaseDescriptor {
    pub fn to_core(self) -> Result<ObjectReleaseMetadata, NnrpFfiStatus> {
        Ok(ObjectReleaseMetadata {
            object_id: self.object_id,
            operation_id: self.operation_id,
            release_reason: ObjectReleaseReason::try_from_u16(self.release_reason)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            source_role: RuntimeRole::try_from_u8(self.source_role)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            flags: self.flags,
            diagnostic_bytes: self.diagnostic_bytes,
        })
    }
}

impl From<ObjectReleaseMetadata> for NnrpObjectReleaseDescriptor {
    fn from(value: ObjectReleaseMetadata) -> Self {
        Self {
            object_id: value.object_id,
            operation_id: value.operation_id,
            release_reason: value.release_reason as u16,
            source_role: value.source_role as u8,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpObjectDeltaDescriptor {
    pub object_id: u64,
    pub delta_sequence: u64,
    pub region_offset: u64,
    pub region_bytes: u32,
    pub delta_bytes: u32,
    pub flags: u32,
    pub metadata_bytes: u32,
}

impl From<ObjectDeltaMetadata> for NnrpObjectDeltaDescriptor {
    fn from(value: ObjectDeltaMetadata) -> Self {
        Self {
            object_id: value.object_id,
            delta_sequence: value.delta_sequence,
            region_offset: value.region_offset,
            region_bytes: value.region_bytes,
            delta_bytes: value.delta_bytes,
            flags: value.flags,
            metadata_bytes: value.metadata_bytes,
        }
    }
}

impl From<NnrpObjectDeltaDescriptor> for ObjectDeltaMetadata {
    fn from(value: NnrpObjectDeltaDescriptor) -> Self {
        Self {
            object_id: value.object_id,
            delta_sequence: value.delta_sequence,
            region_offset: value.region_offset,
            region_bytes: value.region_bytes,
            delta_bytes: value.delta_bytes,
            flags: value.flags,
            metadata_bytes: value.metadata_bytes,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpCacheReferenceDescriptor {
    pub cache_namespace: u32,
    pub profile_id: u16,
    pub reuse_scope: u16,
    pub cache_key_hi: u64,
    pub cache_key_lo: u64,
    pub lease_id: u64,
    pub producer_trace_id: u64,
    pub expiration_hint_ms: u32,
    pub metadata_bytes: u32,
    pub flags: u32,
}

impl NnrpCacheReferenceDescriptor {
    pub fn to_core(self) -> Result<CacheReferenceMetadata, NnrpFfiStatus> {
        Ok(CacheReferenceMetadata {
            cache_namespace: self.cache_namespace,
            cache_key_hi: self.cache_key_hi,
            cache_key_lo: self.cache_key_lo,
            profile_id: self.profile_id,
            reuse_scope: CacheReuseScope::try_from_u16(self.reuse_scope)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            lease_id: self.lease_id,
            producer_trace_id: self.producer_trace_id,
            expiration_hint_ms: self.expiration_hint_ms,
            metadata_bytes: self.metadata_bytes,
            flags: self.flags,
        })
    }
}

impl From<CacheReferenceMetadata> for NnrpCacheReferenceDescriptor {
    fn from(value: CacheReferenceMetadata) -> Self {
        Self {
            cache_namespace: value.cache_namespace,
            cache_key_hi: value.cache_key_hi,
            cache_key_lo: value.cache_key_lo,
            profile_id: value.profile_id,
            reuse_scope: value.reuse_scope as u16,
            lease_id: value.lease_id,
            producer_trace_id: value.producer_trace_id,
            expiration_hint_ms: value.expiration_hint_ms,
            metadata_bytes: value.metadata_bytes,
            flags: value.flags,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpCacheMissDescriptor {
    pub cache_namespace: u32,
    pub miss_reason: u16,
    pub profile_id: u16,
    pub cache_key_hi: u64,
    pub cache_key_lo: u64,
    pub diagnostic_bytes: u32,
}

impl NnrpCacheMissDescriptor {
    pub fn to_core(self) -> Result<CacheMissMetadata, NnrpFfiStatus> {
        Ok(CacheMissMetadata {
            cache_namespace: self.cache_namespace,
            cache_key_hi: self.cache_key_hi,
            cache_key_lo: self.cache_key_lo,
            miss_reason: CacheMissReason::try_from_u16(self.miss_reason)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            profile_id: self.profile_id,
            diagnostic_bytes: self.diagnostic_bytes,
        })
    }
}

impl From<CacheMissMetadata> for NnrpCacheMissDescriptor {
    fn from(value: CacheMissMetadata) -> Self {
        Self {
            cache_namespace: value.cache_namespace,
            cache_key_hi: value.cache_key_hi,
            cache_key_lo: value.cache_key_lo,
            miss_reason: value.miss_reason as u16,
            profile_id: value.profile_id,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpControlRequestDescriptor {
    pub operation_id: u64,
    pub control_sequence: u64,
    pub reason_code: u16,
    pub source_role: u8,
    pub flags: u8,
    pub diagnostic_bytes: u32,
}

impl From<ControlRequestMetadata> for NnrpControlRequestDescriptor {
    fn from(value: ControlRequestMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            control_sequence: value.control_sequence,
            reason_code: value.reason_code,
            source_role: value.source_role,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

impl From<NnrpControlRequestDescriptor> for ControlRequestMetadata {
    fn from(value: NnrpControlRequestDescriptor) -> Self {
        Self {
            operation_id: value.operation_id,
            control_sequence: value.control_sequence,
            reason_code: value.reason_code,
            source_role: value.source_role,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpSchedulingDescriptor {
    pub operation_id: u64,
    pub control_sequence: u64,
    pub priority_class: u16,
    pub priority_delta: i16,
    pub deadline_unix_ms: u64,
    pub flags: u32,
}

impl From<SchedulingMetadata> for NnrpSchedulingDescriptor {
    fn from(value: SchedulingMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            control_sequence: value.control_sequence,
            priority_class: value.priority_class,
            priority_delta: value.priority_delta,
            deadline_unix_ms: value.deadline_unix_ms,
            flags: value.flags,
        }
    }
}

impl From<NnrpSchedulingDescriptor> for SchedulingMetadata {
    fn from(value: NnrpSchedulingDescriptor) -> Self {
        Self {
            operation_id: value.operation_id,
            control_sequence: value.control_sequence,
            priority_class: value.priority_class,
            priority_delta: value.priority_delta,
            deadline_unix_ms: value.deadline_unix_ms,
            flags: value.flags,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpSupersedeDescriptor {
    pub old_operation_id: u64,
    pub new_operation_id: u64,
    pub control_sequence: u64,
    pub drop_reason_code: u16,
    pub flags: u16,
    pub diagnostic_bytes: u32,
}

impl From<SupersedeMetadata> for NnrpSupersedeDescriptor {
    fn from(value: SupersedeMetadata) -> Self {
        Self {
            old_operation_id: value.old_operation_id,
            new_operation_id: value.new_operation_id,
            control_sequence: value.control_sequence,
            drop_reason_code: value.drop_reason_code,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

impl From<NnrpSupersedeDescriptor> for SupersedeMetadata {
    fn from(value: NnrpSupersedeDescriptor) -> Self {
        Self {
            old_operation_id: value.old_operation_id,
            new_operation_id: value.new_operation_id,
            control_sequence: value.control_sequence,
            drop_reason_code: value.drop_reason_code,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpBudgetDescriptor {
    pub operation_id: u64,
    pub compute_budget_units: u64,
    pub memory_budget_bytes: u64,
    pub bandwidth_budget_bytes: u64,
    pub token_budget: u32,
    pub flags: u32,
}

impl From<BudgetMetadata> for NnrpBudgetDescriptor {
    fn from(value: BudgetMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            compute_budget_units: value.compute_budget_units,
            memory_budget_bytes: value.memory_budget_bytes,
            bandwidth_budget_bytes: value.bandwidth_budget_bytes,
            token_budget: value.token_budget,
            flags: value.flags,
        }
    }
}

impl From<NnrpBudgetDescriptor> for BudgetMetadata {
    fn from(value: NnrpBudgetDescriptor) -> Self {
        Self {
            operation_id: value.operation_id,
            compute_budget_units: value.compute_budget_units,
            memory_budget_bytes: value.memory_budget_bytes,
            bandwidth_budget_bytes: value.bandwidth_budget_bytes,
            token_budget: value.token_budget,
            flags: value.flags,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpProgressDescriptor {
    pub operation_id: u64,
    pub progress_sequence: u64,
    pub stage_code: u16,
    pub percent_x100: u16,
    pub object_id: u64,
    pub body_bytes: u32,
}

impl From<ProgressMetadata> for NnrpProgressDescriptor {
    fn from(value: ProgressMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            progress_sequence: value.progress_sequence,
            stage_code: value.stage_code,
            percent_x100: value.percent_x100,
            object_id: value.object_id,
            body_bytes: value.body_bytes,
        }
    }
}

impl From<NnrpProgressDescriptor> for ProgressMetadata {
    fn from(value: NnrpProgressDescriptor) -> Self {
        Self {
            operation_id: value.operation_id,
            progress_sequence: value.progress_sequence,
            stage_code: value.stage_code,
            percent_x100: value.percent_x100,
            object_id: value.object_id,
            body_bytes: value.body_bytes,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpPartialResultDescriptor {
    pub operation_id: u64,
    pub result_sequence: u64,
    pub object_id: u64,
    pub delta_sequence: u64,
    pub body_bytes: u32,
    pub flags: u32,
}

impl From<PartialResultMetadata> for NnrpPartialResultDescriptor {
    fn from(value: PartialResultMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            result_sequence: value.result_sequence,
            object_id: value.object_id,
            delta_sequence: value.delta_sequence,
            body_bytes: value.body_bytes,
            flags: value.flags,
        }
    }
}

impl From<NnrpPartialResultDescriptor> for PartialResultMetadata {
    fn from(value: NnrpPartialResultDescriptor) -> Self {
        Self {
            operation_id: value.operation_id,
            result_sequence: value.result_sequence,
            object_id: value.object_id,
            delta_sequence: value.delta_sequence,
            body_bytes: value.body_bytes,
            flags: value.flags,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpPressureDescriptor {
    pub scope_id: u64,
    pub credit_window: u64,
    pub pressure_level: u16,
    pub pressure_reason: u16,
    pub retry_after_ms: u32,
    pub flags: u32,
}

impl From<PressureMetadata> for NnrpPressureDescriptor {
    fn from(value: PressureMetadata) -> Self {
        Self {
            scope_id: value.scope_id,
            credit_window: value.credit_window,
            pressure_level: value.pressure_level,
            pressure_reason: value.pressure_reason,
            retry_after_ms: value.retry_after_ms,
            flags: value.flags,
        }
    }
}

impl From<NnrpPressureDescriptor> for PressureMetadata {
    fn from(value: NnrpPressureDescriptor) -> Self {
        Self {
            scope_id: value.scope_id,
            credit_window: value.credit_window,
            pressure_level: value.pressure_level,
            pressure_reason: value.pressure_reason,
            retry_after_ms: value.retry_after_ms,
            flags: value.flags,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpCapabilityDescriptor {
    pub profile_id: u16,
    pub capability_count: u16,
    pub cost_model_id: u16,
    pub preference_rank: u16,
    pub limit_bytes: u64,
    pub limit_units: u64,
    pub body_bytes: u32,
    pub flags: u32,
}

impl From<CapabilityMetadata> for NnrpCapabilityDescriptor {
    fn from(value: CapabilityMetadata) -> Self {
        Self {
            profile_id: value.profile_id,
            capability_count: value.capability_count,
            cost_model_id: value.cost_model_id,
            preference_rank: value.preference_rank,
            limit_bytes: value.limit_bytes,
            limit_units: value.limit_units,
            body_bytes: value.body_bytes,
            flags: value.flags,
        }
    }
}

impl From<NnrpCapabilityDescriptor> for CapabilityMetadata {
    fn from(value: NnrpCapabilityDescriptor) -> Self {
        Self {
            profile_id: value.profile_id,
            capability_count: value.capability_count,
            cost_model_id: value.cost_model_id,
            preference_rank: value.preference_rank,
            limit_bytes: value.limit_bytes,
            limit_units: value.limit_units,
            body_bytes: value.body_bytes,
            flags: value.flags,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpRouteHintDescriptor {
    pub operation_id: u64,
    pub route_id: u32,
    pub executor_class: u16,
    pub affinity_class: u16,
    pub deadline_unix_ms: u64,
    pub body_bytes: u32,
    pub flags: u32,
}

impl From<RouteHintMetadata> for NnrpRouteHintDescriptor {
    fn from(value: RouteHintMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            route_id: value.route_id,
            executor_class: value.executor_class,
            affinity_class: value.affinity_class,
            deadline_unix_ms: value.deadline_unix_ms,
            body_bytes: value.body_bytes,
            flags: value.flags,
        }
    }
}

impl From<NnrpRouteHintDescriptor> for RouteHintMetadata {
    fn from(value: NnrpRouteHintDescriptor) -> Self {
        Self {
            operation_id: value.operation_id,
            route_id: value.route_id,
            executor_class: value.executor_class,
            affinity_class: value.affinity_class,
            deadline_unix_ms: value.deadline_unix_ms,
            body_bytes: value.body_bytes,
            flags: value.flags,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpTraceContextDescriptor {
    pub trace_id: u64,
    pub span_id: u64,
    pub parent_span_id: u64,
    pub stage_code: u16,
    pub flags: u16,
    pub body_bytes: u32,
}

impl From<TraceContextMetadata> for NnrpTraceContextDescriptor {
    fn from(value: TraceContextMetadata) -> Self {
        Self {
            trace_id: value.trace_id,
            span_id: value.span_id,
            parent_span_id: value.parent_span_id,
            stage_code: value.stage_code,
            flags: value.flags,
            body_bytes: value.body_bytes,
        }
    }
}

impl From<NnrpTraceContextDescriptor> for TraceContextMetadata {
    fn from(value: NnrpTraceContextDescriptor) -> Self {
        Self {
            trace_id: value.trace_id,
            span_id: value.span_id,
            parent_span_id: value.parent_span_id,
            stage_code: value.stage_code,
            flags: value.flags,
            body_bytes: value.body_bytes,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpResultDropReasonDescriptor {
    pub operation_id: u64,
    pub result_sequence: u64,
    pub drop_reason_code: u16,
    pub source_role: u8,
    pub flags: u8,
    pub diagnostic_bytes: u32,
}

impl From<ResultDropReasonMetadata> for NnrpResultDropReasonDescriptor {
    fn from(value: ResultDropReasonMetadata) -> Self {
        Self {
            operation_id: value.operation_id,
            result_sequence: value.result_sequence,
            drop_reason_code: value.drop_reason_code,
            source_role: value.source_role,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

impl From<NnrpResultDropReasonDescriptor> for ResultDropReasonMetadata {
    fn from(value: NnrpResultDropReasonDescriptor) -> Self {
        Self {
            operation_id: value.operation_id,
            result_sequence: value.result_sequence,
            drop_reason_code: value.drop_reason_code,
            source_role: value.source_role,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpRecoverableErrorDescriptor {
    pub error_code: u32,
    pub error_scope: u32,
    pub recovery_action: u16,
    pub source_role: u8,
    pub flags: u8,
    pub retry_after_ms: u32,
    pub related_session_id: u32,
    pub related_frame_id: u32,
    pub related_view_id: u32,
    pub diagnostic_bytes: u32,
}

impl NnrpRecoverableErrorDescriptor {
    pub fn to_core(self) -> Result<RecoverableErrorMetadata, NnrpFfiStatus> {
        Ok(RecoverableErrorMetadata {
            error_code: self.error_code,
            error_scope: ErrorScope::try_from_u32(self.error_scope)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            recovery_action: self.recovery_action,
            source_role: self.source_role,
            flags: self.flags,
            retry_after_ms: self.retry_after_ms,
            related_session_id: self.related_session_id,
            related_frame_id: self.related_frame_id,
            related_view_id: self.related_view_id,
            diagnostic_bytes: self.diagnostic_bytes,
        })
    }
}

impl From<RecoverableErrorMetadata> for NnrpRecoverableErrorDescriptor {
    fn from(value: RecoverableErrorMetadata) -> Self {
        Self {
            error_code: value.error_code,
            error_scope: value.error_scope as u32,
            recovery_action: value.recovery_action,
            source_role: value.source_role,
            flags: value.flags,
            retry_after_ms: value.retry_after_ms,
            related_session_id: value.related_session_id,
            related_frame_id: value.related_frame_id,
            related_view_id: value.related_view_id,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpRetryAfterDescriptor {
    pub scope_id: u64,
    pub control_sequence: u64,
    pub retry_after_ms: u32,
    pub jitter_ms: u32,
    pub reason_code: u16,
    pub source_role: u8,
    pub flags: u8,
    pub diagnostic_bytes: u32,
}

impl From<RetryAfterMetadata> for NnrpRetryAfterDescriptor {
    fn from(value: RetryAfterMetadata) -> Self {
        Self {
            scope_id: value.scope_id,
            control_sequence: value.control_sequence,
            retry_after_ms: value.retry_after_ms,
            jitter_ms: value.jitter_ms,
            reason_code: value.reason_code,
            source_role: value.source_role,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

impl From<NnrpRetryAfterDescriptor> for RetryAfterMetadata {
    fn from(value: NnrpRetryAfterDescriptor) -> Self {
        Self {
            scope_id: value.scope_id,
            control_sequence: value.control_sequence,
            retry_after_ms: value.retry_after_ms,
            jitter_ms: value.jitter_ms,
            reason_code: value.reason_code,
            source_role: value.source_role,
            flags: value.flags,
            diagnostic_bytes: value.diagnostic_bytes,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpSessionResumeRequest {
    pub connection: NnrpHandle,
    pub requested_session_id: u32,
    pub generation: u32,
    pub profile_id: u16,
    pub schema_id: u32,
    pub schema_version: u32,
    pub resume_token_bytes: u32,
}

impl From<TypedPayloadDescriptor> for NnrpTypedPayloadDescriptor {
    fn from(value: TypedPayloadDescriptor) -> Self {
        Self {
            profile_id: value.profile_id,
            descriptor_flags: value.descriptor_flags,
            schema_id: value.schema_id,
            schema_version: value.schema_version,
            stream_semantics: value.stream_semantics,
            reserved0: 0,
            offset: value.offset,
            length: value.length,
        }
    }
}

impl From<NnrpTypedPayloadDescriptor> for TypedPayloadDescriptor {
    fn from(value: NnrpTypedPayloadDescriptor) -> Self {
        Self {
            profile_id: value.profile_id,
            descriptor_flags: value.descriptor_flags,
            schema_id: value.schema_id,
            schema_version: value.schema_version,
            stream_semantics: value.stream_semantics,
            offset: value.offset,
            length: value.length,
        }
    }
}

impl NnrpTypedPayloadDescriptor {
    fn to_core(self) -> Result<TypedPayloadDescriptor, NnrpFfiStatus> {
        if self.reserved0 != 0 {
            return Err(NnrpFfiStatus::from_core_error(
                &NnrpError::NonZeroReservedField {
                    field: "typed_payload_descriptor.reserved0",
                },
            ));
        }

        Ok(self.into())
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpSessionRecoveryOutcome {
    pub outcome_code: u32,
    pub resume_window_ms: u32,
}

impl NnrpSessionRecoveryOutcome {
    fn from_core(value: CoreSessionRecoveryOutcome) -> Self {
        match value {
            CoreSessionRecoveryOutcome::Fresh => Self {
                outcome_code: NNRP_SESSION_RECOVERY_OUTCOME_FRESH,
                resume_window_ms: 0,
            },
            CoreSessionRecoveryOutcome::ResumeEnabled { resume_window_ms } => Self {
                outcome_code: NNRP_SESSION_RECOVERY_OUTCOME_RESUME_ENABLED,
                resume_window_ms,
            },
            CoreSessionRecoveryOutcome::Resumed { resume_window_ms } => Self {
                outcome_code: NNRP_SESSION_RECOVERY_OUTCOME_RESUMED,
                resume_window_ms,
            },
            CoreSessionRecoveryOutcome::ResumeRejected => Self {
                outcome_code: NNRP_SESSION_RECOVERY_OUTCOME_RESUME_REJECTED,
                resume_window_ms: 0,
            },
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NnrpEventKind {
    None = 0,
    ConnectionOpened = 1,
    SessionOpened = 2,
    SessionPatched = 3,
    SessionClosed = 4,
    SubmitAccepted = 5,
    ResultPushed = 6,
    ResultDropped = 7,
    FlowUpdated = 8,
    Control = 9,
    Error = 10,
    ResultHint = 11,
    PartialResult = 12,
    RuntimeFrame = 13,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpEvent {
    pub kind: u32,
    pub message_type: u32,
    pub connection: NnrpHandle,
    pub session: NnrpHandle,
    pub operation: NnrpHandle,
    pub frame_id: u32,
    pub payload_owner: NnrpHandle,
    pub payload: NnrpBufferView,
    pub diagnostic: NnrpFfiDiagnostic,
}

impl NnrpEvent {
    pub const fn none() -> Self {
        Self {
            kind: NnrpEventKind::None as u32,
            message_type: 0,
            connection: NnrpHandle::invalid(),
            session: NnrpHandle::invalid(),
            operation: NnrpHandle::invalid(),
            frame_id: 0,
            payload_owner: NnrpHandle::invalid(),
            payload: NnrpBufferView::empty(),
            diagnostic: NnrpFfiDiagnostic {
                status: NnrpFfiStatus::ok(),
                related_connection_id: 0,
                related_session_id: 0,
                related_operation_id: 0,
                related_frame_id: 0,
            },
        }
    }
}

pub type NnrpEventCallback =
    Option<extern "C" fn(user_data: *mut c_void, event: *const NnrpEvent) -> u32>;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NnrpCallbackSink {
    pub user_data: *mut c_void,
    pub on_event: NnrpEventCallback,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpPollResult {
    pub status: NnrpFfiStatus,
    pub has_event: u8,
    pub event: NnrpEvent,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpCompactResult {
    pub status: NnrpFfiStatus,
    pub has_result: u8,
    pub event_kind: u32,
    pub result_state: u32,
    pub operation: NnrpHandle,
    pub operation_id: u64,
    pub frame_id: u32,
    pub payload: NnrpBufferView,
    pub diagnostic: NnrpFfiDiagnostic,
}

impl NnrpCompactResult {
    pub const fn none(status: NnrpFfiStatus) -> Self {
        Self {
            status,
            has_result: 0,
            event_kind: NnrpEventKind::None as u32,
            result_state: NNRP_RESULT_STATE_NONE,
            operation: NnrpHandle::invalid(),
            operation_id: 0,
            frame_id: 0,
            payload: NnrpBufferView::empty(),
            diagnostic: NnrpFfiDiagnostic {
                status,
                related_connection_id: 0,
                related_session_id: 0,
                related_operation_id: 0,
                related_frame_id: 0,
            },
        }
    }

    #[cfg(any(test, feature = "benchmark-ffi"))]
    fn from_event(status: NnrpFfiStatus, event: NnrpEvent, operation_id: u64) -> Self {
        Self {
            status,
            has_result: 1,
            event_kind: event.kind,
            result_state: compact_result_state(status, event.kind),
            operation: event.operation,
            operation_id,
            frame_id: event.frame_id,
            payload: event.payload,
            diagnostic: event.diagnostic,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
pub struct NnrpConnectionBootstrap {
    pub connection_id: u64,
    pub generation: u32,
    pub transport_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpClientConnectRequest {
    pub connection_id: u64,
    pub generation: u32,
    #[cfg(not(test))]
    pub reserved0: u32,
    #[cfg(not(test))]
    pub transport_connection: NnrpHandle,
    #[cfg(test)]
    pub transport_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpServerBindRequest {
    pub server_id: u64,
    pub generation: u32,
    #[cfg(not(test))]
    pub reserved0: u32,
    #[cfg(not(test))]
    pub transport_listener: NnrpHandle,
    #[cfg(test)]
    pub transport_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpSessionOpenRequest {
    pub connection: NnrpHandle,
    pub requested_session_id: u32,
    pub generation: u32,
    pub profile_id: u16,
    pub schema_id: u32,
    pub schema_version: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpSubmitRequest {
    pub session: NnrpHandle,
    pub operation_id: u64,
    pub frame_id: u32,
    pub payload: NnrpBufferView,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpClientCancelRequest {
    pub session: NnrpHandle,
    pub frame_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpServerAcceptRequest {
    pub server: NnrpHandle,
    #[cfg(not(test))]
    pub session_handle_id: u64,
    pub generation: u32,
    #[cfg(not(test))]
    pub timeout_ms: u32,
    #[cfg(test)]
    pub session_id: u32,
    #[cfg(test)]
    pub profile_id: u16,
    #[cfg(test)]
    pub schema_id: u32,
    #[cfg(test)]
    pub schema_version: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpRoleEventPollRequest {
    pub scope: NnrpHandle,
    pub max_events: u32,
    pub timeout_ms: u32,
    pub flags: u32,
    pub reserved0: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
pub struct NnrpServerReceiveSubmitRequest {
    pub session: NnrpHandle,
    pub operation_id: u64,
    pub frame_id: u32,
    pub payload: NnrpBufferView,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpServerSendResultRequest {
    pub operation: NnrpHandle,
    pub payload: NnrpBufferView,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpServerSendPartialResultRequest {
    pub operation: NnrpHandle,
    pub partial_result: NnrpPartialResultDescriptor,
    pub partial_body: NnrpBufferView,
    pub max_events: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpServerDropStaleResultRequest {
    pub operation: NnrpHandle,
    pub drop_reason: NnrpResultDropReasonDescriptor,
    pub diagnostics: NnrpBufferView,
    pub max_events: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpClientCompleteOperationRequest {
    pub operation: NnrpHandle,
    pub payload: NnrpBufferView,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpClientDropOperationRequest {
    pub operation: NnrpHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpClientSubmitResultRequest {
    pub session: NnrpHandle,
    pub operation_id: u64,
    pub frame_id: u32,
    pub submit_payload: NnrpBufferView,
    pub result_payload: NnrpBufferView,
    pub max_events: usize,
}

const fn enabled_transport_slots() -> u32 {
    let mut slots = 0;
    if cfg!(feature = "transport-quic") {
        slots |= transport_slot_bit(TransportId::Quic);
    }
    if cfg!(feature = "transport-tcp") {
        slots |= transport_slot_bit(TransportId::Tcp);
    }
    if cfg!(feature = "transport-ipc") {
        slots |= transport_slot_bit(TransportId::Ipc);
    }
    if cfg!(feature = "transport-websocket") {
        slots |= transport_slot_bit(TransportId::WebSocket);
    }
    slots
}

#[cfg(test)]
fn transport_id_enabled(transport_id: u32) -> bool {
    match TransportId::try_from_u32(transport_id) {
        Ok(TransportId::Quic) => cfg!(feature = "transport-quic"),
        Ok(TransportId::Tcp) => cfg!(feature = "transport-tcp"),
        Ok(TransportId::Ipc) => cfg!(feature = "transport-ipc"),
        Ok(TransportId::WebSocket) => cfg!(feature = "transport-websocket"),
        Ok(TransportId::Unspecified) | Err(_) => false,
    }
}

#[cfg(not(test))]
const fn runtime_transport_id(transport: RuntimeTransportKind) -> u32 {
    match transport {
        RuntimeTransportKind::Quic => TransportId::Quic as u32,
        RuntimeTransportKind::Tcp => TransportId::Tcp as u32,
        RuntimeTransportKind::Ipc => TransportId::Ipc as u32,
        RuntimeTransportKind::WebSocket => TransportId::WebSocket as u32,
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct NnrpClientSubmitResultBatchRequest {
    pub session: NnrpHandle,
    pub operation_id_start: u64,
    pub frame_id_start: u32,
    pub frame_id_stride: u32,
    pub submit_payload: NnrpBufferView,
    pub result_payload: NnrpBufferView,
    pub max_events: usize,
    pub iterations: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpClientRuntimeObjectLoopRequest {
    pub session: NnrpHandle,
    pub operation_id: u64,
    pub frame_id: u32,
    pub submit_payload: NnrpBufferView,
    pub object_descriptor: NnrpRuntimeObjectDescriptor,
    pub object_metadata: NnrpBufferView,
    pub cache_reference: NnrpCacheReferenceDescriptor,
    pub cache_reference_metadata: NnrpBufferView,
    pub progress: NnrpProgressDescriptor,
    pub progress_body: NnrpBufferView,
    pub partial_result: NnrpPartialResultDescriptor,
    pub partial_body: NnrpBufferView,
    pub object_release: NnrpObjectReleaseDescriptor,
    pub release_diagnostics: NnrpBufferView,
    pub result_payload: NnrpBufferView,
    pub max_events: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
pub struct NnrpControlRequest {
    pub handle: NnrpHandle,
    pub control_code: u32,
    pub payload: NnrpBufferView,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpRuntimeFrameSendRequest {
    pub handle: NnrpHandle,
    pub message_type: u32,
    pub frame_id: u32,
    pub payload: NnrpBufferView,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
pub struct NnrpClientSubmitControlRequest {
    pub control: NnrpControlRequest,
    pub max_events: usize,
}

#[no_mangle]
/// # Safety
///
/// `out_connection` must be either null or a valid writable pointer to one
/// `NnrpHandle`. When non-null, the pointed memory must be owned by the caller.
#[allow(clippy::needless_return)]
pub unsafe extern "C" fn nnrp_client_connect(
    request: NnrpClientConnectRequest,
    out_connection: *mut NnrpHandle,
) -> NnrpFfiStatus {
    #[cfg(test)]
    {
        if out_connection.is_null() || request.connection_id == 0 || request.generation == 0 {
            return NnrpFfiStatus::invalid_argument(10);
        }
        if !transport_id_enabled(request.transport_id) {
            return NnrpFfiStatus::invalid_argument(46);
        }

        let handle = NnrpHandle::new(
            NnrpHandleKind::Connection,
            request.connection_id,
            request.generation,
        );
        let mut store = handle_store();
        if let Err(status) = store.insert(
            handle,
            NnrpFfiResource::Connection {
                transport_id: request.transport_id,
                role: NnrpFfiConnectionRole::Client,
            },
        ) {
            return status;
        }
        store.push_event(NnrpQueuedEvent::plain(
            NnrpEventKind::ConnectionOpened,
            handle,
            NnrpHandle::invalid(),
            NnrpHandle::invalid(),
            0,
        ));
        *out_connection = handle;
        return NnrpFfiStatus::ok();
    }

    #[cfg(not(test))]
    {
        if out_connection.is_null()
            || request.connection_id == 0
            || request.generation == 0
            || request.reserved0 != 0
        {
            return NnrpFfiStatus::invalid_argument(10);
        }

        let carrier = match transport::role_transport(request.transport_connection) {
            Ok(carrier) => carrier,
            Err(status) => return status,
        };
        let transport_id = runtime_transport_id(carrier.transport_kind());

        let handle = NnrpHandle::new(
            NnrpHandleKind::Connection,
            request.connection_id,
            request.generation,
        );
        let mut store = handle_store();
        if let Err(status) = store.insert(
            handle,
            NnrpFfiResource::Connection {
                transport_id,
                role: NnrpFfiConnectionRole::Client,
                runtime: NnrpFfiConnectionRuntime::Client {
                    carrier: Some(carrier),
                },
            },
        ) {
            return status;
        }

        if let Err(status) = transport::consume_role_transport(request.transport_connection) {
            let _ = store.remove(handle, NnrpHandleKind::Connection);
            return status;
        }

        *out_connection = handle;
        NnrpFfiStatus::ok()
    }
}

#[cfg(test)]
#[no_mangle]
/// # Safety
///
/// Test-only logical bootstrap retained for the crate's legacy unit fixtures.
pub unsafe extern "C" fn nnrp_connection_bootstrap(
    request: NnrpConnectionBootstrap,
    out_connection: *mut NnrpHandle,
) -> NnrpFfiStatus {
    nnrp_client_connect(
        NnrpClientConnectRequest {
            connection_id: request.connection_id,
            generation: request.generation,
            transport_id: request.transport_id,
        },
        out_connection,
    )
}

#[no_mangle]
/// # Safety
///
/// `out_session` must be either null or a valid writable pointer to one
/// `NnrpHandle`. The connection handle is copied by value and is not retained.
pub unsafe extern "C" fn nnrp_session_open(
    request: NnrpSessionOpenRequest,
    out_session: *mut NnrpHandle,
) -> NnrpFfiStatus {
    nnrp_client_open_session(request, out_session)
}

#[no_mangle]
/// # Safety
///
/// `out_session` must be either null or a valid writable pointer to one
/// `NnrpHandle`. The connection handle is copied by value and is not retained.
#[allow(clippy::needless_return)]
pub unsafe extern "C" fn nnrp_client_open_session(
    request: NnrpSessionOpenRequest,
    out_session: *mut NnrpHandle,
) -> NnrpFfiStatus {
    #[cfg(test)]
    {
        if out_session.is_null() || request.requested_session_id == 0 || request.generation == 0 {
            return NnrpFfiStatus::invalid_argument(11);
        }
        let mut store = handle_store();
        match store.get_connection_role(request.connection) {
            Ok(NnrpFfiConnectionRole::Client) => {}
            Ok(NnrpFfiConnectionRole::Server) => {
                return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32);
            }
            Err(status) => return status,
        }

        let handle = NnrpHandle::new(
            NnrpHandleKind::Session,
            request.requested_session_id as u64,
            request.generation,
        );
        if let Err(status) = store.insert(
            handle,
            NnrpFfiResource::Session {
                connection: request.connection,
                profile_id: request.profile_id,
                schema_id: request.schema_id,
                schema_version: request.schema_version,
            },
        ) {
            return status;
        }
        store.push_event(NnrpQueuedEvent::plain(
            NnrpEventKind::SessionOpened,
            request.connection,
            handle,
            NnrpHandle::invalid(),
            0,
        ));
        *out_session = handle;
        return NnrpFfiStatus::ok();
    }

    #[cfg(not(test))]
    {
        if out_session.is_null() || request.requested_session_id == 0 || request.generation == 0 {
            return NnrpFfiStatus::invalid_argument(11);
        }
        let (transport_id, carrier) = {
            let mut store = handle_store();
            match store.get_mut(request.connection, NnrpHandleKind::Connection) {
                Ok(NnrpFfiResource::Connection {
                    transport_id,
                    role: NnrpFfiConnectionRole::Client,
                    runtime: NnrpFfiConnectionRuntime::Client { carrier },
                }) => {
                    let Some(carrier) = carrier.take() else {
                        return NnrpFfiStatus::invalid_state(11);
                    };
                    (*transport_id, carrier)
                }
                Ok(_) => {
                    return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32);
                }
                Err(status) => return status,
            }
        };

        let transport_kind = carrier.transport_kind();
        if runtime_transport_id(transport_kind) != transport_id {
            return NnrpFfiStatus::invalid_state(12);
        }
        let mut config = NnrpClientConfig::default().with_transport(transport_kind);
        config.requested_session_id = request.requested_session_id;
        config.profile_id = request.profile_id;
        config.schema_id = request.schema_id;
        config.schema_version = request.schema_version;
        let client = match NnrpClient::from_boxed_transport(carrier, config) {
            Ok(client) => client,
            Err(error) => return transport::role_status_from_runtime_error(error),
        };
        let session = match transport::run_role_async(async move { client.open_session().await }, 0)
        {
            Ok(session) => session,
            Err(status) => return status,
        };

        let handle = NnrpHandle::new(
            NnrpHandleKind::Session,
            session.session_id() as u64,
            request.generation,
        );
        let mut store = handle_store();
        if let Err(status) = store.insert(
            handle,
            NnrpFfiResource::Session {
                connection: request.connection,
                profile_id: request.profile_id,
                schema_id: request.schema_id,
                schema_version: request.schema_version,
                runtime: NnrpFfiSessionRuntime::Client(Arc::new(AsyncMutex::new(session))),
            },
        ) {
            return status;
        }

        *out_session = handle;
        NnrpFfiStatus::ok()
    }
}

#[no_mangle]
/// # Safety
///
/// `out_session` and `out_outcome` must be either null or valid writable
/// pointers to one value each. The connection handle is copied by value.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_client_resume_session(request: NnrpSessionResumeRequest, out_session: *mut NnrpHandle, out_outcome: *mut NnrpSessionRecoveryOutcome) -> NnrpFfiStatus {
    nnrp_client_resume_session_impl(request, out_session, out_outcome)
}

unsafe fn nnrp_client_resume_session_impl(
    request: NnrpSessionResumeRequest,
    out_session: *mut NnrpHandle,
    out_outcome: *mut NnrpSessionRecoveryOutcome,
) -> NnrpFfiStatus {
    if out_session.is_null()
        || out_outcome.is_null()
        || request.requested_session_id == 0
        || request.generation == 0
        || request.resume_token_bytes == 0
    {
        return NnrpFfiStatus::invalid_argument(39);
    }

    let open = SessionOpenMetadata {
        requested_session_id: request.requested_session_id,
        profile_id: request.profile_id,
        priority_class: nnrp_core::SessionPriorityClass::Balanced,
        session_flags: SESSION_FLAG_ALLOW_RESUME,
        schema_id: request.schema_id,
        schema_version: request.schema_version,
        default_deadline_ms: 0,
        max_in_flight_operations: 1,
        lease_ttl_hint_ms: 0,
        resume_token_bytes: request.resume_token_bytes,
        auth_bytes: 0,
        session_extension_bytes: 0,
        client_session_tag: request.requested_session_id as u64,
    };
    if let Err(error) = validate_session_recovery_request(&open) {
        return NnrpFfiStatus::from_core_error(&error);
    }

    let session_request = NnrpSessionOpenRequest {
        connection: request.connection,
        requested_session_id: request.requested_session_id,
        generation: request.generation,
        profile_id: request.profile_id,
        schema_id: request.schema_id,
        schema_version: request.schema_version,
    };
    let status = nnrp_client_open_session(session_request, out_session);
    if status.status_code != NnrpFfiStatusCode::Ok as u32 {
        return status;
    }
    *out_outcome = NnrpSessionRecoveryOutcome {
        outcome_code: NNRP_SESSION_RECOVERY_OUTCOME_RESUMED,
        resume_window_ms: 0,
    };
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// `out_operation` must be either null or a valid writable pointer to one
/// `NnrpHandle`. `request.payload` must remain readable for `request.payload.len`
/// bytes for the duration of the call.
pub unsafe extern "C" fn nnrp_submit(
    request: NnrpSubmitRequest,
    out_operation: *mut NnrpHandle,
) -> NnrpFfiStatus {
    nnrp_client_submit(request, out_operation)
}

#[no_mangle]
/// # Safety
///
/// `out_operation` must be either null or a valid writable pointer to one
/// `NnrpHandle`. `request.payload` must remain readable for `request.payload.len`
/// bytes for the duration of the call.
pub unsafe extern "C" fn nnrp_client_submit(
    request: NnrpSubmitRequest,
    out_operation: *mut NnrpHandle,
) -> NnrpFfiStatus {
    nnrp_client_submit_impl(request, out_operation)
}

#[cfg(test)]
unsafe fn nnrp_client_submit_impl(
    request: NnrpSubmitRequest,
    out_operation: *mut NnrpHandle,
) -> NnrpFfiStatus {
    if out_operation.is_null() || request.operation_id == 0 {
        return NnrpFfiStatus::invalid_argument(12);
    }
    if let Err(status) = request.payload.validate() {
        return status;
    }
    let mut store = handle_store();
    let session_resource = match store.get(request.session, NnrpHandleKind::Session) {
        Ok(NnrpFfiResource::Session { connection, .. }) => *connection,
        Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
        Err(status) => return status,
    };

    if store.entries.values().any(|entry| {
        matches!(
            entry.resource,
            NnrpFfiResource::Operation {
                session,
                operation_id,
                ..
            } if session == request.session && operation_id == request.operation_id
        )
    }) {
        return NnrpFfiStatus::invalid_argument(145);
    }
    let handle = NnrpHandle::new(
        NnrpHandleKind::Operation,
        next_handle_id(&mut store, NnrpHandleKind::Operation),
        request.session.generation,
    );
    if let Err(status) = store.insert(
        handle,
        NnrpFfiResource::Operation {
            session: request.session,
            operation_id: request.operation_id,
            frame_id: request.frame_id,
            payload_len: request.payload.len,
        },
    ) {
        return status;
    }

    store.push_event(NnrpQueuedEvent::plain(
        NnrpEventKind::SubmitAccepted,
        session_resource,
        request.session,
        handle,
        request.frame_id,
    ));
    *out_operation = handle;
    NnrpFfiStatus::ok()
}

#[cfg(not(test))]
unsafe fn nnrp_client_submit_impl(
    request: NnrpSubmitRequest,
    out_operation: *mut NnrpHandle,
) -> NnrpFfiStatus {
    if out_operation.is_null() || request.operation_id == 0 || request.frame_id == 0 {
        return NnrpFfiStatus::invalid_argument(12);
    }
    if let Err(status) = request.payload.validate() {
        return status;
    }
    let payload = ffi_read_slice(request.payload);
    if payload.len() < FRAME_SUBMIT_METADATA_LEN {
        return NnrpFfiStatus::invalid_argument(144);
    }
    let metadata = match FrameSubmitMetadata::parse(&payload[..FRAME_SUBMIT_METADATA_LEN]) {
        Ok(metadata) => metadata,
        Err(error) => return NnrpFfiStatus::from_core_error(&error),
    };
    if metadata.operation_id != request.operation_id {
        return NnrpFfiStatus::invalid_argument(150);
    }
    let body = payload[FRAME_SUBMIT_METADATA_LEN..].to_vec();
    let (session, operation) = {
        let mut store = handle_store();
        let session = match store.get(request.session, NnrpHandleKind::Session) {
            Ok(NnrpFfiResource::Session {
                runtime: NnrpFfiSessionRuntime::Client(session),
                ..
            }) => Arc::clone(session),
            Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
            Err(status) => return status,
        };
        if store.entries.values().any(|entry| {
            matches!(
                entry.resource,
                NnrpFfiResource::Operation {
                    session,
                    operation_id,
                    ..
                } if session == request.session && operation_id == request.operation_id
            )
        }) {
            return NnrpFfiStatus::invalid_argument(145);
        }
        let operation = NnrpHandle::new(
            NnrpHandleKind::Operation,
            next_handle_id(&mut store, NnrpHandleKind::Operation),
            request.session.generation,
        );
        if let Err(status) = store.insert(
            operation,
            NnrpFfiResource::Operation {
                session: request.session,
                operation_id: request.operation_id,
                frame_id: request.frame_id,
                payload_len: request.payload.len,
            },
        ) {
            return status;
        }
        (session, operation)
    };
    let frame_id = request.frame_id;
    if let Err(status) = transport::run_role_async(
        async move {
            session
                .lock()
                .await
                .submit_with_frame_id(frame_id, metadata, body)
                .await
                .map(|_| ())
        },
        0,
    ) {
        let _ = handle_store().remove(operation, NnrpHandleKind::Operation);
        return status;
    }
    *out_operation = operation;
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// The session handle is copied by value. This function does not dereference
/// caller-provided pointers.
pub unsafe extern "C" fn nnrp_session_close(session: NnrpHandle) -> NnrpFfiStatus {
    nnrp_client_close(session)
}

#[no_mangle]
/// # Safety
///
/// The session handle is copied by value. This function does not dereference
/// caller-provided pointers.
pub unsafe extern "C" fn nnrp_client_close(session: NnrpHandle) -> NnrpFfiStatus {
    #[cfg(not(test))]
    {
        let runtime = {
            let store = handle_store();
            match store.get(session, NnrpHandleKind::Session) {
                Ok(NnrpFfiResource::Session {
                    runtime: NnrpFfiSessionRuntime::Client(runtime),
                    ..
                }) => Arc::clone(runtime),
                Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
                Err(status) => return status,
            }
        };
        if let Err(status) = transport::run_role_async(
            async move { runtime.lock().await.close_in_place().await },
            0,
        ) {
            return status;
        }
        return handle_store()
            .close_session(session)
            .map(|_| NnrpFfiStatus::ok())
            .unwrap_or_else(|status| status);
    }

    #[cfg(test)]
    {
        let mut store = handle_store();
        let connection = match store.get(session, NnrpHandleKind::Session) {
            Ok(NnrpFfiResource::Session { connection, .. }) => *connection,
            Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
            Err(status) => return status,
        };
        store
            .close_session(session)
            .map(|_| {
                store.push_event(NnrpQueuedEvent::plain(
                    NnrpEventKind::SessionClosed,
                    connection,
                    session,
                    NnrpHandle::invalid(),
                    0,
                ));
                NnrpFfiStatus::ok()
            })
            .unwrap_or_else(|status| status)
    }
}

#[no_mangle]
/// # Safety
///
/// The connection handle is copied by value. This function does not dereference
/// caller-provided pointers.
pub unsafe extern "C" fn nnrp_connection_close(connection: NnrpHandle) -> NnrpFfiStatus {
    nnrp_client_close_connection(connection)
}

#[no_mangle]
/// # Safety
///
/// The connection handle is copied by value. This function does not dereference
/// caller-provided pointers. Closing a connection invalidates all owned session
/// and operation handles.
pub unsafe extern "C" fn nnrp_client_close_connection(connection: NnrpHandle) -> NnrpFfiStatus {
    let mut store = handle_store();
    store
        .close_connection(connection)
        .map(|_| NnrpFfiStatus::ok())
        .unwrap_or_else(|status| status)
}

#[no_mangle]
/// # Safety
///
/// The session handle is copied by value. This function does not dereference
/// caller-provided pointers.
pub unsafe extern "C" fn nnrp_client_cancel(request: NnrpClientCancelRequest) -> NnrpFfiStatus {
    if request.frame_id == 0 {
        return NnrpFfiStatus::invalid_argument(16);
    }
    #[cfg(not(test))]
    {
        let runtime = {
            let store = handle_store();
            match store.get(request.session, NnrpHandleKind::Session) {
                Ok(NnrpFfiResource::Session {
                    runtime: NnrpFfiSessionRuntime::Client(runtime),
                    ..
                }) => Arc::clone(runtime),
                Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
                Err(status) => return status,
            }
        };
        transport::run_role_async(
            async move { runtime.lock().await.cancel_frame(request.frame_id).await },
            0,
        )
        .map(|_| NnrpFfiStatus::ok())
        .unwrap_or_else(|status| status)
    }

    #[cfg(test)]
    {
        let mut store = handle_store();
        let connection = match store.get(request.session, NnrpHandleKind::Session) {
            Ok(NnrpFfiResource::Session { connection, .. }) => *connection,
            Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
            Err(status) => return status,
        };
        store.push_event(NnrpQueuedEvent::plain(
            NnrpEventKind::Control,
            connection,
            request.session,
            NnrpHandle::invalid(),
            request.frame_id,
        ));
        NnrpFfiStatus::ok()
    }
}

#[cfg(any(test, feature = "benchmark-ffi"))]
unsafe fn benchmark_client_complete_operation(
    request: NnrpClientCompleteOperationRequest,
) -> NnrpFfiStatus {
    if let Err(status) = request.payload.validate() {
        return status;
    }
    push_operation_event(request.operation, NnrpEventKind::ResultPushed, true)
}

#[cfg(test)]
fn benchmark_client_drop_operation(request: NnrpClientDropOperationRequest) -> NnrpFfiStatus {
    push_operation_event(request.operation, NnrpEventKind::ResultDropped, true)
}

#[cfg(test)]
unsafe fn benchmark_client_submit_result(
    request: NnrpClientSubmitResultRequest,
    out_operation: *mut NnrpHandle,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    benchmark_client_submit_result_impl(request, out_operation, out_result)
}

#[cfg(any(test, feature = "benchmark-ffi"))]
#[cfg_attr(feature = "benchmark-ffi", no_mangle)]
/// # Safety
///
/// `request.submit_payload` and `request.result_payload` must remain readable
/// for their declared lengths for the duration of the call. `out_result` must
/// point to one caller-owned writable `NnrpCompactResult`. This benchmark-only
/// helper synthesizes completion after submission to measure host FFI overhead.
pub unsafe extern "C" fn nnrp_benchmark_client_submit_result_compact(
    request: NnrpClientSubmitResultRequest,
    out_result: *mut NnrpCompactResult,
) -> NnrpFfiStatus {
    benchmark_client_submit_result_compact_impl(request, out_result)
}

#[cfg(any(test, feature = "benchmark-ffi"))]
#[cfg_attr(feature = "benchmark-ffi", no_mangle)]
/// # Safety
///
/// `request.submit_payload` and `request.result_payload` must remain readable
/// for their declared lengths for the duration of the call. `out_last_result`
/// must point to one caller-owned writable `NnrpCompactResult`; `out_completed`
/// must point to one caller-owned writable `uintptr_t`. This helper repeats the
/// synthetic compact submit/result operation in one ABI call. This symbol is
/// exported only by an explicit `benchmark-ffi` build.
pub unsafe extern "C" fn nnrp_benchmark_client_submit_result_compact_batch(
    request: NnrpClientSubmitResultBatchRequest,
    out_last_result: *mut NnrpCompactResult,
    out_completed: *mut usize,
) -> NnrpFfiStatus {
    benchmark_client_submit_result_compact_batch_impl(request, out_last_result, out_completed)
}

#[cfg(any(test, feature = "benchmark-ffi"))]
#[cfg_attr(feature = "benchmark-ffi", no_mangle)]
/// # Safety
///
/// All buffer views in `request` must remain readable for their declared
/// lengths for the duration of the call. `out_result` must point to one
/// caller-owned writable `NnrpCompactResult`. This helper validates the
/// runtime-object declare/cache/progress/partial-result/release metadata and
/// then executes a synthetic compact submit/result path. This symbol is
/// exported only by an explicit `benchmark-ffi` build.
pub unsafe extern "C" fn nnrp_benchmark_client_runtime_object_loop_compact(
    request: NnrpClientRuntimeObjectLoopRequest,
    out_result: *mut NnrpCompactResult,
) -> NnrpFfiStatus {
    benchmark_client_runtime_object_loop_compact_impl(request, out_result)
}

#[cfg(test)]
unsafe fn benchmark_client_submit_result_impl(
    request: NnrpClientSubmitResultRequest,
    out_operation: *mut NnrpHandle,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    if out_operation.is_null() || out_result.is_null() {
        return NnrpFfiStatus::invalid_argument(47);
    }
    if let Err(status) = request.result_payload.validate() {
        return status;
    }
    let submit_request = NnrpSubmitRequest {
        session: request.session,
        operation_id: request.operation_id,
        frame_id: request.frame_id,
        payload: request.submit_payload,
    };
    let mut operation = NnrpHandle::invalid();
    let submit_status = nnrp_client_submit(submit_request, &mut operation);
    if submit_status.status_code != NnrpFfiStatusCode::Ok as u32 {
        return submit_status;
    }
    *out_operation = operation;
    let complete_status = benchmark_client_complete_operation(NnrpClientCompleteOperationRequest {
        operation,
        payload: request.result_payload,
    });
    if complete_status.status_code != NnrpFfiStatusCode::Ok as u32 {
        return complete_status;
    }

    poll_matching_operation_result(
        request.session,
        operation,
        request.frame_id,
        request.max_events,
        out_result,
    )
}

#[cfg(any(test, feature = "benchmark-ffi"))]
unsafe fn benchmark_client_submit_result_compact_impl(
    request: NnrpClientSubmitResultRequest,
    out_result: *mut NnrpCompactResult,
) -> NnrpFfiStatus {
    if out_result.is_null() {
        return NnrpFfiStatus::invalid_argument(48);
    }
    if let Err(status) = request.result_payload.validate() {
        return status;
    }
    let submit_request = NnrpSubmitRequest {
        session: request.session,
        operation_id: request.operation_id,
        frame_id: request.frame_id,
        payload: request.submit_payload,
    };
    let mut operation = NnrpHandle::invalid();
    let submit_status = nnrp_client_submit(submit_request, &mut operation);
    if submit_status.status_code != NnrpFfiStatusCode::Ok as u32 {
        *out_result = NnrpCompactResult::none(submit_status);
        return submit_status;
    }
    let complete_status = benchmark_client_complete_operation(NnrpClientCompleteOperationRequest {
        operation,
        payload: request.result_payload,
    });
    if complete_status.status_code != NnrpFfiStatusCode::Ok as u32 {
        *out_result = NnrpCompactResult::none(complete_status);
        return complete_status;
    }

    poll_matching_operation_compact_result(
        request.session,
        operation,
        request.operation_id,
        request.frame_id,
        request.result_payload,
        request.max_events,
        out_result,
    )
}

#[cfg(any(test, feature = "benchmark-ffi"))]
unsafe fn benchmark_client_submit_result_compact_batch_impl(
    request: NnrpClientSubmitResultBatchRequest,
    out_last_result: *mut NnrpCompactResult,
    out_completed: *mut usize,
) -> NnrpFfiStatus {
    if out_last_result.is_null() || out_completed.is_null() {
        return NnrpFfiStatus::invalid_argument(126);
    }
    if request.iterations == 0 {
        *out_completed = 0;
        *out_last_result = NnrpCompactResult::none(NnrpFfiStatus::ok());
        return NnrpFfiStatus::ok();
    }

    let stride = if request.frame_id_stride == 0 {
        1
    } else {
        request.frame_id_stride
    };
    let mut completed = 0usize;
    let mut last = NnrpCompactResult::none(NnrpFfiStatus::ok());
    for index in 0..request.iterations {
        let frame_id = request
            .frame_id_start
            .wrapping_add((index as u32).wrapping_mul(stride));
        let status = benchmark_client_submit_result_compact_impl(
            NnrpClientSubmitResultRequest {
                session: request.session,
                operation_id: request.operation_id_start.wrapping_add(index as u64),
                frame_id,
                submit_payload: request.submit_payload,
                result_payload: request.result_payload,
                max_events: request.max_events,
            },
            &mut last,
        );
        if status.status_code != NnrpFfiStatusCode::Ok as u32 {
            *out_completed = completed;
            *out_last_result = last;
            return status;
        }
        completed += 1;
    }

    *out_completed = completed;
    *out_last_result = last;
    NnrpFfiStatus::ok()
}

#[cfg(any(test, feature = "benchmark-ffi"))]
unsafe fn benchmark_client_runtime_object_loop_compact_impl(
    request: NnrpClientRuntimeObjectLoopRequest,
    out_result: *mut NnrpCompactResult,
) -> NnrpFfiStatus {
    if out_result.is_null() {
        return NnrpFfiStatus::invalid_argument(58);
    }
    if let Err(status) = validate_runtime_object_loop_request(request) {
        *out_result = NnrpCompactResult::none(status);
        return status;
    }

    benchmark_client_submit_result_compact_impl(
        NnrpClientSubmitResultRequest {
            session: request.session,
            operation_id: request.operation_id,
            frame_id: request.frame_id,
            submit_payload: request.submit_payload,
            result_payload: request.result_payload,
            max_events: request.max_events,
        },
        out_result,
    )
}

#[cfg(any(test, feature = "benchmark-ffi"))]
unsafe fn validate_runtime_object_loop_request(
    request: NnrpClientRuntimeObjectLoopRequest,
) -> Result<(), NnrpFfiStatus> {
    request.submit_payload.validate()?;
    request.object_metadata.validate()?;
    request.cache_reference_metadata.validate()?;
    request.progress_body.validate()?;
    request.partial_body.validate()?;
    request.release_diagnostics.validate()?;
    request.result_payload.validate()?;

    request
        .object_descriptor
        .to_core()?
        .to_vec_with_extension(ffi_read_slice(request.object_metadata))
        .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
    request
        .cache_reference
        .to_core()?
        .to_vec_with_extension(ffi_read_slice(request.cache_reference_metadata))
        .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;

    let progress = ProgressMetadata::from(request.progress);
    validate_progress_semantics(&progress)
        .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
    progress
        .to_vec_with_body(ffi_read_slice(request.progress_body))
        .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;

    let partial_result = PartialResultMetadata::from(request.partial_result);
    validate_partial_result_semantics(&partial_result)
        .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
    partial_result
        .to_vec_with_body(ffi_read_slice(request.partial_body))
        .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;

    request
        .object_release
        .to_core()?
        .to_vec_with_diagnostics(ffi_read_slice(request.release_diagnostics))
        .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;

    Ok(())
}

#[cfg(test)]
unsafe fn test_client_send_result_hint(request: NnrpControlRequest) -> NnrpFfiStatus {
    if request.control_code != MessageType::ResultHint as u32 {
        return NnrpFfiStatus::invalid_argument(34);
    }
    test_control_event_injection(request)
}

#[no_mangle]
/// # Safety
///
/// `out_result` must be either null or a valid writable pointer to one
/// `NnrpPollResult`. When non-null, the pointed memory must be owned by the caller.
#[allow(clippy::needless_return)]
pub unsafe extern "C" fn nnrp_client_await_event(
    scope: NnrpHandle,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    #[cfg(not(test))]
    {
        if out_result.is_null() {
            return NnrpFfiStatus::invalid_argument(17);
        }
        let mut event = NnrpEvent::none();
        let mut count = 0usize;
        let status = role_client_await_events_impl(
            NnrpRoleEventPollRequest {
                scope,
                max_events: 1,
                timeout_ms: 0,
                flags: 0,
                reserved0: 0,
            },
            &mut event,
            1,
            &mut count,
        );
        *out_result = NnrpPollResult {
            status,
            has_event: u8::from(count == 1),
            event,
        };
        return status;
    }

    #[cfg(test)]
    {
        if out_result.is_null() {
            return NnrpFfiStatus::invalid_argument(17);
        }
        let mut store = handle_store();
        match store.poll_event(scope) {
            Ok(Some(event)) => {
                *out_result = NnrpPollResult {
                    status: NnrpFfiStatus::ok(),
                    has_event: 1,
                    event,
                };
                NnrpFfiStatus::ok()
            }
            Ok(None) => {
                *out_result = NnrpPollResult {
                    status: NnrpFfiStatus {
                        status_code: NnrpFfiStatusCode::WouldBlock as u32,
                        error_family: NnrpErrorFamily::None as u32,
                        protocol_error_code: 0,
                        detail_code: 0,
                    },
                    has_event: 0,
                    event: NnrpEvent::none(),
                };
                (*out_result).status
            }
            Err(status) => status,
        }
    }
}

#[cfg(test)]
#[no_mangle]
/// # Safety
///
/// `out_events` must point to `event_capacity` writable `NnrpEvent` entries
/// when `event_capacity` is non-zero. `out_event_count` must point to one
/// writable `usize`. Both pointed regions must be caller-owned for the duration
/// of this call.
pub unsafe extern "C" fn nnrp_client_await_events(
    connection: NnrpHandle,
    out_events: *mut NnrpEvent,
    event_capacity: usize,
    out_event_count: *mut usize,
) -> NnrpFfiStatus {
    client_await_events_impl(connection, out_events, event_capacity, out_event_count)
}

#[cfg(not(test))]
#[no_mangle]
/// Reads and decodes a bounded client event batch from a live role session.
///
/// # Safety
///
/// `out_events` must point to `event_capacity` writable `NnrpEvent` entries
/// when `event_capacity` is non-zero. `out_event_count` must point to one
/// writable `usize`.
pub unsafe extern "C" fn nnrp_client_await_events(
    request: NnrpRoleEventPollRequest,
    out_events: *mut NnrpEvent,
    event_capacity: usize,
    out_event_count: *mut usize,
) -> NnrpFfiStatus {
    role_client_await_events_impl(request, out_events, event_capacity, out_event_count)
}

#[no_mangle]
/// # Safety
///
/// `source` must remain readable for `source.len` bytes for the duration of
/// this call. `out_descriptor` must be either null or a valid writable pointer
/// to one `NnrpSchemaDescriptorHeader`.
pub unsafe extern "C" fn nnrp_schema_descriptor_parse(
    source: NnrpBufferView,
    out_descriptor: *mut NnrpSchemaDescriptorHeader,
) -> NnrpFfiStatus {
    schema_descriptor_parse_impl(source, out_descriptor)
}

unsafe fn schema_descriptor_parse_impl(
    source: NnrpBufferView,
    out_descriptor: *mut NnrpSchemaDescriptorHeader,
) -> NnrpFfiStatus {
    if out_descriptor.is_null() {
        return NnrpFfiStatus::invalid_argument(33);
    }
    if let Err(status) = source.validate() {
        return status;
    }
    let bytes = ffi_read_slice(source);
    match SchemaDescriptorHeader::parse(bytes) {
        Ok(descriptor) => {
            *out_descriptor = descriptor.into();
            NnrpFfiStatus::ok()
        }
        Err(error) => NnrpFfiStatus::from_core_error(&error),
    }
}

#[no_mangle]
/// # Safety
///
/// `destination` must remain writable for `destination.len` bytes for the
/// duration of this call.
pub unsafe extern "C" fn nnrp_schema_descriptor_write(
    descriptor: NnrpSchemaDescriptorHeader,
    destination: NnrpBufferViewMut,
) -> NnrpFfiStatus {
    schema_descriptor_write_impl(descriptor, destination)
}

unsafe fn schema_descriptor_write_impl(
    descriptor: NnrpSchemaDescriptorHeader,
    destination: NnrpBufferViewMut,
) -> NnrpFfiStatus {
    if let Err(status) = destination.validate() {
        return status;
    }
    let bytes = ffi_write_slice(destination);
    let core_descriptor = match descriptor.to_core() {
        Ok(descriptor) => descriptor,
        Err(status) => return status,
    };
    core_descriptor
        .write(bytes)
        .map(|_| NnrpFfiStatus::ok())
        .unwrap_or_else(|error| NnrpFfiStatus::from_core_error(&error))
}

#[no_mangle]
/// # Safety
///
/// `out_descriptor` must be either null or a valid writable pointer to one
/// `NnrpSchemaDescriptorHeader`.
pub unsafe extern "C" fn nnrp_token_delta_schema_descriptor(
    out_descriptor: *mut NnrpSchemaDescriptorHeader,
) -> NnrpFfiStatus {
    token_delta_schema_descriptor_impl(out_descriptor)
}

unsafe fn token_delta_schema_descriptor_impl(
    out_descriptor: *mut NnrpSchemaDescriptorHeader,
) -> NnrpFfiStatus {
    if out_descriptor.is_null() {
        return NnrpFfiStatus::invalid_argument(34);
    }
    *out_descriptor = token_delta_schema_descriptor().into();
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// `source` must remain readable for `source.len` bytes for the duration of
/// this call. `out_descriptor` must be either null or a valid writable pointer
/// to one `NnrpTypedPayloadDescriptor`.
pub unsafe extern "C" fn nnrp_typed_payload_descriptor_parse(
    source: NnrpBufferView,
    out_descriptor: *mut NnrpTypedPayloadDescriptor,
) -> NnrpFfiStatus {
    typed_payload_descriptor_parse_impl(source, out_descriptor)
}

unsafe fn typed_payload_descriptor_parse_impl(
    source: NnrpBufferView,
    out_descriptor: *mut NnrpTypedPayloadDescriptor,
) -> NnrpFfiStatus {
    if out_descriptor.is_null() {
        return NnrpFfiStatus::invalid_argument(35);
    }
    if let Err(status) = source.validate() {
        return status;
    }
    let bytes = ffi_read_slice(source);
    match TypedPayloadDescriptor::parse(bytes) {
        Ok(descriptor) => {
            *out_descriptor = descriptor.into();
            NnrpFfiStatus::ok()
        }
        Err(error) => NnrpFfiStatus::from_core_error(&error),
    }
}

#[no_mangle]
/// # Safety
///
/// `destination` must remain writable for `destination.len` bytes for the
/// duration of this call.
pub unsafe extern "C" fn nnrp_typed_payload_descriptor_write(
    descriptor: NnrpTypedPayloadDescriptor,
    destination: NnrpBufferViewMut,
) -> NnrpFfiStatus {
    typed_payload_descriptor_write_impl(descriptor, destination)
}

unsafe fn typed_payload_descriptor_write_impl(
    descriptor: NnrpTypedPayloadDescriptor,
    destination: NnrpBufferViewMut,
) -> NnrpFfiStatus {
    if let Err(status) = destination.validate() {
        return status;
    }
    let bytes = ffi_write_slice(destination);
    let core_descriptor = match descriptor.to_core() {
        Ok(descriptor) => descriptor,
        Err(status) => return status,
    };
    core_descriptor
        .write(bytes)
        .map(|_| NnrpFfiStatus::ok())
        .unwrap_or_else(|error| NnrpFfiStatus::from_core_error(&error))
}

#[no_mangle]
/// # Safety
///
/// `schema_descriptors` must point to `schema_count` readable
/// `NnrpSchemaDescriptorHeader` entries when `schema_count` is non-zero.
pub unsafe extern "C" fn nnrp_typed_payload_validate_binding(
    schema_descriptors: *const NnrpSchemaDescriptorHeader,
    schema_count: usize,
    descriptor: NnrpTypedPayloadDescriptor,
) -> NnrpFfiStatus {
    typed_payload_validate_binding_impl(schema_descriptors, schema_count, descriptor)
}

#[no_mangle]
/// # Safety
///
/// `out_registry` must be either null or a valid writable pointer to one
/// `NnrpHandle`.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_schema_registry_create(out_registry: *mut NnrpHandle) -> NnrpFfiStatus { nnrp_schema_registry_create_impl(out_registry) }

unsafe fn nnrp_schema_registry_create_impl(out_registry: *mut NnrpHandle) -> NnrpFfiStatus {
    if out_registry.is_null() {
        return NnrpFfiStatus::invalid_argument(40);
    }
    let mut store = handle_store();
    let id = next_handle_id(&mut store, NnrpHandleKind::SchemaRegistry);
    let handle = NnrpHandle::new(NnrpHandleKind::SchemaRegistry, id, 1);
    if let Err(status) = store.insert(
        handle,
        NnrpFfiResource::SchemaRegistry {
            registry: SchemaRegistry::with_standard_preview3_profiles(),
        },
    ) {
        return status;
    }
    *out_registry = handle;
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// `out_action` must be either null or a valid writable pointer to one `u32`.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_schema_registry_install(registry: NnrpHandle, descriptor: NnrpSchemaDescriptorHeader, out_action: *mut u32) -> NnrpFfiStatus {
    nnrp_schema_registry_install_impl(registry, descriptor, out_action)
}

unsafe fn nnrp_schema_registry_install_impl(
    registry: NnrpHandle,
    descriptor: NnrpSchemaDescriptorHeader,
    out_action: *mut u32,
) -> NnrpFfiStatus {
    if out_action.is_null() {
        return NnrpFfiStatus::invalid_argument(41);
    }
    let descriptor = match descriptor.to_core() {
        Ok(descriptor) => descriptor,
        Err(status) => return status,
    };
    let mut store = handle_store();
    match store.get_mut(registry, NnrpHandleKind::SchemaRegistry) {
        Ok(NnrpFfiResource::SchemaRegistry { registry }) => match registry.install(descriptor) {
            Ok(action) => {
                *out_action = schema_registry_action_code(action);
                NnrpFfiStatus::ok()
            }
            Err(failure) => schema_registry_failure_status(failure),
        },
        Ok(_) => NnrpFfiStatus::invalid_handle(NnrpHandleKind::SchemaRegistry as u32),
        Err(status) => status,
    }
}

#[no_mangle]
/// # Safety
///
/// `out_descriptor` must be either null or a valid writable pointer to one
/// `NnrpSchemaDescriptorHeader`.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_schema_registry_lookup(registry: NnrpHandle, schema_id: u32, schema_version: u32, out_descriptor: *mut NnrpSchemaDescriptorHeader) -> NnrpFfiStatus {
    nnrp_schema_registry_lookup_impl(registry, schema_id, schema_version, out_descriptor)
}

unsafe fn nnrp_schema_registry_lookup_impl(
    registry: NnrpHandle,
    schema_id: u32,
    schema_version: u32,
    out_descriptor: *mut NnrpSchemaDescriptorHeader,
) -> NnrpFfiStatus {
    if out_descriptor.is_null() {
        return NnrpFfiStatus::invalid_argument(42);
    }
    let store = handle_store();
    match store.get(registry, NnrpHandleKind::SchemaRegistry) {
        Ok(NnrpFfiResource::SchemaRegistry { registry }) => {
            match registry.get(schema_id, schema_version) {
                Some(descriptor) => {
                    *out_descriptor = (*descriptor).into();
                    NnrpFfiStatus::ok()
                }
                None => schema_registry_failure_status(SchemaRegistryFailure::VersionUnknown),
            }
        }
        Ok(_) => NnrpFfiStatus::invalid_handle(NnrpHandleKind::SchemaRegistry as u32),
        Err(status) => status,
    }
}

#[no_mangle]
/// # Safety
///
/// `out_action` must be either null or a valid writable pointer to one `u32`.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_schema_registry_invalidate(registry: NnrpHandle, schema_id: u32, schema_version: u32, out_action: *mut u32) -> NnrpFfiStatus {
    nnrp_schema_registry_invalidate_impl(registry, schema_id, schema_version, out_action)
}

unsafe fn nnrp_schema_registry_invalidate_impl(
    registry: NnrpHandle,
    schema_id: u32,
    schema_version: u32,
    out_action: *mut u32,
) -> NnrpFfiStatus {
    if out_action.is_null() {
        return NnrpFfiStatus::invalid_argument(43);
    }
    let mut store = handle_store();
    match store.get_mut(registry, NnrpHandleKind::SchemaRegistry) {
        Ok(NnrpFfiResource::SchemaRegistry { registry }) => {
            match registry.invalidate(schema_id, schema_version) {
                Ok(action) => {
                    *out_action = schema_registry_action_code(action);
                    NnrpFfiStatus::ok()
                }
                Err(failure) => schema_registry_failure_status(failure),
            }
        }
        Ok(_) => NnrpFfiStatus::invalid_handle(NnrpHandleKind::SchemaRegistry as u32),
        Err(status) => status,
    }
}

#[no_mangle]
/// # Safety
///
/// The registry handle and descriptor are copied by value.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_schema_registry_validate_binding(registry: NnrpHandle, descriptor: NnrpTypedPayloadDescriptor) -> NnrpFfiStatus {
    nnrp_schema_registry_validate_binding_impl(registry, descriptor)
}

unsafe fn nnrp_schema_registry_validate_binding_impl(
    registry: NnrpHandle,
    descriptor: NnrpTypedPayloadDescriptor,
) -> NnrpFfiStatus {
    let descriptor = match descriptor.to_core() {
        Ok(descriptor) => descriptor,
        Err(status) => return status,
    };
    let store = handle_store();
    match store.get(registry, NnrpHandleKind::SchemaRegistry) {
        Ok(NnrpFfiResource::SchemaRegistry { registry }) => registry
            .validate_descriptor_binding(&descriptor)
            .map(|_| NnrpFfiStatus::ok())
            .unwrap_or_else(schema_registry_failure_status),
        Ok(_) => NnrpFfiStatus::invalid_handle(NnrpHandleKind::SchemaRegistry as u32),
        Err(status) => status,
    }
}

#[no_mangle]
/// # Safety
///
/// The registry handle is copied by value. This function does not dereference
/// caller-provided pointers.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_schema_registry_release(registry: NnrpHandle) -> NnrpFfiStatus { nnrp_schema_registry_release_impl(registry) }

unsafe fn nnrp_schema_registry_release_impl(registry: NnrpHandle) -> NnrpFfiStatus {
    let mut store = handle_store();
    store
        .remove(registry, NnrpHandleKind::SchemaRegistry)
        .map(|_| NnrpFfiStatus::ok())
        .unwrap_or_else(|status| status)
}

#[no_mangle]
/// # Safety
///
/// `session_open_metadata` must remain readable for `len` bytes for the
/// duration of this call.
pub unsafe extern "C" fn nnrp_session_recovery_request_validate(
    session_open_metadata: NnrpBufferView,
) -> NnrpFfiStatus {
    session_recovery_request_validate_impl(session_open_metadata)
}

unsafe fn session_recovery_request_validate_impl(
    session_open_metadata: NnrpBufferView,
) -> NnrpFfiStatus {
    if let Err(status) = session_open_metadata.validate() {
        return status;
    }
    let request = match SessionOpenMetadata::parse(ffi_read_slice(session_open_metadata)) {
        Ok(metadata) => metadata,
        Err(error) => return NnrpFfiStatus::from_core_error(&error),
    };
    validate_session_recovery_request(&request)
        .map(|_| NnrpFfiStatus::ok())
        .unwrap_or_else(|error| NnrpFfiStatus::from_core_error(&error))
}

#[no_mangle]
/// # Safety
///
/// `session_open_metadata` and `session_open_ack_metadata` must remain readable
/// for their declared lengths. `out_outcome` must be either null or a valid
/// writable pointer to one `NnrpSessionRecoveryOutcome`.
pub unsafe extern "C" fn nnrp_session_recovery_ack_validate(
    session_open_metadata: NnrpBufferView,
    session_open_ack_metadata: NnrpBufferView,
    out_outcome: *mut NnrpSessionRecoveryOutcome,
) -> NnrpFfiStatus {
    session_recovery_ack_validate_impl(
        session_open_metadata,
        session_open_ack_metadata,
        out_outcome,
    )
}

unsafe fn session_recovery_ack_validate_impl(
    session_open_metadata: NnrpBufferView,
    session_open_ack_metadata: NnrpBufferView,
    out_outcome: *mut NnrpSessionRecoveryOutcome,
) -> NnrpFfiStatus {
    if out_outcome.is_null() {
        return NnrpFfiStatus::invalid_argument(37);
    }
    if let Err(status) = session_open_metadata.validate() {
        return status;
    }
    if let Err(status) = session_open_ack_metadata.validate() {
        return status;
    }
    let request = match SessionOpenMetadata::parse(ffi_read_slice(session_open_metadata)) {
        Ok(metadata) => metadata,
        Err(error) => return NnrpFfiStatus::from_core_error(&error),
    };
    let ack = match SessionOpenAckMetadata::parse(ffi_read_slice(session_open_ack_metadata)) {
        Ok(metadata) => metadata,
        Err(error) => return NnrpFfiStatus::from_core_error(&error),
    };
    match validate_session_recovery_ack(&request, &ack) {
        Ok(outcome) => {
            *out_outcome = NnrpSessionRecoveryOutcome::from_core(outcome);
            NnrpFfiStatus::ok()
        }
        Err(error) => NnrpFfiStatus::from_core_error(&error),
    }
}

#[no_mangle]
/// # Safety
///
/// `session_migrate_metadata` and `session_migrate_ack_metadata` must remain
/// readable for their declared lengths for the duration of this call.
pub unsafe extern "C" fn nnrp_migration_recovery_validate(
    session_migrate_metadata: NnrpBufferView,
    session_migrate_ack_metadata: NnrpBufferView,
) -> NnrpFfiStatus {
    migration_recovery_validate_impl(session_migrate_metadata, session_migrate_ack_metadata)
}

unsafe fn migration_recovery_validate_impl(
    session_migrate_metadata: NnrpBufferView,
    session_migrate_ack_metadata: NnrpBufferView,
) -> NnrpFfiStatus {
    if let Err(status) = session_migrate_metadata.validate() {
        return status;
    }
    if let Err(status) = session_migrate_ack_metadata.validate() {
        return status;
    }
    let request = match SessionMigrateMetadata::parse(ffi_read_slice(session_migrate_metadata)) {
        Ok(metadata) => metadata,
        Err(error) => return NnrpFfiStatus::from_core_error(&error),
    };
    let ack = match SessionMigrateAckMetadata::parse(ffi_read_slice(session_migrate_ack_metadata)) {
        Ok(metadata) => metadata,
        Err(error) => return NnrpFfiStatus::from_core_error(&error),
    };
    validate_migration_recovery(&request, &ack)
        .map(|_| NnrpFfiStatus::ok())
        .unwrap_or_else(|error| NnrpFfiStatus::from_core_error(&error))
}

#[no_mangle]
/// # Safety
///
/// `session_migrate_ack_metadata` must remain readable for `len` bytes.
/// `out_should_replay` must be a valid writable pointer to one byte.
pub unsafe extern "C" fn nnrp_migration_should_replay_frame(
    session_migrate_ack_metadata: NnrpBufferView,
    frame_id: u64,
    out_should_replay: *mut u8,
) -> NnrpFfiStatus {
    migration_should_replay_frame_impl(session_migrate_ack_metadata, frame_id, out_should_replay)
}

unsafe fn migration_should_replay_frame_impl(
    session_migrate_ack_metadata: NnrpBufferView,
    frame_id: u64,
    out_should_replay: *mut u8,
) -> NnrpFfiStatus {
    if out_should_replay.is_null() {
        return NnrpFfiStatus::invalid_argument(38);
    }
    if let Err(status) = session_migrate_ack_metadata.validate() {
        return status;
    }
    let ack = match SessionMigrateAckMetadata::parse(ffi_read_slice(session_migrate_ack_metadata)) {
        Ok(metadata) => metadata,
        Err(error) => return NnrpFfiStatus::from_core_error(&error),
    };
    *out_should_replay = u8::from(should_replay_frame_after_migration(&ack, frame_id));
    NnrpFfiStatus::ok()
}

unsafe fn typed_payload_validate_binding_impl(
    schema_descriptors: *const NnrpSchemaDescriptorHeader,
    schema_count: usize,
    descriptor: NnrpTypedPayloadDescriptor,
) -> NnrpFfiStatus {
    if schema_count > 0 && schema_descriptors.is_null() {
        return NnrpFfiStatus::invalid_argument(36);
    }

    let schemas = if schema_count == 0 {
        &[][..]
    } else {
        core::slice::from_raw_parts(schema_descriptors, schema_count)
    };
    let mut registry = SchemaRegistry::new();
    for schema in schemas {
        let core_schema = match schema.to_core() {
            Ok(schema) => schema,
            Err(status) => return status,
        };
        if let Err(failure) = registry.install(core_schema) {
            return schema_registry_failure_status(failure);
        }
    }

    let core_descriptor = match descriptor.to_core() {
        Ok(descriptor) => descriptor,
        Err(status) => return status,
    };
    registry
        .validate_descriptor_binding(&core_descriptor)
        .map(|_| NnrpFfiStatus::ok())
        .unwrap_or_else(schema_registry_failure_status)
}

#[cfg(test)]
unsafe fn client_await_events_impl(
    connection: NnrpHandle,
    out_events: *mut NnrpEvent,
    event_capacity: usize,
    out_event_count: *mut usize,
) -> NnrpFfiStatus {
    if out_event_count.is_null() {
        return NnrpFfiStatus::invalid_argument(31);
    }
    *out_event_count = 0;

    if event_capacity == 0 || out_events.is_null() {
        return NnrpFfiStatus::invalid_argument(32);
    }

    let mut store = handle_store();
    for index in 0..event_capacity {
        match store.poll_event(connection) {
            Ok(Some(event)) => {
                *out_events.add(index) = event;
                *out_event_count += 1;
            }
            Ok(None) => {
                return if *out_event_count == 0 {
                    NnrpFfiStatus {
                        status_code: NnrpFfiStatusCode::WouldBlock as u32,
                        error_family: NnrpErrorFamily::None as u32,
                        protocol_error_code: 0,
                        detail_code: 0,
                    }
                } else {
                    NnrpFfiStatus::ok()
                };
            }
            Err(status) => return status,
        }
    }

    NnrpFfiStatus::ok()
}

#[cfg(not(test))]
fn validate_role_event_poll(
    request: NnrpRoleEventPollRequest,
    out_events: *mut NnrpEvent,
    event_capacity: usize,
    out_event_count: *mut usize,
) -> Result<usize, NnrpFfiStatus> {
    if out_event_count.is_null() {
        return Err(NnrpFfiStatus::invalid_argument(31));
    }
    if event_capacity == 0 || out_events.is_null() {
        return Err(NnrpFfiStatus::invalid_argument(32));
    }
    if request.flags != 0 || request.reserved0 != 0 {
        return Err(NnrpFfiStatus::invalid_argument(146));
    }
    let requested = if request.max_events == 0 {
        16usize
    } else {
        request.max_events as usize
    };
    Ok(requested.min(event_capacity))
}

#[cfg(not(test))]
fn client_role_event(
    scope: NnrpHandle,
    event: NnrpClientEvent,
) -> Result<NnrpEvent, NnrpFfiStatus> {
    let connection = role_session_connection(scope, NnrpFfiConnectionRole::Client)?;
    let (kind, frame_id, operation_id, message_type, payload, terminal) = match event {
        NnrpClientEvent::Result(result) => {
            let mut payload = result
                .metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec();
            payload.extend_from_slice(&result.body);
            (
                NnrpEventKind::ResultPushed,
                result.frame_id,
                None,
                MessageType::ResultPush,
                payload,
                true,
            )
        }
        NnrpClientEvent::PartialResult { metadata, body } => {
            let operation_id = metadata.operation_id;
            (
                NnrpEventKind::RuntimeFrame,
                0,
                Some(operation_id),
                MessageType::PartialResult,
                metadata
                    .to_vec_with_body(&body)
                    .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
                false,
            )
        }
        NnrpClientEvent::Progress { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            0,
            Some(metadata.operation_id),
            MessageType::Progress,
            metadata
                .to_vec_with_body(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::Control {
            message_type,
            metadata,
            body,
        } => (
            NnrpEventKind::RuntimeFrame,
            0,
            Some(metadata.operation_id),
            message_type,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::Scheduling {
            message_type,
            metadata,
        } => (
            NnrpEventKind::RuntimeFrame,
            0,
            Some(metadata.operation_id),
            message_type,
            metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
        NnrpClientEvent::Supersede { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            0,
            Some(metadata.old_operation_id),
            MessageType::Supersede,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::Budget(metadata) => (
            NnrpEventKind::RuntimeFrame,
            0,
            Some(metadata.operation_id),
            MessageType::BudgetUpdate,
            metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
        NnrpClientEvent::ResultDrop { frame_id } => (
            NnrpEventKind::ResultDropped,
            frame_id,
            None,
            MessageType::ResultDrop,
            Vec::new(),
            true,
        ),
        NnrpClientEvent::ResultDropReason { metadata, body } => (
            NnrpEventKind::ResultDropped,
            0,
            Some(metadata.operation_id),
            MessageType::ResultDropReason,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            true,
        ),
        NnrpClientEvent::FlowUpdate(metadata) => (
            NnrpEventKind::FlowUpdated,
            0,
            None,
            MessageType::FlowUpdate,
            metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
        NnrpClientEvent::Backpressure(metadata) => (
            NnrpEventKind::RuntimeFrame,
            0,
            None,
            MessageType::Backpressure,
            metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
        NnrpClientEvent::CreditUpdate(metadata) => (
            NnrpEventKind::RuntimeFrame,
            0,
            None,
            MessageType::CreditUpdate,
            metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
        NnrpClientEvent::Capability {
            message_type,
            metadata,
            body,
        } => (
            NnrpEventKind::RuntimeFrame,
            0,
            None,
            message_type,
            metadata
                .to_vec_with_body(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::RouteHint {
            message_type,
            metadata,
            body,
        } => (
            NnrpEventKind::RuntimeFrame,
            0,
            (metadata.operation_id != 0).then_some(metadata.operation_id),
            message_type,
            metadata
                .to_vec_with_body(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::TraceContext {
            frame_id,
            metadata,
            body,
        } => (
            NnrpEventKind::RuntimeFrame,
            frame_id,
            None,
            MessageType::TraceContext,
            metadata
                .to_vec_with_body(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::RecoverableError { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            metadata.related_frame_id,
            None,
            MessageType::ErrorRecoverable,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::RetryAfter { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            0,
            None,
            MessageType::RetryAfter,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::ResultHint(metadata) => (
            NnrpEventKind::ResultHint,
            0,
            None,
            MessageType::ResultHint,
            metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
        NnrpClientEvent::ObjectDeclare { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            0,
            None,
            MessageType::ObjectDeclare,
            metadata
                .to_vec_with_extension(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::ObjectRef { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            0,
            Some(metadata.operation_id),
            MessageType::ObjectRef,
            metadata
                .to_vec_with_extension(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::ObjectRelease { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            0,
            Some(metadata.operation_id),
            MessageType::ObjectRelease,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::ObjectDelta {
            message_type,
            metadata,
            body,
        } => {
            let mut payload = metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec();
            payload.extend_from_slice(&body);
            (
                NnrpEventKind::RuntimeFrame,
                0,
                None,
                message_type,
                payload,
                false,
            )
        }
        NnrpClientEvent::CacheReference { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            0,
            None,
            MessageType::CacheReference,
            metadata
                .to_vec_with_extension(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::CacheMiss { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            0,
            None,
            MessageType::CacheMiss,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpClientEvent::CacheInvalidate(metadata) => (
            NnrpEventKind::RuntimeFrame,
            0,
            None,
            MessageType::CacheInvalidate,
            metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
    };

    let mut store = handle_store();
    let operation = if let Some(operation_id) = operation_id {
        find_operation_handle(&store, scope, Some(operation_id), None)?
    } else if frame_id != 0 {
        find_operation_handle(&store, scope, None, Some(frame_id)).unwrap_or(NnrpHandle::invalid())
    } else {
        NnrpHandle::invalid()
    };
    if matches!(
        kind,
        NnrpEventKind::ResultPushed | NnrpEventKind::ResultDropped | NnrpEventKind::PartialResult
    ) && operation.kind != NnrpHandleKind::Operation as u32
    {
        return Err(NnrpFfiStatus::invalid_handle(
            NnrpHandleKind::Operation as u32,
        ));
    }
    let frame_id = if frame_id != 0 {
        frame_id
    } else if operation.kind == NnrpHandleKind::Operation as u32 {
        match store.get(operation, NnrpHandleKind::Operation)? {
            NnrpFfiResource::Operation { frame_id, .. } => *frame_id,
            _ => 0,
        }
    } else {
        0
    };
    let (payload_owner, payload_view) = if payload.is_empty() {
        (NnrpHandle::invalid(), NnrpBufferView::empty())
    } else {
        insert_owned_buffer(&mut store, payload)?
    };
    if terminal && operation.kind == NnrpHandleKind::Operation as u32 {
        store.remove(operation, NnrpHandleKind::Operation)?;
    }
    Ok(NnrpEvent {
        kind: kind as u32,
        message_type: message_type as u32,
        connection,
        session: scope,
        operation,
        frame_id,
        payload_owner,
        payload: payload_view,
        ..NnrpEvent::none()
    })
}

#[cfg(not(test))]
fn find_operation_handle(
    store: &NnrpFfiHandleStore,
    session: NnrpHandle,
    operation_id: Option<u64>,
    frame_id: Option<u32>,
) -> Result<NnrpHandle, NnrpFfiStatus> {
    store
        .entries
        .iter()
        .find_map(|((kind, id), entry)| match &entry.resource {
            NnrpFfiResource::Operation {
                session: owned_session,
                operation_id: owned_operation,
                frame_id: owned_frame,
                ..
            } if *owned_session == session
                && operation_id.is_none_or(|value| value == *owned_operation)
                && frame_id.is_none_or(|value| value == *owned_frame) =>
            {
                Some(NnrpHandle {
                    kind: *kind,
                    id: *id,
                    generation: entry.generation,
                    flags: 0,
                })
            }
            _ => None,
        })
        .ok_or_else(|| NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32))
}

#[cfg(not(test))]
fn role_session_connection(
    session: NnrpHandle,
    expected_role: NnrpFfiConnectionRole,
) -> Result<NnrpHandle, NnrpFfiStatus> {
    let store = handle_store();
    let connection = match store.get(session, NnrpHandleKind::Session)? {
        NnrpFfiResource::Session { connection, .. } => *connection,
        _ => {
            return Err(NnrpFfiStatus::invalid_handle(
                NnrpHandleKind::Session as u32,
            ))
        }
    };
    if store.get_connection_role(connection)? != expected_role {
        return Err(NnrpFfiStatus::invalid_handle(
            NnrpHandleKind::Session as u32,
        ));
    }
    Ok(connection)
}

#[cfg(not(test))]
unsafe fn role_client_await_events_impl(
    request: NnrpRoleEventPollRequest,
    out_events: *mut NnrpEvent,
    event_capacity: usize,
    out_event_count: *mut usize,
) -> NnrpFfiStatus {
    let limit = match validate_role_event_poll(request, out_events, event_capacity, out_event_count)
    {
        Ok(limit) => limit,
        Err(status) => return status,
    };
    *out_event_count = 0;
    let session = {
        let store = handle_store();
        match store.get(request.scope, NnrpHandleKind::Session) {
            Ok(NnrpFfiResource::Session {
                runtime: NnrpFfiSessionRuntime::Client(session),
                ..
            }) => Arc::clone(session),
            Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
            Err(status) => return status,
        }
    };

    for index in 0..limit {
        let timeout_ms = if index == 0 { request.timeout_ms } else { 1 };
        let runtime = Arc::clone(&session);
        let event = match transport::run_role_async(
            async move { runtime.lock().await.await_event().await },
            timeout_ms,
        ) {
            Ok(event) => event,
            Err(status)
                if index > 0 && status.status_code == NnrpFfiStatusCode::WouldBlock as u32 =>
            {
                break;
            }
            Err(status) => return status,
        };
        let event = match client_role_event(request.scope, event) {
            Ok(event) => event,
            Err(status) => return status,
        };
        *out_events.add(index) = event;
        *out_event_count += 1;
    }
    NnrpFfiStatus::ok()
}

#[cfg(not(test))]
unsafe fn role_server_await_events_impl(
    request: NnrpRoleEventPollRequest,
    out_events: *mut NnrpEvent,
    event_capacity: usize,
    out_event_count: *mut usize,
) -> NnrpFfiStatus {
    let limit = match validate_role_event_poll(request, out_events, event_capacity, out_event_count)
    {
        Ok(limit) => limit,
        Err(status) => return status,
    };
    *out_event_count = 0;
    let (session, connection) = {
        let store = handle_store();
        match store.get(request.scope, NnrpHandleKind::Session) {
            Ok(NnrpFfiResource::Session {
                connection,
                runtime: NnrpFfiSessionRuntime::Server(session),
                ..
            }) => (Arc::clone(session), *connection),
            Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
            Err(status) => return status,
        }
    };
    if let Err(status) = role_session_connection(request.scope, NnrpFfiConnectionRole::Server) {
        return status;
    }

    for index in 0..limit {
        let timeout_ms = if index == 0 { request.timeout_ms } else { 1 };
        let runtime = Arc::clone(&session);
        let event = match transport::run_role_async(
            async move { runtime.lock().await.await_event().await },
            timeout_ms,
        ) {
            Ok(event) => event,
            Err(status)
                if index > 0 && status.status_code == NnrpFfiStatusCode::WouldBlock as u32 =>
            {
                break;
            }
            Err(status) => return status,
        };
        *out_events.add(index) = match server_role_event(request.scope, connection, event) {
            Ok(event) => event,
            Err(status) => return status,
        };
        *out_event_count += 1;
    }
    NnrpFfiStatus::ok()
}

#[cfg(not(test))]
fn server_role_event(
    scope: NnrpHandle,
    connection: NnrpHandle,
    event: NnrpServerEvent,
) -> Result<NnrpEvent, NnrpFfiStatus> {
    let (kind, message_type, operation_id, frame_id, payload, create_operation) = match event {
        NnrpServerEvent::Submit(submit) => {
            let mut payload = submit
                .metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec();
            payload.extend_from_slice(&submit.body);
            (
                NnrpEventKind::SubmitAccepted,
                MessageType::FrameSubmit,
                Some(submit.operation_id),
                submit.frame_id,
                payload,
                true,
            )
        }
        NnrpServerEvent::FrameCancel(cancel) => (
            NnrpEventKind::Control,
            MessageType::FrameCancel,
            None,
            cancel.frame_id,
            Vec::new(),
            false,
        ),
        NnrpServerEvent::PartialResult { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            MessageType::PartialResult,
            Some(metadata.operation_id),
            0,
            metadata
                .to_vec_with_body(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::Progress { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            MessageType::Progress,
            Some(metadata.operation_id),
            0,
            metadata
                .to_vec_with_body(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::ResultDropReason { metadata, body } => (
            NnrpEventKind::ResultDropped,
            MessageType::ResultDropReason,
            Some(metadata.operation_id),
            0,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::Control(control) => (
            NnrpEventKind::RuntimeFrame,
            control.message_type,
            Some(control.metadata.operation_id),
            0,
            control
                .metadata
                .to_vec_with_diagnostics(&control.body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::Scheduling(update) => (
            NnrpEventKind::RuntimeFrame,
            update.message_type,
            Some(update.metadata.operation_id),
            0,
            update
                .metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
        NnrpServerEvent::Supersede { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            MessageType::Supersede,
            Some(metadata.old_operation_id),
            0,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::Budget(metadata) => (
            NnrpEventKind::RuntimeFrame,
            MessageType::BudgetUpdate,
            Some(metadata.operation_id),
            0,
            metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
        NnrpServerEvent::FlowUpdate(metadata) => (
            NnrpEventKind::FlowUpdated,
            MessageType::FlowUpdate,
            None,
            0,
            metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
        NnrpServerEvent::Pressure(update) => (
            NnrpEventKind::RuntimeFrame,
            update.message_type,
            None,
            0,
            update
                .metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
        NnrpServerEvent::Capability {
            message_type,
            metadata,
            body,
        } => (
            NnrpEventKind::RuntimeFrame,
            message_type,
            None,
            0,
            metadata
                .to_vec_with_body(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::RouteHint {
            message_type,
            metadata,
            body,
        } => (
            NnrpEventKind::RuntimeFrame,
            message_type,
            (metadata.operation_id != 0).then_some(metadata.operation_id),
            0,
            metadata
                .to_vec_with_body(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::TraceContext {
            frame_id,
            metadata,
            body,
        } => (
            NnrpEventKind::RuntimeFrame,
            MessageType::TraceContext,
            None,
            frame_id,
            metadata
                .to_vec_with_body(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::RecoverableError { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            MessageType::ErrorRecoverable,
            None,
            metadata.related_frame_id,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::RetryAfter { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            MessageType::RetryAfter,
            None,
            0,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::ObjectDeclare { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            MessageType::ObjectDeclare,
            None,
            0,
            metadata
                .to_vec_with_extension(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::ObjectRef { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            MessageType::ObjectRef,
            Some(metadata.operation_id),
            0,
            metadata
                .to_vec_with_extension(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::ObjectRelease { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            MessageType::ObjectRelease,
            Some(metadata.operation_id),
            0,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::ObjectDelta {
            message_type,
            metadata,
            body,
        } => {
            let mut payload = metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec();
            payload.extend_from_slice(&body);
            (
                NnrpEventKind::RuntimeFrame,
                message_type,
                None,
                0,
                payload,
                false,
            )
        }
        NnrpServerEvent::CacheReference { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            MessageType::CacheReference,
            None,
            0,
            metadata
                .to_vec_with_extension(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::CacheMiss { metadata, body } => (
            NnrpEventKind::RuntimeFrame,
            MessageType::CacheMiss,
            None,
            0,
            metadata
                .to_vec_with_diagnostics(&body)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?,
            false,
        ),
        NnrpServerEvent::CacheInvalidate(metadata) => (
            NnrpEventKind::RuntimeFrame,
            MessageType::CacheInvalidate,
            None,
            0,
            metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
        NnrpServerEvent::Close(metadata) => (
            NnrpEventKind::SessionClosed,
            MessageType::SessionClose,
            None,
            0,
            metadata
                .to_bytes()
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?
                .to_vec(),
            false,
        ),
    };

    let mut store = handle_store();
    let operation = if create_operation {
        let operation = NnrpHandle::new(
            NnrpHandleKind::Operation,
            next_handle_id(&mut store, NnrpHandleKind::Operation),
            scope.generation,
        );
        store.insert(
            operation,
            NnrpFfiResource::Operation {
                session: scope,
                operation_id: operation_id.expect("submit event has operation id"),
                frame_id,
                payload_len: payload.len(),
            },
        )?;
        operation
    } else if let Some(operation_id) = operation_id {
        find_operation_handle(&store, scope, Some(operation_id), None)?
    } else if frame_id != 0 {
        find_operation_handle(&store, scope, None, Some(frame_id)).unwrap_or(NnrpHandle::invalid())
    } else {
        NnrpHandle::invalid()
    };
    let frame_id = if frame_id != 0 {
        frame_id
    } else if operation.kind == NnrpHandleKind::Operation as u32 {
        match store.get(operation, NnrpHandleKind::Operation)? {
            NnrpFfiResource::Operation { frame_id, .. } => *frame_id,
            _ => 0,
        }
    } else {
        0
    };
    let (payload_owner, payload_view) = if payload.is_empty() {
        (NnrpHandle::invalid(), NnrpBufferView::empty())
    } else {
        match insert_owned_buffer(&mut store, payload) {
            Ok(value) => value,
            Err(status) => {
                if create_operation {
                    store.entries.remove(&(operation.kind, operation.id));
                }
                return Err(status);
            }
        }
    };
    Ok(NnrpEvent {
        kind: kind as u32,
        message_type: message_type as u32,
        connection,
        session: scope,
        operation,
        frame_id,
        payload_owner,
        payload: payload_view,
        ..NnrpEvent::none()
    })
}

const fn poll_result_none(status: NnrpFfiStatus) -> NnrpPollResult {
    NnrpPollResult {
        status,
        has_event: 0,
        event: NnrpEvent::none(),
    }
}

#[cfg(test)]
unsafe fn poll_matching_operation_result(
    session: NnrpHandle,
    operation: NnrpHandle,
    frame_id: u32,
    max_events: usize,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    let mut store = handle_store();
    let connection = match store.get(session, NnrpHandleKind::Session) {
        Ok(NnrpFfiResource::Session { connection, .. }) => *connection,
        Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
        Err(status) => return status,
    };
    let mut seen_events = 0usize;
    while max_events == 0 || seen_events < max_events {
        match store.poll_event(connection) {
            Ok(Some(event)) => {
                seen_events += 1;
                if event_is_operation_result(event, session, operation, frame_id) {
                    *out_result = NnrpPollResult {
                        status: NnrpFfiStatus::ok(),
                        has_event: 1,
                        event,
                    };
                    return NnrpFfiStatus::ok();
                }
            }
            Ok(None) => {
                *out_result = NnrpPollResult {
                    status: NnrpFfiStatus {
                        status_code: NnrpFfiStatusCode::WouldBlock as u32,
                        error_family: NnrpErrorFamily::None as u32,
                        protocol_error_code: 0,
                        detail_code: 0,
                    },
                    has_event: 0,
                    event: NnrpEvent::none(),
                };
                return (*out_result).status;
            }
            Err(status) => return status,
        }
    }
    *out_result = NnrpPollResult {
        status: NnrpFfiStatus {
            status_code: NnrpFfiStatusCode::WouldBlock as u32,
            error_family: NnrpErrorFamily::None as u32,
            protocol_error_code: 0,
            detail_code: 0,
        },
        has_event: 0,
        event: NnrpEvent::none(),
    };
    (*out_result).status
}

#[cfg(test)]
unsafe fn poll_matching_operation_event_from_scope(
    scope: OperationEventScope,
    operation: NnrpHandle,
    operation_id: u64,
    event_kind: NnrpEventKind,
    payload: NnrpBufferView,
    max_events: usize,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    let mut store = handle_store();
    let mut seen_events = 0usize;
    while max_events == 0 || seen_events < max_events {
        match store.poll_event(scope.connection) {
            Ok(Some(mut event)) => {
                seen_events += 1;
                if event.kind == event_kind as u32
                    && event.session == scope.session
                    && event.operation.id == operation.id
                    && event.operation.generation == operation.generation
                    && (event.operation.id == operation_id || event.frame_id == scope.frame_id)
                {
                    event.payload = payload;
                    *out_result = NnrpPollResult {
                        status: NnrpFfiStatus::ok(),
                        has_event: 1,
                        event,
                    };
                    return NnrpFfiStatus::ok();
                }
            }
            Ok(None) => {
                let status = NnrpFfiStatus {
                    status_code: NnrpFfiStatusCode::WouldBlock as u32,
                    error_family: NnrpErrorFamily::None as u32,
                    protocol_error_code: 0,
                    detail_code: 0,
                };
                *out_result = poll_result_none(status);
                return status;
            }
            Err(status) => {
                *out_result = poll_result_none(status);
                return status;
            }
        }
    }

    let status = NnrpFfiStatus {
        status_code: NnrpFfiStatusCode::WouldBlock as u32,
        error_family: NnrpErrorFamily::None as u32,
        protocol_error_code: 0,
        detail_code: 0,
    };
    *out_result = poll_result_none(status);
    status
}

#[cfg(test)]
unsafe fn poll_matching_control_event(
    handle: NnrpHandle,
    event_kind: NnrpEventKind,
    max_events: usize,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    let mut store = handle_store();
    let (connection, session, operation) = match event_scope_for_handle(&store, handle) {
        Ok(scope) => scope,
        Err(status) => {
            *out_result = poll_result_none(status);
            return status;
        }
    };

    let mut seen_events = 0usize;
    while max_events == 0 || seen_events < max_events {
        match store.poll_event(connection) {
            Ok(Some(event)) => {
                seen_events += 1;
                if event.kind == event_kind as u32
                    && event.connection == connection
                    && event.session == session
                    && event.operation == operation
                {
                    *out_result = NnrpPollResult {
                        status: NnrpFfiStatus::ok(),
                        has_event: 1,
                        event,
                    };
                    return NnrpFfiStatus::ok();
                }
            }
            Ok(None) => {
                let status = NnrpFfiStatus {
                    status_code: NnrpFfiStatusCode::WouldBlock as u32,
                    error_family: NnrpErrorFamily::None as u32,
                    protocol_error_code: 0,
                    detail_code: 0,
                };
                *out_result = poll_result_none(status);
                return status;
            }
            Err(status) => {
                *out_result = poll_result_none(status);
                return status;
            }
        }
    }

    let status = NnrpFfiStatus {
        status_code: NnrpFfiStatusCode::WouldBlock as u32,
        error_family: NnrpErrorFamily::None as u32,
        protocol_error_code: 0,
        detail_code: 0,
    };
    *out_result = poll_result_none(status);
    status
}

#[cfg(any(test, feature = "benchmark-ffi"))]
unsafe fn poll_matching_operation_compact_result(
    session: NnrpHandle,
    operation: NnrpHandle,
    operation_id: u64,
    frame_id: u32,
    payload: NnrpBufferView,
    max_events: usize,
    out_result: *mut NnrpCompactResult,
) -> NnrpFfiStatus {
    let mut store = handle_store();
    let connection = match store.get(session, NnrpHandleKind::Session) {
        Ok(NnrpFfiResource::Session { connection, .. }) => *connection,
        Ok(_) => {
            let status = NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32);
            *out_result = NnrpCompactResult::none(status);
            return status;
        }
        Err(status) => {
            *out_result = NnrpCompactResult::none(status);
            return status;
        }
    };
    let mut seen_events = 0usize;
    while max_events == 0 || seen_events < max_events {
        match store.poll_event(connection) {
            Ok(Some(event)) => {
                seen_events += 1;
                if event_is_operation_result(event, session, operation, frame_id) {
                    let mut result =
                        NnrpCompactResult::from_event(NnrpFfiStatus::ok(), event, operation_id);
                    result.payload = payload;
                    *out_result = result;
                    return NnrpFfiStatus::ok();
                }
            }
            Ok(None) => {
                let status = NnrpFfiStatus {
                    status_code: NnrpFfiStatusCode::WouldBlock as u32,
                    error_family: NnrpErrorFamily::None as u32,
                    protocol_error_code: 0,
                    detail_code: 0,
                };
                *out_result = NnrpCompactResult::none(status);
                return status;
            }
            Err(status) => {
                *out_result = NnrpCompactResult::none(status);
                return status;
            }
        }
    }
    let status = NnrpFfiStatus {
        status_code: NnrpFfiStatusCode::WouldBlock as u32,
        error_family: NnrpErrorFamily::None as u32,
        protocol_error_code: 0,
        detail_code: 0,
    };
    *out_result = NnrpCompactResult::none(status);
    status
}

fn event_scope_for_handle(
    store: &NnrpFfiHandleStore,
    handle: NnrpHandle,
) -> Result<(NnrpHandle, NnrpHandle, NnrpHandle), NnrpFfiStatus> {
    match handle.kind {
        value if value == NnrpHandleKind::Connection as u32 => {
            match store.get(handle, NnrpHandleKind::Connection) {
                Ok(NnrpFfiResource::Connection { .. }) => {
                    Ok((handle, NnrpHandle::invalid(), NnrpHandle::invalid()))
                }
                Ok(_) => Err(NnrpFfiStatus::invalid_handle(
                    NnrpHandleKind::Connection as u32,
                )),
                Err(status) => Err(status),
            }
        }
        value if value == NnrpHandleKind::Session as u32 => {
            match store.get(handle, NnrpHandleKind::Session) {
                Ok(NnrpFfiResource::Session { connection, .. }) => {
                    Ok((*connection, handle, NnrpHandle::invalid()))
                }
                Ok(_) => Err(NnrpFfiStatus::invalid_handle(
                    NnrpHandleKind::Session as u32,
                )),
                Err(status) => Err(status),
            }
        }
        value if value == NnrpHandleKind::Operation as u32 => {
            match store.get(handle, NnrpHandleKind::Operation) {
                Ok(NnrpFfiResource::Operation { session, .. }) => {
                    let session = *session;
                    match store.get(session, NnrpHandleKind::Session) {
                        Ok(NnrpFfiResource::Session { connection, .. }) => {
                            Ok((*connection, session, handle))
                        }
                        Ok(_) => Err(NnrpFfiStatus::invalid_handle(
                            NnrpHandleKind::Session as u32,
                        )),
                        Err(status) => Err(status),
                    }
                }
                Ok(_) => Err(NnrpFfiStatus::invalid_handle(
                    NnrpHandleKind::Operation as u32,
                )),
                Err(status) => Err(status),
            }
        }
        _ => Err(NnrpFfiStatus::invalid_handle(handle.kind)),
    }
}

#[cfg(any(test, feature = "benchmark-ffi"))]
fn compact_result_state(status: NnrpFfiStatus, event_kind: u32) -> u32 {
    if status.status_code != NnrpFfiStatusCode::Ok as u32
        || event_kind == NnrpEventKind::Error as u32
    {
        return NNRP_RESULT_STATE_FAILED;
    }
    if event_kind == NnrpEventKind::ResultDropped as u32 {
        return NNRP_RESULT_STATE_CANCELLED;
    }
    if event_kind == NnrpEventKind::ResultPushed as u32 {
        return NNRP_RESULT_STATE_COMPLETED;
    }
    NNRP_RESULT_STATE_NONE
}

#[cfg(any(test, feature = "benchmark-ffi"))]
fn event_is_operation_result(
    event: NnrpEvent,
    session: NnrpHandle,
    operation: NnrpHandle,
    frame_id: u32,
) -> bool {
    matches!(
        event.kind,
        value if value == NnrpEventKind::ResultPushed as u32
            || value == NnrpEventKind::ResultDropped as u32
            || value == NnrpEventKind::Error as u32
    ) && event.session == session
        && (event.operation.id == operation.id || event.frame_id == frame_id)
}

fn validate_control_metadata_payload(
    control_code: u32,
    payload: NnrpBufferView,
) -> Result<(), NnrpFfiStatus> {
    payload.validate()?;
    let control_code =
        u8::try_from(control_code).map_err(|_| NnrpFfiStatus::invalid_argument(128))?;
    let message_type = MessageType::try_from_u8(control_code)
        .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
    let payload = unsafe { ffi_read_slice(payload) };

    match message_type {
        MessageType::Cancel | MessageType::Abort => {
            let (metadata, _) = ControlRequestMetadata::parse_with_diagnostics(payload)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
            validate_control_request_semantics(message_type, &metadata)
        }
        MessageType::PriorityUpdate | MessageType::Deadline | MessageType::ExpireAt => {
            let metadata = SchedulingMetadata::parse(payload)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
            validate_scheduling_semantics(message_type, &metadata)
        }
        MessageType::Supersede => SupersedeMetadata::parse_with_diagnostics(payload).map(|_| ()),
        MessageType::BudgetUpdate => BudgetMetadata::parse(payload).map(|_| ()),
        MessageType::Progress => {
            let (metadata, _) = ProgressMetadata::parse_with_body(payload)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
            validate_progress_semantics(&metadata)
        }
        MessageType::PartialResult => {
            let (metadata, _) = PartialResultMetadata::parse_with_body(payload)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
            validate_partial_result_semantics(&metadata)
        }
        MessageType::Backpressure | MessageType::CreditUpdate => {
            let metadata = PressureMetadata::parse(payload)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
            validate_pressure_semantics(message_type, &metadata)
        }
        MessageType::CapabilityNegotiation | MessageType::DegradeProfile => {
            CapabilityMetadata::parse_with_body(payload).map(|_| ())
        }
        MessageType::RouteHint | MessageType::ExecutionHint => {
            RouteHintMetadata::parse_with_body(payload).map(|_| ())
        }
        MessageType::TraceContext => {
            let (metadata, _) = TraceContextMetadata::parse_with_body(payload)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
            validate_trace_context_semantics(&metadata)
        }
        MessageType::ResultDropReason => {
            let (metadata, _) = ResultDropReasonMetadata::parse_with_diagnostics(payload)
                .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
            validate_result_drop_reason_semantics(&metadata)
        }
        MessageType::ErrorRecoverable => {
            RecoverableErrorMetadata::parse_with_diagnostics(payload).map(|_| ())
        }
        MessageType::RetryAfter => RetryAfterMetadata::parse_with_diagnostics(payload).map(|_| ()),
        MessageType::ResultHint => ResultHintMetadata::parse(payload).map(|_| ()),
        _ => Err(NnrpError::InvalidProtocolCombination {
            rule: "control metadata submit requires a control metadata message type",
        }),
    }
    .map_err(|error| NnrpFfiStatus::from_core_error(&error))
}

fn validate_runtime_frame_payload(
    message_type: u32,
    payload: NnrpBufferView,
) -> Result<MessageType, NnrpFfiStatus> {
    payload.validate()?;
    let message_code =
        u8::try_from(message_type).map_err(|_| NnrpFfiStatus::invalid_argument(140))?;
    let message_type = MessageType::try_from_u8(message_code)
        .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
    let bytes = unsafe { ffi_read_slice(payload) };

    let result = match message_type {
        MessageType::Cancel
        | MessageType::Abort
        | MessageType::PriorityUpdate
        | MessageType::Deadline
        | MessageType::ExpireAt
        | MessageType::Supersede
        | MessageType::BudgetUpdate
        | MessageType::Progress
        | MessageType::PartialResult
        | MessageType::Backpressure
        | MessageType::CreditUpdate
        | MessageType::CapabilityNegotiation
        | MessageType::DegradeProfile
        | MessageType::RouteHint
        | MessageType::ExecutionHint
        | MessageType::TraceContext
        | MessageType::ResultDropReason
        | MessageType::ErrorRecoverable
        | MessageType::RetryAfter => {
            validate_control_metadata_payload(message_type as u32, payload)
        }
        MessageType::ResultHint => ResultHintMetadata::parse(bytes)
            .map(|_| ())
            .map_err(|error| NnrpFfiStatus::from_core_error(&error)),
        MessageType::FlowUpdate => FlowUpdateMetadata::parse(bytes)
            .map(|_| ())
            .map_err(|error| NnrpFfiStatus::from_core_error(&error)),
        MessageType::ObjectDeclare => ObjectDescriptorMetadata::parse_with_extension(bytes)
            .map(|_| ())
            .map_err(|error| NnrpFfiStatus::from_core_error(&error)),
        MessageType::ObjectRef => ObjectReferenceMetadata::parse_with_extension(bytes)
            .map(|_| ())
            .map_err(|error| NnrpFfiStatus::from_core_error(&error)),
        MessageType::ObjectRelease => ObjectReleaseMetadata::parse_with_diagnostics(bytes)
            .map(|_| ())
            .map_err(|error| NnrpFfiStatus::from_core_error(&error)),
        MessageType::ObjectPatch | MessageType::ObjectDelta => validate_object_delta_payload(bytes),
        MessageType::CacheReference => CacheReferenceMetadata::parse_with_extension(bytes)
            .map(|_| ())
            .map_err(|error| NnrpFfiStatus::from_core_error(&error)),
        MessageType::CacheMiss => CacheMissMetadata::parse_with_diagnostics(bytes)
            .map(|_| ())
            .map_err(|error| NnrpFfiStatus::from_core_error(&error)),
        MessageType::CacheInvalidate => CacheInvalidateMetadata::parse(bytes)
            .map(|_| ())
            .map_err(|error| NnrpFfiStatus::from_core_error(&error)),
        _ => Err(NnrpFfiStatus::from_core_error(
            &NnrpError::InvalidProtocolCombination {
                rule: "runtime frame send requires a preview4 control, object, or cache message",
            },
        )),
    };
    result.map(|_| message_type)
}

fn validate_object_delta_payload(bytes: &[u8]) -> Result<(), NnrpFfiStatus> {
    let metadata = ObjectDeltaMetadata::parse(bytes)
        .map_err(|error| NnrpFfiStatus::from_core_error(&error))?;
    let declared = OBJECT_DELTA_METADATA_LEN
        .checked_add(metadata.metadata_bytes as usize)
        .and_then(|length| length.checked_add(metadata.delta_bytes as usize))
        .ok_or_else(|| NnrpFfiStatus::from_core_error(&NnrpError::MessageLengthOverflow))?;
    if declared != bytes.len() {
        return Err(NnrpFfiStatus::from_core_error(
            &NnrpError::DeclaredLengthMismatch {
                field: "object_delta.payload_bytes",
                declared,
                actual: bytes.len(),
            },
        ));
    }
    let metadata_end = OBJECT_DELTA_METADATA_LEN + metadata.metadata_bytes as usize;
    ObjectDeltaMetadata::parse_with_extension(&bytes[..metadata_end])
        .map(|_| ())
        .map_err(|error| NnrpFfiStatus::from_core_error(&error))
}

fn validate_runtime_frame_direction(
    role: NnrpFfiConnectionRole,
    message_type: MessageType,
) -> Result<(), NnrpFfiStatus> {
    if message_type == MessageType::ResultHint && role != NnrpFfiConnectionRole::Server {
        return Err(NnrpFfiStatus::from_core_error(
            &NnrpError::InvalidProtocolCombination {
                rule: "RESULT_HINT is sent from server to client",
            },
        ));
    }
    Ok(())
}

#[cfg(test)]
fn control_event_kind(control_code: u32) -> NnrpEventKind {
    if control_code == MessageType::ResultHint as u32 {
        NnrpEventKind::ResultHint
    } else {
        NnrpEventKind::Control
    }
}

unsafe fn ffi_read_slice<'a>(view: NnrpBufferView) -> &'a [u8] {
    if view.len == 0 {
        &[]
    } else {
        core::slice::from_raw_parts(view.ptr, view.len)
    }
}

unsafe fn ffi_write_slice<'a>(view: NnrpBufferViewMut) -> &'a mut [u8] {
    if view.len == 0 {
        &mut []
    } else {
        core::slice::from_raw_parts_mut(view.ptr, view.len)
    }
}

fn next_handle_id(store: &mut NnrpFfiHandleStore, kind: NnrpHandleKind) -> u64 {
    let kind = kind as u32;
    let next_id = store.next_ids.entry(kind).or_default();
    loop {
        *next_id = next_id.wrapping_add(1);
        if *next_id != 0 && !store.entries.contains_key(&(kind, *next_id)) {
            return *next_id;
        }
    }
}

#[no_mangle]
/// # Safety
///
/// `source` must remain readable for `source.len` bytes for the duration of
/// the call. `out_buffer` and `out_view` must be either null or valid writable
/// pointers to one value each.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_buffer_acquire_copy(source: NnrpBufferView, out_buffer: *mut NnrpHandle, out_view: *mut NnrpBufferView) -> NnrpFfiStatus {
    nnrp_buffer_acquire_copy_impl(source, out_buffer, out_view)
}

unsafe fn nnrp_buffer_acquire_copy_impl(
    source: NnrpBufferView,
    out_buffer: *mut NnrpHandle,
    out_view: *mut NnrpBufferView,
) -> NnrpFfiStatus {
    if out_buffer.is_null() || out_view.is_null() {
        return NnrpFfiStatus::invalid_argument(44);
    }
    if let Err(status) = source.validate() {
        return status;
    }
    let bytes = ffi_read_slice(source).to_vec();
    let mut store = handle_store();
    let (handle, view) = match insert_owned_buffer(&mut store, bytes) {
        Ok(result) => result,
        Err(status) => return status,
    };
    *out_buffer = handle;
    *out_view = view;
    NnrpFfiStatus::ok()
}

fn insert_owned_buffer(
    store: &mut NnrpFfiHandleStore,
    bytes: Vec<u8>,
) -> Result<(NnrpHandle, NnrpBufferView), NnrpFfiStatus> {
    let id = next_handle_id(store, NnrpHandleKind::Buffer);
    let handle = NnrpHandle::new(NnrpHandleKind::Buffer, id, 1);
    store.insert(handle, NnrpFfiResource::Buffer { bytes })?;
    let view = match store.get(handle, NnrpHandleKind::Buffer) {
        Ok(NnrpFfiResource::Buffer { bytes }) => NnrpBufferView {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
        },
        _ => return Err(NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)),
    };
    Ok((handle, view))
}

pub(crate) fn store_owned_buffer(
    bytes: Vec<u8>,
) -> Result<(NnrpHandle, NnrpBufferView), NnrpFfiStatus> {
    insert_owned_buffer(&mut handle_store(), bytes)
}

#[no_mangle]
/// # Safety
///
/// `out_view` must be either null or a valid writable pointer to one
/// `NnrpBufferView`.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_buffer_view(buffer: NnrpHandle, out_view: *mut NnrpBufferView) -> NnrpFfiStatus { nnrp_buffer_view_impl(buffer, out_view) }

unsafe fn nnrp_buffer_view_impl(
    buffer: NnrpHandle,
    out_view: *mut NnrpBufferView,
) -> NnrpFfiStatus {
    if out_view.is_null() {
        return NnrpFfiStatus::invalid_argument(45);
    }
    let store = handle_store();
    match store.get(buffer, NnrpHandleKind::Buffer) {
        Ok(NnrpFfiResource::Buffer { bytes }) => {
            *out_view = NnrpBufferView {
                ptr: bytes.as_ptr(),
                len: bytes.len(),
            };
            NnrpFfiStatus::ok()
        }
        Ok(_) => NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32),
        Err(status) => status,
    }
}

#[no_mangle]
/// # Safety
///
/// The buffer handle is copied by value. This function does not dereference
/// caller-provided pointers.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_buffer_release(buffer: NnrpHandle) -> NnrpFfiStatus { nnrp_buffer_release_impl(buffer) }

fn nnrp_buffer_release_impl(buffer: NnrpHandle) -> NnrpFfiStatus {
    let mut store = handle_store();
    match store.get(buffer, NnrpHandleKind::Buffer) {
        Ok(NnrpFfiResource::Buffer { .. }) => store
            .remove(buffer, NnrpHandleKind::Buffer)
            .map(|_| NnrpFfiStatus::ok())
            .unwrap_or_else(|status| status),
        Ok(_) => NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32),
        Err(status) => status,
    }
}

#[no_mangle]
/// # Safety
///
/// `source` must remain readable for `source.len` bytes for the duration of
/// the call. `out_buffer` and `out_view` must be either null or valid writable
/// pointers to one value each.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_object_metadata_buffer_acquire_copy(source: NnrpBufferView, out_buffer: *mut NnrpHandle, out_view: *mut NnrpBufferView) -> NnrpFfiStatus {
    nnrp_buffer_acquire_copy_impl(source, out_buffer, out_view)
}

#[no_mangle]
/// # Safety
///
/// `out_view` must be either null or a valid writable pointer to one
/// `NnrpBufferView`.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_object_metadata_buffer_view(buffer: NnrpHandle, out_view: *mut NnrpBufferView) -> NnrpFfiStatus {
    nnrp_buffer_view_impl(buffer, out_view)
}

#[no_mangle]
/// # Safety
///
/// The buffer handle is copied by value. This function does not dereference
/// caller-provided pointers.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_object_metadata_buffer_release(buffer: NnrpHandle) -> NnrpFfiStatus {
    nnrp_buffer_release_impl(buffer)
}

#[no_mangle]
/// # Safety
///
/// `metadata` must remain readable for `metadata.len` bytes for the duration of
/// the call. `out_handle` must be either null or a valid writable pointer to
/// one `NnrpHandle`.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_object_descriptor_create(descriptor: NnrpRuntimeObjectDescriptor, metadata: NnrpBufferView, out_handle: *mut NnrpHandle) -> NnrpFfiStatus {
    nnrp_object_descriptor_create_impl(descriptor, metadata, out_handle)
}

unsafe fn nnrp_object_descriptor_create_impl(
    descriptor: NnrpRuntimeObjectDescriptor,
    metadata: NnrpBufferView,
    out_handle: *mut NnrpHandle,
) -> NnrpFfiStatus {
    if out_handle.is_null() {
        return NnrpFfiStatus::invalid_argument(50);
    }
    if descriptor.metadata_bytes as usize != metadata.len {
        return NnrpFfiStatus::invalid_argument(51);
    }
    if let Err(status) = metadata.validate() {
        return status;
    }
    let descriptor = match descriptor.to_core() {
        Ok(descriptor) => descriptor,
        Err(status) => return status,
    };
    let metadata = ffi_read_slice(metadata).to_vec();
    let mut store = handle_store();
    let id = next_handle_id(&mut store, NnrpHandleKind::ObjectDescriptor);
    let handle = NnrpHandle::new(NnrpHandleKind::ObjectDescriptor, id, 1);
    if let Err(status) = store.insert(
        handle,
        NnrpFfiResource::ObjectDescriptor {
            descriptor,
            metadata,
        },
    ) {
        return status;
    }
    *out_handle = handle;
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// `out_descriptor` and `out_metadata` must be either null or valid writable
/// pointers to one value each.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_object_descriptor_view(handle: NnrpHandle, out_descriptor: *mut NnrpRuntimeObjectDescriptor, out_metadata: *mut NnrpBufferView) -> NnrpFfiStatus {
    nnrp_object_descriptor_view_impl(handle, out_descriptor, out_metadata)
}

unsafe fn nnrp_object_descriptor_view_impl(
    handle: NnrpHandle,
    out_descriptor: *mut NnrpRuntimeObjectDescriptor,
    out_metadata: *mut NnrpBufferView,
) -> NnrpFfiStatus {
    if out_descriptor.is_null() || out_metadata.is_null() {
        return NnrpFfiStatus::invalid_argument(52);
    }
    let store = handle_store();
    match store.get(handle, NnrpHandleKind::ObjectDescriptor) {
        Ok(NnrpFfiResource::ObjectDescriptor {
            descriptor,
            metadata,
        }) => {
            *out_descriptor = (*descriptor).into();
            *out_metadata = NnrpBufferView {
                ptr: metadata.as_ptr(),
                len: metadata.len(),
            };
            NnrpFfiStatus::ok()
        }
        Ok(_) => NnrpFfiStatus::invalid_handle(NnrpHandleKind::ObjectDescriptor as u32),
        Err(status) => status,
    }
}

#[no_mangle]
/// # Safety
///
/// `out_buffer` and `out_view` must be either null or valid writable pointers
/// to one value each.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_object_descriptor_metadata_snapshot(handle: NnrpHandle, out_buffer: *mut NnrpHandle, out_view: *mut NnrpBufferView) -> NnrpFfiStatus {
    nnrp_object_descriptor_metadata_snapshot_impl(handle, out_buffer, out_view)
}

unsafe fn nnrp_object_descriptor_metadata_snapshot_impl(
    handle: NnrpHandle,
    out_buffer: *mut NnrpHandle,
    out_view: *mut NnrpBufferView,
) -> NnrpFfiStatus {
    if out_buffer.is_null() || out_view.is_null() {
        return NnrpFfiStatus::invalid_argument(56);
    }
    let mut store = handle_store();
    let metadata = match store.get(handle, NnrpHandleKind::ObjectDescriptor) {
        Ok(NnrpFfiResource::ObjectDescriptor { metadata, .. }) => metadata.clone(),
        Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::ObjectDescriptor as u32),
        Err(status) => return status,
    };
    let (buffer, view) = match insert_owned_buffer(&mut store, metadata) {
        Ok(result) => result,
        Err(status) => return status,
    };
    *out_buffer = buffer;
    *out_view = view;
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// The descriptor handle is copied by value. This function does not dereference
/// caller-provided pointers.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_object_descriptor_release(handle: NnrpHandle) -> NnrpFfiStatus {
    nnrp_object_descriptor_release_impl(handle)
}

fn nnrp_object_descriptor_release_impl(handle: NnrpHandle) -> NnrpFfiStatus {
    let mut store = handle_store();
    match store.get(handle, NnrpHandleKind::ObjectDescriptor) {
        Ok(NnrpFfiResource::ObjectDescriptor { .. }) => store
            .remove(handle, NnrpHandleKind::ObjectDescriptor)
            .map(|_| NnrpFfiStatus::ok())
            .unwrap_or_else(|status| status),
        Ok(_) => NnrpFfiStatus::invalid_handle(NnrpHandleKind::ObjectDescriptor as u32),
        Err(status) => status,
    }
}

#[no_mangle]
/// # Safety
///
/// `metadata` must remain readable for `metadata.len` bytes for the duration of
/// the call. `out_handle` must be either null or a valid writable pointer to
/// one `NnrpHandle`.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_cache_reference_descriptor_create(descriptor: NnrpCacheReferenceDescriptor, metadata: NnrpBufferView, out_handle: *mut NnrpHandle) -> NnrpFfiStatus {
    nnrp_cache_reference_descriptor_create_impl(descriptor, metadata, out_handle)
}

unsafe fn nnrp_cache_reference_descriptor_create_impl(
    descriptor: NnrpCacheReferenceDescriptor,
    metadata: NnrpBufferView,
    out_handle: *mut NnrpHandle,
) -> NnrpFfiStatus {
    if out_handle.is_null() {
        return NnrpFfiStatus::invalid_argument(53);
    }
    if descriptor.metadata_bytes as usize != metadata.len {
        return NnrpFfiStatus::invalid_argument(54);
    }
    if let Err(status) = metadata.validate() {
        return status;
    }
    let descriptor = match descriptor.to_core() {
        Ok(descriptor) => descriptor,
        Err(status) => return status,
    };
    let metadata = ffi_read_slice(metadata).to_vec();
    let mut store = handle_store();
    let id = next_handle_id(&mut store, NnrpHandleKind::CacheReferenceDescriptor);
    let handle = NnrpHandle::new(NnrpHandleKind::CacheReferenceDescriptor, id, 1);
    if let Err(status) = store.insert(
        handle,
        NnrpFfiResource::CacheReferenceDescriptor {
            descriptor,
            metadata,
        },
    ) {
        return status;
    }
    *out_handle = handle;
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// `out_descriptor` and `out_metadata` must be either null or valid writable
/// pointers to one value each.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_cache_reference_descriptor_view(handle: NnrpHandle, out_descriptor: *mut NnrpCacheReferenceDescriptor, out_metadata: *mut NnrpBufferView) -> NnrpFfiStatus {
    nnrp_cache_reference_descriptor_view_impl(handle, out_descriptor, out_metadata)
}

unsafe fn nnrp_cache_reference_descriptor_view_impl(
    handle: NnrpHandle,
    out_descriptor: *mut NnrpCacheReferenceDescriptor,
    out_metadata: *mut NnrpBufferView,
) -> NnrpFfiStatus {
    if out_descriptor.is_null() || out_metadata.is_null() {
        return NnrpFfiStatus::invalid_argument(55);
    }
    let store = handle_store();
    match store.get(handle, NnrpHandleKind::CacheReferenceDescriptor) {
        Ok(NnrpFfiResource::CacheReferenceDescriptor {
            descriptor,
            metadata,
        }) => {
            *out_descriptor = (*descriptor).into();
            *out_metadata = NnrpBufferView {
                ptr: metadata.as_ptr(),
                len: metadata.len(),
            };
            NnrpFfiStatus::ok()
        }
        Ok(_) => NnrpFfiStatus::invalid_handle(NnrpHandleKind::CacheReferenceDescriptor as u32),
        Err(status) => status,
    }
}

#[no_mangle]
/// # Safety
///
/// `out_buffer` and `out_view` must be either null or valid writable pointers
/// to one value each.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_cache_reference_descriptor_metadata_snapshot(handle: NnrpHandle, out_buffer: *mut NnrpHandle, out_view: *mut NnrpBufferView) -> NnrpFfiStatus {
    nnrp_cache_reference_descriptor_metadata_snapshot_impl(handle, out_buffer, out_view)
}

unsafe fn nnrp_cache_reference_descriptor_metadata_snapshot_impl(
    handle: NnrpHandle,
    out_buffer: *mut NnrpHandle,
    out_view: *mut NnrpBufferView,
) -> NnrpFfiStatus {
    if out_buffer.is_null() || out_view.is_null() {
        return NnrpFfiStatus::invalid_argument(57);
    }
    let mut store = handle_store();
    let metadata = match store.get(handle, NnrpHandleKind::CacheReferenceDescriptor) {
        Ok(NnrpFfiResource::CacheReferenceDescriptor { metadata, .. }) => metadata.clone(),
        Ok(_) => {
            return NnrpFfiStatus::invalid_handle(NnrpHandleKind::CacheReferenceDescriptor as u32)
        }
        Err(status) => return status,
    };
    let (buffer, view) = match insert_owned_buffer(&mut store, metadata) {
        Ok(result) => result,
        Err(status) => return status,
    };
    *out_buffer = buffer;
    *out_view = view;
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// The descriptor handle is copied by value. This function does not dereference
/// caller-provided pointers.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_cache_reference_descriptor_release(handle: NnrpHandle) -> NnrpFfiStatus {
    nnrp_cache_reference_descriptor_release_impl(handle)
}

fn nnrp_cache_reference_descriptor_release_impl(handle: NnrpHandle) -> NnrpFfiStatus {
    let mut store = handle_store();
    match store.get(handle, NnrpHandleKind::CacheReferenceDescriptor) {
        Ok(NnrpFfiResource::CacheReferenceDescriptor { .. }) => store
            .remove(handle, NnrpHandleKind::CacheReferenceDescriptor)
            .map(|_| NnrpFfiStatus::ok())
            .unwrap_or_else(|status| status),
        Ok(_) => NnrpFfiStatus::invalid_handle(NnrpHandleKind::CacheReferenceDescriptor as u32),
        Err(status) => status,
    }
}

#[no_mangle]
/// # Safety
///
/// `out_result` must be either null or a valid writable pointer to one
/// `NnrpCacheLeaseResult`.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_cache_query(request: NnrpCacheLeaseRequest, out_result: *mut NnrpCacheLeaseResult) -> NnrpFfiStatus {
    cache_query_impl(request, out_result)
}

#[no_mangle]
/// # Safety
///
/// `out_result` must be either null or a valid writable pointer to one
/// `NnrpCacheLeaseResult`.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_cache_touch(request: NnrpCacheLeaseRequest, out_result: *mut NnrpCacheLeaseResult) -> NnrpFfiStatus {
    nnrp_cache_touch_impl(request, out_result)
}

unsafe fn nnrp_cache_touch_impl(
    request: NnrpCacheLeaseRequest,
    out_result: *mut NnrpCacheLeaseResult,
) -> NnrpFfiStatus {
    if request.ttl_ms == 0 {
        return NnrpFfiStatus::invalid_argument(46);
    }
    cache_query_impl(request, out_result)
}

#[no_mangle]
/// # Safety
///
/// `objects` must point to `object_count` readable `NnrpCacheObjectId` values
/// when `object_count` is non-zero. `out_results` must point to
/// `object_count` writable result slots.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_cache_prefetch(owner: NnrpHandle, objects: *const NnrpCacheObjectId, object_count: usize, now_ms: u64, ttl_ms: u32, out_results: *mut NnrpCacheLeaseResult) -> NnrpFfiStatus {
    nnrp_cache_prefetch_impl(owner, objects, object_count, now_ms, ttl_ms, out_results)
}

unsafe fn nnrp_cache_prefetch_impl(
    owner: NnrpHandle,
    objects: *const NnrpCacheObjectId,
    object_count: usize,
    now_ms: u64,
    ttl_ms: u32,
    out_results: *mut NnrpCacheLeaseResult,
) -> NnrpFfiStatus {
    if object_count > 0 && (objects.is_null() || out_results.is_null()) {
        return NnrpFfiStatus::invalid_argument(47);
    }
    let object_ids = if object_count == 0 {
        &[]
    } else {
        core::slice::from_raw_parts(objects, object_count)
    };
    for (index, object) in object_ids.iter().enumerate() {
        let request = NnrpCacheLeaseRequest {
            owner,
            object_id: *object,
            expected_version: 0,
            now_ms,
            ttl_ms,
        };
        let status = cache_query_impl(request, out_results.add(index));
        if status.status_code != NnrpFfiStatusCode::Ok as u32 {
            return status;
        }
    }
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// `out_result` must be either null or a valid writable pointer to one
/// `NnrpCacheLeaseResult`.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_cache_release(lease_handle: NnrpHandle, out_result: *mut NnrpCacheLeaseResult) -> NnrpFfiStatus {
    nnrp_cache_release_impl(lease_handle, out_result)
}

unsafe fn nnrp_cache_release_impl(
    lease_handle: NnrpHandle,
    out_result: *mut NnrpCacheLeaseResult,
) -> NnrpFfiStatus {
    if out_result.is_null() {
        return NnrpFfiStatus::invalid_argument(48);
    }
    let mut store = handle_store();
    match store.get_mut(lease_handle, NnrpHandleKind::CacheLease) {
        Ok(NnrpFfiResource::CacheLease {
            lease, released, ..
        }) => {
            *released = true;
            *out_result = NnrpCacheLeaseResult::from_lease(
                NNRP_CACHE_LEASE_OUTCOME_RELEASED,
                lease_handle,
                *lease,
            );
            NnrpFfiStatus::ok()
        }
        Ok(_) => NnrpFfiStatus::invalid_handle(NnrpHandleKind::CacheLease as u32),
        Err(status) => status,
    }
}

unsafe fn cache_query_impl(
    request: NnrpCacheLeaseRequest,
    out_result: *mut NnrpCacheLeaseResult,
) -> NnrpFfiStatus {
    if out_result.is_null() {
        return NnrpFfiStatus::invalid_argument(49);
    }
    let object_id = match request.object_id.to_core() {
        Ok(object_id) => object_id,
        Err(status) => return status,
    };
    if request.owner.kind == NnrpHandleKind::Invalid as u32 {
        *out_result = NnrpCacheLeaseResult::miss(object_id);
        return cache_validation_failure_status(CacheValidationFailure::Miss);
    }

    let owner_kind = match cache_owner_handle_kind(request.owner.kind) {
        Ok(kind) => kind,
        Err(status) => {
            *out_result = NnrpCacheLeaseResult::miss(object_id);
            return status;
        }
    };
    let mut store = handle_store();
    if let Err(status) = store.get(request.owner, owner_kind) {
        return status;
    }
    let existing = store
        .entries
        .iter()
        .find_map(|((kind, id), entry)| match &entry.resource {
            NnrpFfiResource::CacheLease {
                owner,
                lease,
                released,
            } if *owner == request.owner && lease.object_id == object_id && !*released => Some((
                NnrpHandle {
                    kind: *kind,
                    id: *id,
                    generation: entry.generation,
                    flags: 0,
                },
                *lease,
            )),
            _ => None,
        });

    if let Some((handle, mut lease)) = existing {
        if lease.validate_live_at(request.now_ms).is_err() {
            *out_result =
                NnrpCacheLeaseResult::from_lease(NNRP_CACHE_LEASE_OUTCOME_EXPIRED, handle, lease);
            return cache_validation_failure_status(CacheValidationFailure::LeaseExpired);
        }
        if request.expected_version != 0
            && lease.validate_version(request.expected_version).is_err()
        {
            *out_result =
                NnrpCacheLeaseResult::from_lease(NNRP_CACHE_LEASE_OUTCOME_VALID, handle, lease);
            return cache_validation_failure_status(CacheValidationFailure::VersionMismatch);
        }
        if request.ttl_ms != 0 {
            lease.ttl_ms = request.ttl_ms;
            if let Ok(NnrpFfiResource::CacheLease {
                lease: stored_lease,
                ..
            }) = store.get_mut(handle, NnrpHandleKind::CacheLease)
            {
                *stored_lease = lease;
            }
        }
        *out_result =
            NnrpCacheLeaseResult::from_lease(NNRP_CACHE_LEASE_OUTCOME_VALID, handle, lease);
        return NnrpFfiStatus::ok();
    }

    let id = next_handle_id(&mut store, NnrpHandleKind::CacheLease);
    let handle = NnrpHandle::new(NnrpHandleKind::CacheLease, id, 1);
    let lease = CacheLease {
        object_id,
        object_version: request.expected_version.max(1),
        lease_id: id,
        owner_scope: cache_owner_scope(request.owner.kind),
        owner_id: request.owner.id,
        granted_at_ms: request.now_ms,
        ttl_ms: request.ttl_ms.max(30_000),
    };
    if let Err(status) = store.insert(
        handle,
        NnrpFfiResource::CacheLease {
            owner: request.owner,
            lease,
            released: false,
        },
    ) {
        return status;
    }
    *out_result = NnrpCacheLeaseResult::from_lease(NNRP_CACHE_LEASE_OUTCOME_VALID, handle, lease);
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// `out_server` must be either null or a valid writable pointer to one
/// `NnrpHandle`. When non-null, the pointed memory must be owned by the caller.
#[allow(clippy::needless_return)]
pub unsafe extern "C" fn nnrp_server_bind(
    request: NnrpServerBindRequest,
    out_server: *mut NnrpHandle,
) -> NnrpFfiStatus {
    #[cfg(test)]
    {
        if out_server.is_null() || request.server_id == 0 || request.generation == 0 {
            return NnrpFfiStatus::invalid_argument(18);
        }
        if !transport_id_enabled(request.transport_id) {
            return NnrpFfiStatus::invalid_argument(47);
        }

        let handle = NnrpHandle::new(
            NnrpHandleKind::Connection,
            request.server_id,
            request.generation,
        );
        let mut store = handle_store();
        if let Err(status) = store.insert(
            handle,
            NnrpFfiResource::Connection {
                transport_id: request.transport_id,
                role: NnrpFfiConnectionRole::Server,
            },
        ) {
            return status;
        }
        store.push_event(NnrpQueuedEvent::plain(
            NnrpEventKind::ConnectionOpened,
            handle,
            NnrpHandle::invalid(),
            NnrpHandle::invalid(),
            0,
        ));
        *out_server = handle;
        return NnrpFfiStatus::ok();
    }

    #[cfg(not(test))]
    {
        if out_server.is_null()
            || request.server_id == 0
            || request.generation == 0
            || request.reserved0 != 0
        {
            return NnrpFfiStatus::invalid_argument(18);
        }
        let listener = match transport::role_listener(request.transport_listener) {
            Ok(listener) => listener,
            Err(status) => return status,
        };
        let transport_kind = listener.transport_kind();
        let transport_id = runtime_transport_id(transport_kind);
        let server = match NnrpServer::from_boxed_listener(
            listener,
            NnrpServerConfig::default().with_transport(transport_kind),
        ) {
            Ok(server) => Arc::new(server),
            Err(error) => return transport::role_status_from_runtime_error(error),
        };

        let handle = NnrpHandle::new(
            NnrpHandleKind::Connection,
            request.server_id,
            request.generation,
        );
        let mut store = handle_store();
        if let Err(status) = store.insert(
            handle,
            NnrpFfiResource::Connection {
                transport_id,
                role: NnrpFfiConnectionRole::Server,
                runtime: NnrpFfiConnectionRuntime::Server(server),
            },
        ) {
            return status;
        }
        if let Err(status) = transport::consume_role_listener(request.transport_listener) {
            let _ = store.remove(handle, NnrpHandleKind::Connection);
            return status;
        }
        *out_server = handle;
        NnrpFfiStatus::ok()
    }
}

#[no_mangle]
/// # Safety
///
/// `out_session` must be either null or a valid writable pointer to one
/// `NnrpHandle`. The server handle is copied by value and is not retained.
#[allow(clippy::needless_return)]
pub unsafe extern "C" fn nnrp_server_accept(
    request: NnrpServerAcceptRequest,
    out_session: *mut NnrpHandle,
) -> NnrpFfiStatus {
    #[cfg(test)]
    {
        if out_session.is_null() || request.session_id == 0 || request.generation == 0 {
            return NnrpFfiStatus::invalid_argument(19);
        }
        let mut store = handle_store();
        match store.get_connection_role(request.server) {
            Ok(NnrpFfiConnectionRole::Server) => {}
            Ok(NnrpFfiConnectionRole::Client) => {
                return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32);
            }
            Err(status) => return status,
        }

        let handle = NnrpHandle::new(
            NnrpHandleKind::Session,
            request.session_id as u64,
            request.generation,
        );
        if let Err(status) = store.insert(
            handle,
            NnrpFfiResource::Session {
                connection: request.server,
                profile_id: request.profile_id,
                schema_id: request.schema_id,
                schema_version: request.schema_version,
            },
        ) {
            return status;
        }
        store.push_event(NnrpQueuedEvent::plain(
            NnrpEventKind::SessionOpened,
            request.server,
            handle,
            NnrpHandle::invalid(),
            0,
        ));
        *out_session = handle;
        return NnrpFfiStatus::ok();
    }

    #[cfg(not(test))]
    {
        if out_session.is_null() || request.session_handle_id == 0 || request.generation == 0 {
            return NnrpFfiStatus::invalid_argument(19);
        }
        let server = {
            let store = handle_store();
            match store.get(request.server, NnrpHandleKind::Connection) {
                Ok(NnrpFfiResource::Connection {
                    role: NnrpFfiConnectionRole::Server,
                    runtime: NnrpFfiConnectionRuntime::Server(server),
                    ..
                }) => Arc::clone(server),
                Ok(_) => {
                    return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32);
                }
                Err(status) => return status,
            }
        };
        let session = match transport::run_role_async(
            async move { server.accept().await },
            request.timeout_ms,
        ) {
            Ok(session) => session,
            Err(status) => return status,
        };
        let profile_id = session.client_open().profile_id;
        let schema_id = session.client_open().schema_id;
        let schema_version = session.client_open().schema_version;

        let handle = NnrpHandle::new(
            NnrpHandleKind::Session,
            request.session_handle_id,
            request.generation,
        );
        let mut store = handle_store();
        if let Err(status) = store.insert(
            handle,
            NnrpFfiResource::Session {
                connection: request.server,
                profile_id,
                schema_id,
                schema_version,
                runtime: NnrpFfiSessionRuntime::Server(Arc::new(AsyncMutex::new(session))),
            },
        ) {
            return status;
        }
        *out_session = handle;
        NnrpFfiStatus::ok()
    }
}

#[cfg(test)]
#[no_mangle]
/// # Safety
///
/// `out_operation` must be either null or a valid writable pointer to one
/// `NnrpHandle`. `request.payload` must remain readable for `request.payload.len`
/// bytes for the duration of the call.
pub unsafe extern "C" fn nnrp_server_receive_submit(
    request: NnrpServerReceiveSubmitRequest,
    out_operation: *mut NnrpHandle,
) -> NnrpFfiStatus {
    if out_operation.is_null() || request.operation_id == 0 || request.frame_id == 0 {
        return NnrpFfiStatus::invalid_argument(20);
    }
    if let Err(status) = request.payload.validate() {
        return status;
    }
    let mut store = handle_store();
    let connection = match store.get(request.session, NnrpHandleKind::Session) {
        Ok(NnrpFfiResource::Session { connection, .. }) => *connection,
        Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
        Err(status) => return status,
    };
    match store.get_connection_role(connection) {
        Ok(NnrpFfiConnectionRole::Server) => {}
        Ok(NnrpFfiConnectionRole::Client) => {
            return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32);
        }
        Err(status) => return status,
    }

    let handle = NnrpHandle::new(
        NnrpHandleKind::Operation,
        request.operation_id,
        request.session.generation,
    );
    if let Err(status) = store.insert(
        handle,
        NnrpFfiResource::Operation {
            session: request.session,
            operation_id: request.operation_id,
            frame_id: request.frame_id,
            payload_len: request.payload.len,
        },
    ) {
        return status;
    }
    store.push_event(NnrpQueuedEvent::plain(
        NnrpEventKind::SubmitAccepted,
        connection,
        request.session,
        handle,
        request.frame_id,
    ));
    *out_operation = handle;
    NnrpFfiStatus::ok()
}

#[cfg(not(test))]
#[no_mangle]
/// Reads and decodes a bounded server event batch from a live role session.
/// Inbound submits create operation handles owned by the server session.
///
/// # Safety
///
/// `out_events` must point to `event_capacity` writable `NnrpEvent` entries
/// when `event_capacity` is non-zero. `out_event_count` must point to one
/// writable `usize`.
pub unsafe extern "C" fn nnrp_server_await_events(
    request: NnrpRoleEventPollRequest,
    out_events: *mut NnrpEvent,
    event_capacity: usize,
    out_event_count: *mut usize,
) -> NnrpFfiStatus {
    role_server_await_events_impl(request, out_events, event_capacity, out_event_count)
}

#[no_mangle]
/// # Safety
///
/// `request.payload` must remain readable for `request.payload.len` bytes for
/// the duration of the call.
pub unsafe extern "C" fn nnrp_server_send_result(
    request: NnrpServerSendResultRequest,
) -> NnrpFfiStatus {
    #[cfg(test)]
    {
        if let Err(status) = request.payload.validate() {
            return status;
        }
        push_operation_event(request.operation, NnrpEventKind::ResultPushed, false)
    }

    #[cfg(not(test))]
    {
        nnrp_server_send_result_impl(request)
    }
}

#[cfg(not(test))]
unsafe fn nnrp_server_send_result_impl(request: NnrpServerSendResultRequest) -> NnrpFfiStatus {
    if let Err(status) = request.payload.validate() {
        return status;
    }
    let payload = ffi_read_slice(request.payload);
    if payload.len() < RESULT_PUSH_METADATA_LEN {
        return NnrpFfiStatus::invalid_argument(148);
    }
    let metadata = match ResultPushMetadata::parse(&payload[..RESULT_PUSH_METADATA_LEN]) {
        Ok(metadata) => metadata,
        Err(error) => return NnrpFfiStatus::from_core_error(&error),
    };
    let body = payload[RESULT_PUSH_METADATA_LEN..].to_vec();
    let (session_handle, frame_id, session) = {
        let store = handle_store();
        let (session_handle, frame_id) =
            match store.get(request.operation, NnrpHandleKind::Operation) {
                Ok(NnrpFfiResource::Operation {
                    session, frame_id, ..
                }) => (*session, *frame_id),
                Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32),
                Err(status) => return status,
            };
        let session = match store.get(session_handle, NnrpHandleKind::Session) {
            Ok(NnrpFfiResource::Session {
                runtime: NnrpFfiSessionRuntime::Server(session),
                ..
            }) => Arc::clone(session),
            Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
            Err(status) => return status,
        };
        (session_handle, frame_id, session)
    };
    if let Err(status) = role_session_connection(session_handle, NnrpFfiConnectionRole::Server) {
        return status;
    }
    if let Err(status) = transport::run_role_async(
        async move {
            session
                .lock()
                .await
                .send_result(frame_id, metadata, body)
                .await
        },
        0,
    ) {
        return status;
    }
    let mut store = handle_store();
    match store.remove(request.operation, NnrpHandleKind::Operation) {
        Ok(()) => NnrpFfiStatus::ok(),
        Err(status) => status,
    }
}

#[no_mangle]
/// # Safety
///
/// `request.partial_body` must remain readable for `partial_body.len` bytes for
/// the duration of the call. `out_result` must point to one caller-owned
/// writable `NnrpPollResult`. The returned event payload aliases the
/// caller-owned partial body view.
pub unsafe extern "C" fn nnrp_server_send_partial_result(
    request: NnrpServerSendPartialResultRequest,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    nnrp_server_send_partial_result_impl(request, out_result)
}

unsafe fn nnrp_server_send_partial_result_impl(
    request: NnrpServerSendPartialResultRequest,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    if out_result.is_null() {
        return NnrpFfiStatus::invalid_argument(132);
    }
    if let Err(status) = request.partial_body.validate() {
        *out_result = poll_result_none(status);
        return status;
    }

    let metadata = PartialResultMetadata::from(request.partial_result);
    let operation_id = match operation_wire_id(request.operation) {
        Ok(operation_id) => operation_id,
        Err(status) => {
            *out_result = poll_result_none(status);
            return status;
        }
    };
    if metadata.operation_id != operation_id {
        let status = NnrpFfiStatus::invalid_argument(133);
        *out_result = poll_result_none(status);
        return status;
    }
    if let Err(error) = validate_partial_result_semantics(&metadata) {
        let status = NnrpFfiStatus::from_core_error(&error);
        *out_result = poll_result_none(status);
        return status;
    }
    if let Err(error) = metadata.to_vec_with_body(ffi_read_slice(request.partial_body)) {
        let status = NnrpFfiStatus::from_core_error(&error);
        *out_result = poll_result_none(status);
        return status;
    }

    #[cfg(not(test))]
    {
        let session = match server_runtime_for_operation(request.operation) {
            Ok(session) => session,
            Err(status) => {
                *out_result = poll_result_none(status);
                return status;
            }
        };
        let body = ffi_read_slice(request.partial_body).to_vec();
        let status = match transport::run_role_async(
            async move {
                session
                    .lock()
                    .await
                    .send_partial_result(metadata, body)
                    .await
            },
            0,
        ) {
            Ok(()) => NnrpFfiStatus::ok(),
            Err(status) => status,
        };
        *out_result = poll_result_none(status);
        status
    }

    #[cfg(test)]
    {
        let scope = match push_operation_event_with_scope(
            request.operation,
            NnrpEventKind::PartialResult,
            false,
        ) {
            Ok(scope) => scope,
            Err(status) => {
                *out_result = poll_result_none(status);
                return status;
            }
        };

        poll_matching_operation_event_from_scope(
            scope,
            request.operation,
            metadata.operation_id,
            NnrpEventKind::PartialResult,
            request.partial_body,
            request.max_events,
            out_result,
        )
    }
}

#[no_mangle]
/// # Safety
///
/// `request.diagnostics` must remain readable for `diagnostics.len` bytes for
/// the duration of the call. `out_result` must point to one caller-owned
/// writable `NnrpPollResult`. The returned event payload aliases the
/// caller-owned diagnostics view.
pub unsafe extern "C" fn nnrp_server_drop_stale_result(
    request: NnrpServerDropStaleResultRequest,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    nnrp_server_drop_stale_result_impl(request, out_result)
}

unsafe fn nnrp_server_drop_stale_result_impl(
    request: NnrpServerDropStaleResultRequest,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    if out_result.is_null() {
        return NnrpFfiStatus::invalid_argument(134);
    }
    if let Err(status) = request.diagnostics.validate() {
        *out_result = poll_result_none(status);
        return status;
    }

    let metadata = ResultDropReasonMetadata::from(request.drop_reason);
    let operation_id = match operation_wire_id(request.operation) {
        Ok(operation_id) => operation_id,
        Err(status) => {
            *out_result = poll_result_none(status);
            return status;
        }
    };
    if metadata.operation_id != operation_id {
        let status = NnrpFfiStatus::invalid_argument(135);
        *out_result = poll_result_none(status);
        return status;
    }
    if let Err(error) = validate_result_drop_reason_semantics(&metadata) {
        let status = NnrpFfiStatus::from_core_error(&error);
        *out_result = poll_result_none(status);
        return status;
    }
    if let Err(error) = metadata.to_vec_with_diagnostics(ffi_read_slice(request.diagnostics)) {
        let status = NnrpFfiStatus::from_core_error(&error);
        *out_result = poll_result_none(status);
        return status;
    }

    #[cfg(not(test))]
    {
        let session = match server_runtime_for_operation(request.operation) {
            Ok(session) => session,
            Err(status) => {
                *out_result = poll_result_none(status);
                return status;
            }
        };
        let diagnostics = ffi_read_slice(request.diagnostics).to_vec();
        let status = match transport::run_role_async(
            async move {
                session
                    .lock()
                    .await
                    .send_result_drop_reason_with_diagnostics(metadata, diagnostics)
                    .await
            },
            0,
        ) {
            Ok(()) => NnrpFfiStatus::ok(),
            Err(status) => status,
        };
        if status.status_code == NnrpFfiStatusCode::Ok as u32 {
            if let Err(remove_status) =
                handle_store().remove(request.operation, NnrpHandleKind::Operation)
            {
                *out_result = poll_result_none(remove_status);
                return remove_status;
            }
        }
        *out_result = poll_result_none(status);
        status
    }

    #[cfg(test)]
    {
        let scope = match push_operation_event_with_scope(
            request.operation,
            NnrpEventKind::ResultDropped,
            true,
        ) {
            Ok(scope) => scope,
            Err(status) => {
                *out_result = poll_result_none(status);
                return status;
            }
        };

        poll_matching_operation_event_from_scope(
            scope,
            request.operation,
            metadata.operation_id,
            NnrpEventKind::ResultDropped,
            request.diagnostics,
            request.max_events,
            out_result,
        )
    }
}

#[cfg(not(test))]
fn server_runtime_for_operation(
    operation: NnrpHandle,
) -> Result<Arc<AsyncMutex<NnrpServerSession>>, NnrpFfiStatus> {
    let store = handle_store();
    let session = match store.get(operation, NnrpHandleKind::Operation)? {
        NnrpFfiResource::Operation { session, .. } => *session,
        _ => {
            return Err(NnrpFfiStatus::invalid_handle(
                NnrpHandleKind::Operation as u32,
            ))
        }
    };
    let (connection, runtime) = match store.get(session, NnrpHandleKind::Session)? {
        NnrpFfiResource::Session {
            connection,
            runtime: NnrpFfiSessionRuntime::Server(session),
            ..
        } => (*connection, Arc::clone(session)),
        _ => {
            return Err(NnrpFfiStatus::invalid_handle(
                NnrpHandleKind::Session as u32,
            ))
        }
    };
    if store.get_connection_role(connection)? != NnrpFfiConnectionRole::Server {
        return Err(NnrpFfiStatus::invalid_handle(
            NnrpHandleKind::Session as u32,
        ));
    }
    Ok(runtime)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(test, feature = "benchmark-ffi"))]
struct OperationEventScope {
    connection: NnrpHandle,
    session: NnrpHandle,
    frame_id: u32,
}

fn operation_wire_id(operation: NnrpHandle) -> Result<u64, NnrpFfiStatus> {
    match handle_store().get(operation, NnrpHandleKind::Operation)? {
        NnrpFfiResource::Operation { operation_id, .. } => Ok(*operation_id),
        _ => Err(NnrpFfiStatus::invalid_handle(
            NnrpHandleKind::Operation as u32,
        )),
    }
}

#[cfg(any(test, feature = "benchmark-ffi"))]
fn push_operation_event(
    operation: NnrpHandle,
    event_kind: NnrpEventKind,
    remove_operation: bool,
) -> NnrpFfiStatus {
    push_operation_event_with_scope(operation, event_kind, remove_operation)
        .map(|_| NnrpFfiStatus::ok())
        .unwrap_or_else(|status| status)
}

#[cfg(any(test, feature = "benchmark-ffi"))]
fn push_operation_event_with_scope(
    operation: NnrpHandle,
    event_kind: NnrpEventKind,
    remove_operation: bool,
) -> Result<OperationEventScope, NnrpFfiStatus> {
    let mut store = handle_store();
    let (connection, session, frame_id) = match store.get(operation, NnrpHandleKind::Operation) {
        Ok(NnrpFfiResource::Operation {
            session, frame_id, ..
        }) => {
            let session_resource = store.get(*session, NnrpHandleKind::Session);
            match session_resource {
                Ok(NnrpFfiResource::Session { connection, .. }) => {
                    (*connection, *session, *frame_id)
                }
                Ok(_) => {
                    return Err(NnrpFfiStatus::invalid_handle(
                        NnrpHandleKind::Session as u32,
                    ))
                }
                Err(status) => return Err(status),
            }
        }
        Ok(_) => {
            return Err(NnrpFfiStatus::invalid_handle(
                NnrpHandleKind::Operation as u32,
            ))
        }
        Err(status) => return Err(status),
    };
    if remove_operation {
        store.entries.remove(&(operation.kind, operation.id));
    }
    store.push_event(NnrpQueuedEvent::plain(
        event_kind, connection, session, operation, frame_id,
    ));
    Ok(OperationEventScope {
        connection,
        session,
        frame_id,
    })
}

#[no_mangle]
/// # Safety
///
/// The session handle is copied by value. This function does not dereference
/// caller-provided pointers.
pub unsafe extern "C" fn nnrp_server_close(session: NnrpHandle) -> NnrpFfiStatus {
    #[cfg(not(test))]
    {
        let runtime = {
            let store = handle_store();
            match store.get(session, NnrpHandleKind::Session) {
                Ok(NnrpFfiResource::Session {
                    runtime: NnrpFfiSessionRuntime::Server(runtime),
                    ..
                }) => Arc::clone(runtime),
                Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
                Err(status) => return status,
            }
        };
        if let Err(status) = transport::run_role_async(
            async move { runtime.lock().await.close_in_place().await },
            0,
        ) {
            return status;
        }
        return handle_store()
            .close_session(session)
            .map(|_| NnrpFfiStatus::ok())
            .unwrap_or_else(|status| status);
    }

    #[cfg(test)]
    {
        let _ = core::hint::black_box(NnrpFfiConnectionRole::Server);
        let mut store = handle_store();
        let connection = match store.get(session, NnrpHandleKind::Session) {
            Ok(NnrpFfiResource::Session { connection, .. }) => *connection,
            Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
            Err(status) => return status,
        };
        store
            .close_session(session)
            .map(|_| {
                store.push_event(NnrpQueuedEvent::plain(
                    NnrpEventKind::SessionClosed,
                    connection,
                    session,
                    NnrpHandle::invalid(),
                    0,
                ));
                NnrpFfiStatus::ok()
            })
            .unwrap_or_else(|status| status)
    }
}

#[no_mangle]
/// Sends one validated Preview4 runtime frame through a client or server
/// session/operation handle and queues an owned typed event.
///
/// # Safety
///
/// `request.payload` must remain readable for `request.payload.len` bytes for
/// the duration of the call. The implementation snapshots the payload before
/// returning. The receiver owns the returned event payload through
/// `NnrpEvent.payload_owner` and must release it with `nnrp_buffer_release`.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_runtime_frame_send(request: NnrpRuntimeFrameSendRequest) -> NnrpFfiStatus { nnrp_runtime_frame_send_impl(request) }

unsafe fn nnrp_runtime_frame_send_impl(request: NnrpRuntimeFrameSendRequest) -> NnrpFfiStatus {
    let message_type = match validate_runtime_frame_payload(request.message_type, request.payload) {
        Ok(message_type) => message_type,
        Err(status) => return status,
    };
    if !matches!(
        request.handle.kind,
        value if value == NnrpHandleKind::Session as u32
            || value == NnrpHandleKind::Operation as u32
    ) {
        return NnrpFfiStatus::invalid_handle(request.handle.kind);
    }

    #[cfg(not(test))]
    {
        nnrp_runtime_frame_send_role_impl(request, message_type)
    }

    #[cfg(test)]
    {
        let mut store = handle_store();
        let (connection, session, operation) = match event_scope_for_handle(&store, request.handle)
        {
            Ok(scope) => scope,
            Err(status) => return status,
        };
        let role = match store.get_connection_role(connection) {
            Ok(role) => role,
            Err(status) => return status,
        };
        if let Err(status) = validate_runtime_frame_direction(role, message_type) {
            return status;
        }

        let payload = ffi_read_slice(request.payload).to_vec();
        let (payload_owner, _) = match insert_owned_buffer(&mut store, payload) {
            Ok(owned) => owned,
            Err(status) => return status,
        };
        store.push_event(NnrpQueuedEvent {
            kind: if message_type == MessageType::FlowUpdate {
                NnrpEventKind::FlowUpdated as u32
            } else {
                NnrpEventKind::RuntimeFrame as u32
            },
            message_type: message_type as u32,
            connection,
            session,
            operation,
            frame_id: request.frame_id,
            payload_owner,
        });
        NnrpFfiStatus::ok()
    }
}

#[cfg(not(test))]
unsafe fn nnrp_runtime_frame_send_role_impl(
    request: NnrpRuntimeFrameSendRequest,
    message_type: MessageType,
) -> NnrpFfiStatus {
    let payload = ffi_read_slice(request.payload).to_vec();
    let (role, runtime) = {
        let store = handle_store();
        let (connection, session, _) = match event_scope_for_handle(&store, request.handle) {
            Ok(scope) => scope,
            Err(status) => return status,
        };
        if session.kind != NnrpHandleKind::Session as u32 {
            return NnrpFfiStatus::invalid_handle(request.handle.kind);
        }
        let role = match store.get_connection_role(connection) {
            Ok(role) => role,
            Err(status) => return status,
        };
        if let Err(status) = validate_runtime_frame_direction(role, message_type) {
            return status;
        }
        let runtime = match store.get(session, NnrpHandleKind::Session) {
            Ok(NnrpFfiResource::Session {
                runtime: NnrpFfiSessionRuntime::Client(session),
                ..
            }) => NnrpFfiRoleSession::Client(Arc::clone(session)),
            Ok(NnrpFfiResource::Session {
                runtime: NnrpFfiSessionRuntime::Server(session),
                ..
            }) => NnrpFfiRoleSession::Server(Arc::clone(session)),
            Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
            Err(status) => return status,
        };
        (role, runtime)
    };

    match (role, runtime) {
        (NnrpFfiConnectionRole::Client, NnrpFfiRoleSession::Client(session)) => {
            transport::run_role_async(
                send_client_runtime_frame(session, message_type, request.frame_id, payload),
                0,
            )
            .map(|_| NnrpFfiStatus::ok())
            .unwrap_or_else(|status| status)
        }
        (NnrpFfiConnectionRole::Server, NnrpFfiRoleSession::Server(session)) => {
            transport::run_role_async(
                send_server_runtime_frame(session, message_type, request.frame_id, payload),
                0,
            )
            .map(|_| NnrpFfiStatus::ok())
            .unwrap_or_else(|status| status)
        }
        _ => NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
    }
}

#[cfg(not(test))]
async fn send_client_runtime_frame(
    session: Arc<AsyncMutex<NnrpClientSession>>,
    message_type: MessageType,
    frame_id: u32,
    payload: Vec<u8>,
) -> Result<(), nnrp_runtime::RuntimeError> {
    let mut session = session.lock().await;
    match message_type {
        MessageType::FlowUpdate => {
            session
                .send_flow_update(FlowUpdateMetadata::parse(&payload)?)
                .await
        }
        MessageType::Progress => {
            let (metadata, body) = ProgressMetadata::parse_with_body(&payload)?;
            session.send_progress(metadata, body.to_vec()).await
        }
        MessageType::PartialResult => {
            let (metadata, body) = PartialResultMetadata::parse_with_body(&payload)?;
            session.send_partial_result(metadata, body.to_vec()).await
        }
        MessageType::Backpressure => {
            session
                .send_backpressure(PressureMetadata::parse(&payload)?)
                .await
        }
        MessageType::ResultDropReason => {
            let (metadata, body) = ResultDropReasonMetadata::parse_with_diagnostics(&payload)?;
            session
                .send_result_drop_reason(metadata, body.to_vec())
                .await
        }
        MessageType::Cancel | MessageType::Abort => {
            let (metadata, body) = ControlRequestMetadata::parse_with_diagnostics(&payload)?;
            session
                .send_control_request_with_diagnostics(message_type, metadata, body.to_vec())
                .await
        }
        MessageType::PriorityUpdate | MessageType::Deadline | MessageType::ExpireAt => {
            session
                .send_scheduling_update(message_type, SchedulingMetadata::parse(&payload)?)
                .await
        }
        MessageType::Supersede => {
            let (metadata, body) = SupersedeMetadata::parse_with_diagnostics(&payload)?;
            session.supersede_operation(metadata, body.to_vec()).await
        }
        MessageType::BudgetUpdate => {
            session
                .update_budget(BudgetMetadata::parse(&payload)?)
                .await
        }
        MessageType::CreditUpdate => {
            session
                .send_credit_update(PressureMetadata::parse(&payload)?)
                .await
        }
        MessageType::CapabilityNegotiation | MessageType::DegradeProfile => {
            let (metadata, body) = CapabilityMetadata::parse_with_body(&payload)?;
            session
                .send_capability(message_type, metadata, body.to_vec())
                .await
        }
        MessageType::RouteHint | MessageType::ExecutionHint => {
            let (metadata, body) = RouteHintMetadata::parse_with_body(&payload)?;
            session
                .send_route_hint(message_type, metadata, body.to_vec())
                .await
        }
        MessageType::TraceContext => {
            let (metadata, body) = TraceContextMetadata::parse_with_body(&payload)?;
            session
                .send_trace_context(frame_id, metadata, body.to_vec())
                .await
        }
        MessageType::ErrorRecoverable => {
            let (metadata, body) = RecoverableErrorMetadata::parse_with_diagnostics(&payload)?;
            session
                .send_recoverable_error(metadata, body.to_vec())
                .await
        }
        MessageType::RetryAfter => {
            let (metadata, body) = RetryAfterMetadata::parse_with_diagnostics(&payload)?;
            session.send_retry_after(metadata, body.to_vec()).await
        }
        MessageType::ObjectDeclare => {
            let (metadata, body) = ObjectDescriptorMetadata::parse_with_extension(&payload)?;
            session.send_object_declare(metadata, body.to_vec()).await
        }
        MessageType::ObjectRef => {
            let (metadata, body) = ObjectReferenceMetadata::parse_with_extension(&payload)?;
            session.send_object_ref(metadata, body.to_vec()).await
        }
        MessageType::ObjectRelease => {
            let (metadata, body) = ObjectReleaseMetadata::parse_with_diagnostics(&payload)?;
            session.send_object_release(metadata, body.to_vec()).await
        }
        MessageType::ObjectPatch | MessageType::ObjectDelta => {
            let metadata = ObjectDeltaMetadata::parse(&payload)?;
            session
                .send_object_delta(
                    message_type,
                    metadata,
                    payload[OBJECT_DELTA_METADATA_LEN..].to_vec(),
                )
                .await
        }
        MessageType::CacheReference => {
            let (metadata, body) = CacheReferenceMetadata::parse_with_extension(&payload)?;
            session.send_cache_reference(metadata, body.to_vec()).await
        }
        MessageType::CacheMiss => {
            let (metadata, body) = CacheMissMetadata::parse_with_diagnostics(&payload)?;
            session.send_cache_miss(metadata, body.to_vec()).await
        }
        MessageType::CacheInvalidate => {
            session
                .send_cache_invalidate(CacheInvalidateMetadata::parse(&payload)?)
                .await
        }
        _ => Err(nnrp_runtime::RuntimeError::UnexpectedMessage(
            "client runtime frame direction is unsupported",
        )),
    }
}

#[cfg(not(test))]
async fn send_server_runtime_frame(
    session: Arc<AsyncMutex<NnrpServerSession>>,
    message_type: MessageType,
    frame_id: u32,
    payload: Vec<u8>,
) -> Result<(), nnrp_runtime::RuntimeError> {
    let mut session = session.lock().await;
    match message_type {
        MessageType::FlowUpdate => {
            session
                .send_flow_update(FlowUpdateMetadata::parse(&payload)?)
                .await
        }
        MessageType::Cancel | MessageType::Abort => {
            let (metadata, body) = ControlRequestMetadata::parse_with_diagnostics(&payload)?;
            session
                .send_control_request(message_type, metadata, body.to_vec())
                .await
        }
        MessageType::PriorityUpdate | MessageType::Deadline | MessageType::ExpireAt => {
            session
                .send_scheduling_update(message_type, SchedulingMetadata::parse(&payload)?)
                .await
        }
        MessageType::Supersede => {
            let (metadata, body) = SupersedeMetadata::parse_with_diagnostics(&payload)?;
            session.supersede_operation(metadata, body.to_vec()).await
        }
        MessageType::BudgetUpdate => {
            session
                .update_budget(BudgetMetadata::parse(&payload)?)
                .await
        }
        MessageType::Progress => {
            let (metadata, body) = ProgressMetadata::parse_with_body(&payload)?;
            session.send_progress(metadata, body.to_vec()).await
        }
        MessageType::PartialResult => {
            let (metadata, body) = PartialResultMetadata::parse_with_body(&payload)?;
            session.send_partial_result(metadata, body.to_vec()).await
        }
        MessageType::Backpressure => {
            session
                .send_backpressure(PressureMetadata::parse(&payload)?)
                .await
        }
        MessageType::CreditUpdate => {
            session
                .send_credit_update(PressureMetadata::parse(&payload)?)
                .await
        }
        MessageType::CapabilityNegotiation | MessageType::DegradeProfile => {
            let (metadata, body) = CapabilityMetadata::parse_with_body(&payload)?;
            session
                .send_capability(message_type, metadata, body.to_vec())
                .await
        }
        MessageType::RouteHint | MessageType::ExecutionHint => {
            let (metadata, body) = RouteHintMetadata::parse_with_body(&payload)?;
            session
                .send_route_hint(message_type, metadata, body.to_vec())
                .await
        }
        MessageType::TraceContext => {
            let (metadata, body) = TraceContextMetadata::parse_with_body(&payload)?;
            session
                .send_trace_context(frame_id, metadata, body.to_vec())
                .await
        }
        MessageType::ResultDropReason => {
            let (metadata, body) = ResultDropReasonMetadata::parse_with_diagnostics(&payload)?;
            session
                .send_result_drop_reason_with_diagnostics(metadata, body.to_vec())
                .await
        }
        MessageType::ErrorRecoverable => {
            let (metadata, body) = RecoverableErrorMetadata::parse_with_diagnostics(&payload)?;
            session
                .send_recoverable_error(metadata, body.to_vec())
                .await
        }
        MessageType::RetryAfter => {
            let (metadata, body) = RetryAfterMetadata::parse_with_diagnostics(&payload)?;
            session.send_retry_after(metadata, body.to_vec()).await
        }
        MessageType::ResultHint => {
            session
                .send_result_hint(ResultHintMetadata::parse(&payload)?)
                .await
        }
        MessageType::ObjectDeclare => {
            let (metadata, body) = ObjectDescriptorMetadata::parse_with_extension(&payload)?;
            session.send_object_declare(metadata, body.to_vec()).await
        }
        MessageType::ObjectRef => {
            let (metadata, body) = ObjectReferenceMetadata::parse_with_extension(&payload)?;
            session.send_object_ref(metadata, body.to_vec()).await
        }
        MessageType::ObjectRelease => {
            let (metadata, body) = ObjectReleaseMetadata::parse_with_diagnostics(&payload)?;
            session.send_object_release(metadata, body.to_vec()).await
        }
        MessageType::ObjectPatch | MessageType::ObjectDelta => {
            let metadata = ObjectDeltaMetadata::parse(&payload)?;
            session
                .send_object_delta(
                    message_type,
                    metadata,
                    payload[OBJECT_DELTA_METADATA_LEN..].to_vec(),
                )
                .await
        }
        MessageType::CacheReference => {
            let (metadata, body) = CacheReferenceMetadata::parse_with_extension(&payload)?;
            session.send_cache_reference(metadata, body.to_vec()).await
        }
        MessageType::CacheMiss => {
            let (metadata, body) = CacheMissMetadata::parse_with_diagnostics(&payload)?;
            session.send_cache_miss(metadata, body.to_vec()).await
        }
        MessageType::CacheInvalidate => {
            session
                .send_cache_invalidate(CacheInvalidateMetadata::parse(&payload)?)
                .await
        }
        _ => Err(nnrp_runtime::RuntimeError::UnexpectedMessage(
            "server runtime frame direction is unsupported",
        )),
    }
}

#[cfg(test)]
unsafe fn test_control_event_injection(request: NnrpControlRequest) -> NnrpFfiStatus {
    if request.handle.kind == NnrpHandleKind::Invalid as u32 {
        return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Invalid as u32);
    }
    if let Err(status) = request.payload.validate() {
        return status;
    }

    let event_kind = if request.control_code == MessageType::ResultHint as u32 {
        match ResultHintMetadata::parse(ffi_read_slice(request.payload)) {
            Ok(_) => NnrpEventKind::ResultHint,
            Err(error) => return NnrpFfiStatus::from_core_error(&error),
        }
    } else {
        NnrpEventKind::Control
    };

    let mut store = handle_store();
    let (connection, session, operation) = match request.handle.kind {
        value if value == NnrpHandleKind::Connection as u32 => {
            match store.get(request.handle, NnrpHandleKind::Connection) {
                Ok(NnrpFfiResource::Connection { .. }) => {
                    (request.handle, NnrpHandle::invalid(), NnrpHandle::invalid())
                }
                Ok(_) => {
                    return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32);
                }
                Err(status) => return status,
            }
        }
        value if value == NnrpHandleKind::Session as u32 => {
            match store.get(request.handle, NnrpHandleKind::Session) {
                Ok(NnrpFfiResource::Session { connection, .. }) => {
                    (*connection, request.handle, NnrpHandle::invalid())
                }
                Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
                Err(status) => return status,
            }
        }
        value if value == NnrpHandleKind::Operation as u32 => {
            match store.get(request.handle, NnrpHandleKind::Operation) {
                Ok(NnrpFfiResource::Operation { session, .. }) => {
                    let session = *session;
                    match store.get(session, NnrpHandleKind::Session) {
                        Ok(NnrpFfiResource::Session { connection, .. }) => {
                            (*connection, session, request.handle)
                        }
                        Ok(_) => {
                            return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
                        }
                        Err(status) => return status,
                    }
                }
                Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32),
                Err(status) => return status,
            }
        }
        _ => return NnrpFfiStatus::invalid_handle(request.handle.kind),
    };
    store.push_event(NnrpQueuedEvent::plain(
        event_kind, connection, session, operation, 0,
    ));
    NnrpFfiStatus::ok()
}

#[cfg(test)]
unsafe fn test_client_submit_control(
    request: NnrpClientSubmitControlRequest,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    if out_result.is_null() {
        return NnrpFfiStatus::invalid_argument(127);
    }
    match validate_control_metadata_payload(request.control.control_code, request.control.payload) {
        Ok(()) => {}
        Err(status) => {
            *out_result = poll_result_none(status);
            return status;
        }
    }

    let event_kind = control_event_kind(request.control.control_code);
    let submit_status = test_control_event_injection(request.control);
    if submit_status.status_code != NnrpFfiStatusCode::Ok as u32 {
        *out_result = poll_result_none(submit_status);
        return submit_status;
    }

    poll_matching_control_event(
        request.control.handle,
        event_kind,
        request.max_events,
        out_result,
    )
}

#[no_mangle]
/// # Safety
///
/// `out_result` must be either null or a valid writable pointer to one
/// `NnrpPollResult`. When non-null, the pointed memory must be owned by the caller.
pub unsafe extern "C" fn nnrp_poll_empty(out_result: *mut NnrpPollResult) -> NnrpFfiStatus {
    if out_result.is_null() {
        return NnrpFfiStatus::invalid_argument(13);
    }

    *out_result = NnrpPollResult {
        status: NnrpFfiStatus {
            status_code: NnrpFfiStatusCode::WouldBlock as u32,
            error_family: NnrpErrorFamily::None as u32,
            protocol_error_code: 0,
            detail_code: 0,
        },
        has_event: 0,
        event: NnrpEvent::none(),
    };
    (*out_result).status
}

#[no_mangle]
/// # Safety
///
/// `event` must be a valid readable pointer to one `NnrpEvent`. The callback
/// must not retain the event pointer after it returns.
pub unsafe extern "C" fn nnrp_dispatch_event(
    sink: NnrpCallbackSink,
    event: *const NnrpEvent,
) -> NnrpFfiStatus {
    if event.is_null() {
        return NnrpFfiStatus::invalid_argument(14);
    }
    let Some(callback) = sink.on_event else {
        return NnrpFfiStatus::invalid_argument(15);
    };

    let callback_status = callback(sink.user_data, event);
    if callback_status != NnrpFfiStatusCode::Ok as u32 {
        return NnrpFfiStatus {
            status_code: NnrpFfiStatusCode::CallbackRejected as u32,
            error_family: NnrpErrorFamily::None as u32,
            protocol_error_code: 0,
            detail_code: callback_status,
        };
    }

    NnrpFfiStatus::ok()
}

pub fn session_error_status(error_code: u32) -> NnrpFfiStatus {
    match error_code {
        SESSION_ERROR_NONE => NnrpFfiStatus::ok(),
        SESSION_ERROR_RESUME_REJECTED
        | SESSION_ERROR_PROFILE_UNSUPPORTED
        | SESSION_ERROR_SCHEMA_UNSUPPORTED => {
            NnrpFfiStatus::protocol(NnrpErrorFamily::Session, error_code)
        }
        _ => NnrpFfiStatus::protocol(NnrpErrorFamily::Session, error_code),
    }
}

pub fn schema_registry_failure_status(failure: SchemaRegistryFailure) -> NnrpFfiStatus {
    NnrpFfiStatus::protocol(NnrpErrorFamily::Schema, failure.error_code())
}

pub fn schema_registry_action_code(action: SchemaRegistryAction) -> u32 {
    match action {
        SchemaRegistryAction::Installed => NNRP_SCHEMA_REGISTRY_ACTION_INSTALLED,
        SchemaRegistryAction::AlreadyInstalled => NNRP_SCHEMA_REGISTRY_ACTION_ALREADY_INSTALLED,
        SchemaRegistryAction::Updated => NNRP_SCHEMA_REGISTRY_ACTION_UPDATED,
        SchemaRegistryAction::Invalidated => NNRP_SCHEMA_REGISTRY_ACTION_INVALIDATED,
    }
}

pub fn cache_validation_failure_status(failure: CacheValidationFailure) -> NnrpFfiStatus {
    NnrpFfiStatus::protocol(NnrpErrorFamily::Cache, failure.error_code())
}

fn cache_owner_handle_kind(kind: u32) -> Result<NnrpHandleKind, NnrpFfiStatus> {
    match kind {
        value if value == NnrpHandleKind::Connection as u32 => Ok(NnrpHandleKind::Connection),
        value if value == NnrpHandleKind::Session as u32 => Ok(NnrpHandleKind::Session),
        value if value == NnrpHandleKind::Operation as u32 => Ok(NnrpHandleKind::Operation),
        _ => Err(NnrpFfiStatus::invalid_handle(kind)),
    }
}

fn cache_owner_scope(kind: u32) -> CacheLeaseOwnerScope {
    match kind {
        value if value == NnrpHandleKind::Session as u32 => CacheLeaseOwnerScope::Session,
        value if value == NnrpHandleKind::Operation as u32 => CacheLeaseOwnerScope::Operation,
        _ => CacheLeaseOwnerScope::Connection,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ptr;

    fn flow_update_payload() -> Vec<u8> {
        FlowUpdateMetadata {
            scope_kind: nnrp_core::FlowScopeKind::Session,
            update_reason: nnrp_core::FlowUpdateReason::Grant,
            backpressure_level: nnrp_core::BackpressureLevel::None,
            connection_credit: 0,
            session_credit: 7,
            operation_credit: 0,
            operation_id: 0,
            retry_after_ms: 0,
            credit_epoch: 1,
            flow_flags: nnrp_core::FLOW_UPDATE_FLAG_CREDIT_VALID,
        }
        .to_bytes()
        .expect("flow update metadata should encode")
        .to_vec()
    }

    #[cfg(feature = "transport-quic")]
    const fn test_transport_id() -> u32 {
        TransportId::Quic as u32
    }

    #[cfg(all(not(feature = "transport-quic"), feature = "transport-tcp"))]
    const fn test_transport_id() -> u32 {
        TransportId::Tcp as u32
    }

    #[cfg(all(
        not(feature = "transport-quic"),
        not(feature = "transport-tcp"),
        feature = "transport-ipc"
    ))]
    const fn test_transport_id() -> u32 {
        TransportId::Ipc as u32
    }

    #[cfg(all(
        not(feature = "transport-quic"),
        not(feature = "transport-tcp"),
        not(feature = "transport-ipc"),
        feature = "transport-websocket"
    ))]
    const fn test_transport_id() -> u32 {
        TransportId::WebSocket as u32
    }

    #[test]
    fn ffi_current_version_stays_aligned() {
        let version = current_protocol_version();
        assert_eq!(version.major, 1);
        assert_eq!(version.wire_format, 0);
        assert_eq!(nnrp_current_protocol_version(), version);
    }

    #[test]
    fn ffi_cache_identity_layout_is_frozen_for_abi_v3() {
        assert_eq!(core::mem::size_of::<NnrpCacheObjectId>(), 24);
        assert_eq!(core::mem::offset_of!(NnrpCacheObjectId, cache_namespace), 0);
        assert_eq!(core::mem::offset_of!(NnrpCacheObjectId, object_kind), 4);
        assert_eq!(core::mem::offset_of!(NnrpCacheObjectId, cache_key_hi), 8);
        assert_eq!(core::mem::offset_of!(NnrpCacheObjectId, cache_key_lo), 16);

        assert_eq!(core::mem::size_of::<NnrpCacheReferenceDescriptor>(), 56);
        assert_eq!(
            core::mem::offset_of!(NnrpCacheReferenceDescriptor, cache_namespace),
            0
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheReferenceDescriptor, profile_id),
            4
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheReferenceDescriptor, reuse_scope),
            6
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheReferenceDescriptor, cache_key_hi),
            8
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheReferenceDescriptor, cache_key_lo),
            16
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheReferenceDescriptor, lease_id),
            24
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheReferenceDescriptor, producer_trace_id),
            32
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheReferenceDescriptor, expiration_hint_ms),
            40
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheReferenceDescriptor, metadata_bytes),
            44
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheReferenceDescriptor, flags),
            48
        );

        assert_eq!(core::mem::size_of::<NnrpCacheMissDescriptor>(), 32);
        assert_eq!(
            core::mem::offset_of!(NnrpCacheMissDescriptor, cache_namespace),
            0
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheMissDescriptor, miss_reason),
            4
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheMissDescriptor, profile_id),
            6
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheMissDescriptor, cache_key_hi),
            8
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheMissDescriptor, cache_key_lo),
            16
        );
        assert_eq!(
            core::mem::offset_of!(NnrpCacheMissDescriptor, diagnostic_bytes),
            24
        );
    }

    #[test]
    fn ffi_runtime_capabilities_report_stable_probe_contract() {
        let capabilities = runtime_capabilities();
        assert_eq!(nnrp_runtime_capabilities(), capabilities);
        assert_eq!(capabilities.abi_major, NNRP_FFI_ABI_MAJOR);
        assert_eq!(capabilities.abi_minor, NNRP_FFI_ABI_MINOR);
        assert_eq!(capabilities.abi_patch, NNRP_FFI_ABI_PATCH);
        assert_eq!(capabilities.reserved0, 0);
        assert_eq!(capabilities.protocol_version, current_protocol_version());
        assert_eq!(capabilities.sdk_major, SDK_MAJOR);
        assert_eq!(capabilities.sdk_minor, SDK_MINOR);
        assert_eq!(capabilities.sdk_patch, SDK_PATCH);
        assert_eq!(capabilities.sdk_preview, SDK_PREVIEW);
        assert_eq!(capabilities.sdk_revision, SDK_REVISION);
        let reported_version = if capabilities.sdk_preview == 0 && capabilities.sdk_revision == 0 {
            format!(
                "{}.{}.{}",
                capabilities.sdk_major, capabilities.sdk_minor, capabilities.sdk_patch
            )
        } else {
            format!(
                "{}.{}.{}-preview.{}.{}",
                capabilities.sdk_major,
                capabilities.sdk_minor,
                capabilities.sdk_patch,
                capabilities.sdk_preview,
                capabilities.sdk_revision
            )
        };
        assert_eq!(reported_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(capabilities.reserved1, 0);
        assert_eq!(capabilities.transport_slots, enabled_transport_slots());
        assert_eq!(transport_slot_bit(TransportId::Unspecified), 0);
        assert_eq!(
            transport_slot_bit(TransportId::Quic),
            NNRP_TRANSPORT_SLOT_QUIC
        );
        assert_eq!(
            transport_slot_bit(TransportId::Tcp),
            NNRP_TRANSPORT_SLOT_TCP
        );
        assert_eq!(
            transport_slot_bit(TransportId::Ipc),
            NNRP_TRANSPORT_SLOT_IPC
        );
        assert_eq!(
            transport_slot_bit(TransportId::WebSocket),
            NNRP_TRANSPORT_SLOT_WEBSOCKET
        );
        assert_eq!(
            enabled_transport_slots() & NNRP_TRANSPORT_SLOT_IPC != 0,
            cfg!(feature = "transport-ipc")
        );
        assert_eq!(
            enabled_transport_slots() & NNRP_TRANSPORT_SLOT_WEBSOCKET != 0,
            cfg!(feature = "transport-websocket")
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_PROTOCOL_CORE,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_CLIENT_API,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_SERVER_API,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_EVENT_POLLING,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_CALLBACK_DISPATCH,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_CACHE_SCHEMA,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_RECOVERY,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_TYPED_PAYLOAD,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_TRANSPORT_SLOTS,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_BATCH_POLLING,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_CACHE_LEASE_OPS,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_SCHEMA_REGISTRY_HANDLES,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_BUFFER_HANDLES,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_EXECUTABLE_RESUME,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_PREVIEW4_CONTROL_EVENTS,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_PREVIEW4_OBJECT_CACHE_EVENTS,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_PREVIEW4_RUNTIME_FRAME_SEND,
            0
        );
    }

    #[test]
    fn ffi_preview4_object_descriptors_round_trip_core_layouts() {
        let object = ObjectDescriptorMetadata {
            object_id: 1,
            object_kind: RuntimeObjectKind::Tensor,
            producer_role: RuntimeRole::Runtime,
            consumer_role: RuntimeRole::Client,
            session_id: 2,
            byte_size: 4096,
            compute_cost_units: 7,
            memory_location_hint: MemoryLocationHint::DeviceMemory,
            ownership_hint: OwnershipHint::Borrowed,
            lifetime_hint_ms: 500,
            metadata_bytes: 12,
        };
        let ffi_object: NnrpRuntimeObjectDescriptor = object.into();
        assert_eq!(ffi_object.to_core().unwrap(), object);

        let reference = ObjectReferenceMetadata {
            object_id: 1,
            operation_id: 3,
            object_version: 4,
            offset: 8,
            length: 16,
            flags: 0x07,
            metadata_bytes: 20,
        };
        let ffi_reference: NnrpObjectReferenceDescriptor = reference.into();
        assert_eq!(ObjectReferenceMetadata::from(ffi_reference), reference);

        let release = ObjectReleaseMetadata {
            object_id: 1,
            operation_id: 3,
            release_reason: ObjectReleaseReason::Cancelled,
            source_role: RuntimeRole::Scheduler,
            flags: 0x03,
            diagnostic_bytes: 8,
        };
        let ffi_release: NnrpObjectReleaseDescriptor = release.into();
        assert_eq!(ffi_release.to_core().unwrap(), release);

        let delta = ObjectDeltaMetadata {
            object_id: 1,
            delta_sequence: 2,
            region_offset: 64,
            region_bytes: 32,
            delta_bytes: 16,
            flags: 0x07,
            metadata_bytes: 4,
        };
        let ffi_delta: NnrpObjectDeltaDescriptor = delta.into();
        assert_eq!(ObjectDeltaMetadata::from(ffi_delta), delta);

        let cache_ref = CacheReferenceMetadata {
            cache_namespace: 7,
            cache_key_hi: 0x1122,
            cache_key_lo: 0x3344,
            profile_id: 3,
            reuse_scope: CacheReuseScope::Session,
            lease_id: 5,
            producer_trace_id: 6,
            expiration_hint_ms: 700,
            metadata_bytes: 24,
            flags: 0x03,
        };
        let ffi_cache_ref: NnrpCacheReferenceDescriptor = cache_ref.into();
        assert_eq!(ffi_cache_ref.to_core().unwrap(), cache_ref);

        let miss = CacheMissMetadata {
            cache_namespace: 7,
            cache_key_hi: 0x1122,
            cache_key_lo: 0x3344,
            miss_reason: CacheMissReason::SchemaMismatch,
            profile_id: 3,
            diagnostic_bytes: 9,
        };
        let ffi_miss: NnrpCacheMissDescriptor = miss.into();
        assert_eq!(ffi_miss.to_core().unwrap(), miss);
    }

    #[test]
    fn ffi_preview4_object_descriptors_reject_unknown_registry_values() {
        let bad_object = NnrpRuntimeObjectDescriptor {
            object_id: 1,
            object_kind: 0x000d,
            producer_role: RuntimeRole::Runtime as u8,
            consumer_role: RuntimeRole::Client as u8,
            session_id: 2,
            byte_size: 4096,
            compute_cost_units: 7,
            memory_location_hint: MemoryLocationHint::DeviceMemory as u16,
            ownership_hint: OwnershipHint::Borrowed as u16,
            lifetime_hint_ms: 500,
            metadata_bytes: 12,
        };
        assert_eq!(
            bad_object.to_core(),
            Err(NnrpFfiStatus {
                status_code: NnrpFfiStatusCode::ProtocolError as u32,
                error_family: NnrpErrorFamily::RuntimeObject as u32,
                protocol_error_code: 0,
                detail_code: 0,
            })
        );

        let bad_release = NnrpObjectReleaseDescriptor {
            object_id: 1,
            operation_id: 2,
            release_reason: ObjectReleaseReason::Cancelled as u16,
            source_role: 0x08,
            flags: 0,
            diagnostic_bytes: 0,
        };
        assert_eq!(
            bad_release.to_core(),
            Err(NnrpFfiStatus {
                status_code: NnrpFfiStatusCode::ProtocolError as u32,
                error_family: NnrpErrorFamily::RuntimeObject as u32,
                protocol_error_code: 0,
                detail_code: 0,
            })
        );
    }

    #[test]
    fn ffi_preview4_control_descriptors_round_trip_core_layouts() {
        let control = ControlRequestMetadata {
            operation_id: 11,
            control_sequence: 12,
            reason_code: 3,
            source_role: RuntimeRole::Scheduler as u8,
            flags: 1,
            diagnostic_bytes: 4,
        };
        let ffi_control: NnrpControlRequestDescriptor = control.into();
        assert_eq!(ControlRequestMetadata::from(ffi_control), control);

        let scheduling = SchedulingMetadata {
            operation_id: 11,
            control_sequence: 12,
            priority_class: 2,
            priority_delta: -4,
            deadline_unix_ms: 99,
            flags: 1,
        };
        let ffi_scheduling: NnrpSchedulingDescriptor = scheduling.into();
        assert_eq!(SchedulingMetadata::from(ffi_scheduling), scheduling);

        let supersede = SupersedeMetadata {
            old_operation_id: 11,
            new_operation_id: 12,
            control_sequence: 13,
            drop_reason_code: 4,
            flags: 1,
            diagnostic_bytes: 5,
        };
        let ffi_supersede: NnrpSupersedeDescriptor = supersede.into();
        assert_eq!(SupersedeMetadata::from(ffi_supersede), supersede);

        let budget = BudgetMetadata {
            operation_id: 11,
            compute_budget_units: 12,
            memory_budget_bytes: 13,
            bandwidth_budget_bytes: 14,
            token_budget: 15,
            flags: 1,
        };
        let ffi_budget: NnrpBudgetDescriptor = budget.into();
        assert_eq!(BudgetMetadata::from(ffi_budget), budget);

        let progress = ProgressMetadata {
            operation_id: 11,
            progress_sequence: 12,
            stage_code: 3,
            percent_x100: 2500,
            object_id: 14,
            body_bytes: 15,
        };
        let ffi_progress: NnrpProgressDescriptor = progress.into();
        assert_eq!(ProgressMetadata::from(ffi_progress), progress);

        let partial = PartialResultMetadata {
            operation_id: 11,
            result_sequence: 12,
            object_id: 13,
            delta_sequence: 14,
            body_bytes: 15,
            flags: 1,
        };
        let ffi_partial: NnrpPartialResultDescriptor = partial.into();
        assert_eq!(PartialResultMetadata::from(ffi_partial), partial);

        let pressure = PressureMetadata {
            scope_id: 11,
            credit_window: 12,
            pressure_level: 2,
            pressure_reason: 3,
            retry_after_ms: 4,
            flags: 1,
        };
        let ffi_pressure: NnrpPressureDescriptor = pressure.into();
        assert_eq!(PressureMetadata::from(ffi_pressure), pressure);

        let capability = CapabilityMetadata {
            profile_id: 1,
            capability_count: 2,
            cost_model_id: 3,
            preference_rank: 4,
            limit_bytes: 5,
            limit_units: 6,
            body_bytes: 7,
            flags: 1,
        };
        let ffi_capability: NnrpCapabilityDescriptor = capability.into();
        assert_eq!(CapabilityMetadata::from(ffi_capability), capability);

        let route = RouteHintMetadata {
            operation_id: 11,
            route_id: 12,
            executor_class: 3,
            affinity_class: 4,
            deadline_unix_ms: 15,
            body_bytes: 6,
            flags: 1,
        };
        let ffi_route: NnrpRouteHintDescriptor = route.into();
        assert_eq!(RouteHintMetadata::from(ffi_route), route);

        let trace = TraceContextMetadata {
            trace_id: 11,
            span_id: 12,
            parent_span_id: 13,
            stage_code: 4,
            flags: 1,
            body_bytes: 5,
        };
        let ffi_trace: NnrpTraceContextDescriptor = trace.into();
        assert_eq!(TraceContextMetadata::from(ffi_trace), trace);

        let drop_reason = ResultDropReasonMetadata {
            operation_id: 11,
            result_sequence: 12,
            drop_reason_code: 3,
            source_role: RuntimeRole::Runtime as u8,
            flags: 1,
            diagnostic_bytes: 4,
        };
        let ffi_drop_reason: NnrpResultDropReasonDescriptor = drop_reason.into();
        assert_eq!(ResultDropReasonMetadata::from(ffi_drop_reason), drop_reason);

        let recoverable = RecoverableErrorMetadata {
            error_code: 1,
            error_scope: ErrorScope::Frame,
            recovery_action: 2,
            source_role: RuntimeRole::Runtime as u8,
            flags: 1,
            retry_after_ms: 3,
            related_session_id: 4,
            related_frame_id: 5,
            related_view_id: 6,
            diagnostic_bytes: 7,
        };
        let ffi_recoverable: NnrpRecoverableErrorDescriptor = recoverable.into();
        assert_eq!(ffi_recoverable.to_core().unwrap(), recoverable);

        let retry = RetryAfterMetadata {
            scope_id: 11,
            control_sequence: 12,
            retry_after_ms: 3,
            jitter_ms: 4,
            reason_code: 5,
            source_role: RuntimeRole::Runtime as u8,
            flags: 1,
            diagnostic_bytes: 6,
        };
        let ffi_retry: NnrpRetryAfterDescriptor = retry.into();
        assert_eq!(RetryAfterMetadata::from(ffi_retry), retry);
    }

    #[test]
    fn ffi_preview4_control_descriptors_reject_unknown_error_scope() {
        let bad = NnrpRecoverableErrorDescriptor {
            error_code: 1,
            error_scope: 99,
            recovery_action: 2,
            source_role: RuntimeRole::Runtime as u8,
            flags: 0,
            retry_after_ms: 3,
            related_session_id: 4,
            related_frame_id: 5,
            related_view_id: 6,
            diagnostic_bytes: 7,
        };
        assert_eq!(
            bad.to_core(),
            Err(NnrpFfiStatus {
                status_code: NnrpFfiStatusCode::ProtocolError as u32,
                error_family: NnrpErrorFamily::Control as u32,
                protocol_error_code: 0,
                detail_code: 0,
            })
        );
    }

    #[test]
    fn ffi_transport_scope_rejects_unavailable_or_unknown_transport_ids() {
        assert_eq!(
            transport_id_enabled(TransportId::Tcp as u32),
            cfg!(feature = "transport-tcp")
        );
        assert_eq!(
            transport_id_enabled(TransportId::Quic as u32),
            cfg!(feature = "transport-quic")
        );
        assert_eq!(
            transport_id_enabled(TransportId::Ipc as u32),
            cfg!(feature = "transport-ipc")
        );
        assert_eq!(
            transport_id_enabled(TransportId::WebSocket as u32),
            cfg!(feature = "transport-websocket")
        );
        assert!(!transport_id_enabled(TransportId::Unspecified as u32));
        assert!(!transport_id_enabled(99));

        let mut client = NnrpHandle::invalid();
        let client_status = unsafe {
            nnrp_client_connect(
                NnrpClientConnectRequest {
                    connection_id: 12,
                    generation: 1,
                    transport_id: TransportId::Unspecified as u32,
                },
                &mut client,
            )
        };
        assert_eq!(client_status, NnrpFfiStatus::invalid_argument(46));
        assert_eq!(client, NnrpHandle::invalid());

        let mut server = NnrpHandle::invalid();
        let server_status = unsafe {
            nnrp_server_bind(
                NnrpServerBindRequest {
                    server_id: 13,
                    generation: 1,
                    transport_id: 99,
                },
                &mut server,
            )
        };
        assert_eq!(server_status, NnrpFfiStatus::invalid_argument(47));
        assert_eq!(server, NnrpHandle::invalid());
    }

    #[cfg(all(feature = "transport-tcp", not(feature = "transport-quic")))]
    #[test]
    fn ffi_tcp_scoped_build_rejects_quic_connection_open() {
        let mut handle = NnrpHandle::invalid();
        let status = unsafe {
            nnrp_client_connect(
                NnrpClientConnectRequest {
                    connection_id: 10,
                    generation: 1,
                    transport_id: TransportId::Quic as u32,
                },
                &mut handle,
            )
        };

        assert_eq!(status, NnrpFfiStatus::invalid_argument(46));
        assert_eq!(handle, NnrpHandle::invalid());
    }

    #[cfg(all(feature = "transport-quic", not(feature = "transport-tcp")))]
    #[test]
    fn ffi_quic_scoped_build_rejects_tcp_server_bind() {
        let mut handle = NnrpHandle::invalid();
        let status = unsafe {
            nnrp_server_bind(
                NnrpServerBindRequest {
                    server_id: 11,
                    generation: 1,
                    transport_id: TransportId::Tcp as u32,
                },
                &mut handle,
            )
        };

        assert_eq!(status, NnrpFfiStatus::invalid_argument(47));
        assert_eq!(handle, NnrpHandle::invalid());
    }

    #[test]
    fn ffi_handles_validate_kind_and_generation() {
        let connection = NnrpHandle::new(NnrpHandleKind::Connection, 7, 1);
        assert_eq!(connection.validate_kind(NnrpHandleKind::Connection), Ok(()));
        assert_eq!(
            connection.validate_kind(NnrpHandleKind::Session),
            Err(NnrpFfiStatus::invalid_handle(
                NnrpHandleKind::Session as u32
            ))
        );
        assert_eq!(
            NnrpHandle::new(NnrpHandleKind::Connection, 0, 1)
                .validate_kind(NnrpHandleKind::Connection),
            Err(NnrpFfiStatus::invalid_handle(
                NnrpHandleKind::Connection as u32
            ))
        );
    }

    #[test]
    fn ffi_handle_ids_do_not_reanimate_released_handles() {
        let mut store = NnrpFfiHandleStore::default();
        let session = NnrpHandle::new(NnrpHandleKind::Session, 1, 1);
        let first = NnrpHandle::new(
            NnrpHandleKind::Operation,
            next_handle_id(&mut store, NnrpHandleKind::Operation),
            session.generation,
        );
        store
            .insert(
                first,
                NnrpFfiResource::Operation {
                    session,
                    operation_id: 1,
                    frame_id: 1,
                    payload_len: 0,
                },
            )
            .expect("first operation should be inserted");
        store
            .remove(first, NnrpHandleKind::Operation)
            .expect("first operation should be released");

        let second = NnrpHandle::new(
            NnrpHandleKind::Operation,
            next_handle_id(&mut store, NnrpHandleKind::Operation),
            session.generation,
        );
        store
            .insert(
                second,
                NnrpFfiResource::Operation {
                    session,
                    operation_id: 2,
                    frame_id: 2,
                    payload_len: 0,
                },
            )
            .expect("second operation should be inserted");

        assert_ne!(first.id, second.id);
        assert!(matches!(
            store.get(first, NnrpHandleKind::Operation),
            Err(status)
                if status
                    == NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32)
        ));
        assert!(store.get(second, NnrpHandleKind::Operation).is_ok());
    }

    #[test]
    fn ffi_buffer_views_reject_null_non_empty_regions() {
        assert_eq!(NnrpBufferView::empty().validate(), Ok(()));
        assert_eq!(
            NnrpBufferView {
                ptr: ptr::null(),
                len: 1
            }
            .validate(),
            Err(NnrpFfiStatus::invalid_argument(1))
        );
        assert_eq!(
            NnrpBufferViewMut {
                ptr: ptr::null_mut(),
                len: 1
            }
            .validate(),
            Err(NnrpFfiStatus::invalid_argument(2))
        );
    }

    #[test]
    fn ffi_entrypoints_bootstrap_open_submit_and_close_handles() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_connection_bootstrap(
                    NnrpConnectionBootstrap {
                        connection_id: 77,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(connection.kind, NnrpHandleKind::Connection as u32);

            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_session_open(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 42,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(session.kind, NnrpHandleKind::Session as u32);

            let payload = [1u8, 2, 3];
            let mut operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_submit(
                    NnrpSubmitRequest {
                        session,
                        operation_id: 100,
                        frame_id: 9,
                        payload: NnrpBufferView {
                            ptr: payload.as_ptr(),
                            len: payload.len(),
                        },
                    },
                    &mut operation
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(operation.kind, NnrpHandleKind::Operation as u32);
            assert_eq!(nnrp_session_close(session), NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_session_close(session),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
            );
        }
    }

    #[test]
    fn ffi_client_abi_emits_pollable_runtime_events() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_001,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut result = empty_poll_result();
            assert_eq!(
                nnrp_client_await_event(connection, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.has_event, 1);
            assert_eq!(result.event.kind, NnrpEventKind::ConnectionOpened as u32);
            assert_eq!(result.event.connection, connection);

            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_002,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_client_await_event(connection, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::SessionOpened as u32);
            assert_eq!(result.event.session, session);

            let payload = [9u8, 8, 7];
            let mut operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_submit(
                    NnrpSubmitRequest {
                        session,
                        operation_id: 91_003,
                        frame_id: 44,
                        payload: NnrpBufferView {
                            ptr: payload.as_ptr(),
                            len: payload.len(),
                        },
                    },
                    &mut operation
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_client_await_event(connection, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::SubmitAccepted as u32);
            assert_eq!(result.event.operation, operation);
            assert_eq!(result.event.frame_id, 44);
            assert_eq!(result.event.payload, NnrpBufferView::empty());

            assert_eq!(
                nnrp_client_cancel(NnrpClientCancelRequest {
                    session,
                    frame_id: 44,
                }),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_client_await_event(connection, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::Control as u32);
            assert_eq!(result.event.frame_id, 44);

            assert_eq!(nnrp_client_close(session), NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_client_await_event(connection, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::SessionClosed as u32);

            let would_block = nnrp_client_await_event(connection, &mut result);
            assert_eq!(
                would_block.status_code,
                NnrpFfiStatusCode::WouldBlock as u32
            );
            assert_eq!(result.has_event, 0);

            let mut reopened_session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_002,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut reopened_session
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(reopened_session, session);
            assert_eq!(
                nnrp_client_await_event(connection, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::SessionOpened as u32);

            let mut reopened_operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_submit(
                    NnrpSubmitRequest {
                        session: reopened_session,
                        operation_id: 91_003,
                        frame_id: 45,
                        payload: NnrpBufferView {
                            ptr: payload.as_ptr(),
                            len: payload.len(),
                        },
                    },
                    &mut reopened_operation
                ),
                NnrpFfiStatus::ok()
            );
            assert_ne!(reopened_operation, operation);
            assert_eq!(
                nnrp_client_close_connection(connection),
                NnrpFfiStatus::ok()
            );
        }
    }

    #[test]
    fn ffi_client_abi_batch_polls_runtime_events() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_101,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_102,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            let mut operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_submit(
                    NnrpSubmitRequest {
                        session,
                        operation_id: 91_103,
                        frame_id: 7,
                        payload: NnrpBufferView::empty(),
                    },
                    &mut operation
                ),
                NnrpFfiStatus::ok()
            );

            let mut events = [NnrpEvent::none(); 4];
            let mut event_count = 0usize;
            assert_eq!(
                nnrp_client_await_events(
                    connection,
                    events.as_mut_ptr(),
                    events.len(),
                    &mut event_count
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(event_count, 3);
            assert_eq!(events[0].kind, NnrpEventKind::ConnectionOpened as u32);
            assert_eq!(events[1].kind, NnrpEventKind::SessionOpened as u32);
            assert_eq!(events[2].kind, NnrpEventKind::SubmitAccepted as u32);
            assert_eq!(events[2].operation, operation);

            assert_eq!(
                nnrp_client_await_events(
                    connection,
                    events.as_mut_ptr(),
                    events.len(),
                    &mut event_count
                ),
                NnrpFfiStatus {
                    status_code: NnrpFfiStatusCode::WouldBlock as u32,
                    error_family: NnrpErrorFamily::None as u32,
                    protocol_error_code: 0,
                    detail_code: 0,
                }
            );
            assert_eq!(event_count, 0);
        }
    }

    #[test]
    fn ffi_client_completion_helpers_emit_terminal_events() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_401,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_402,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let payload = [1u8, 2, 3];
            let mut completed_operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_submit(
                    NnrpSubmitRequest {
                        session,
                        operation_id: 91_403,
                        frame_id: 55,
                        payload: NnrpBufferView {
                            ptr: payload.as_ptr(),
                            len: payload.len(),
                        },
                    },
                    &mut completed_operation
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);
            assert_eq!(
                benchmark_client_complete_operation(NnrpClientCompleteOperationRequest {
                    operation: completed_operation,
                    payload: NnrpBufferView {
                        ptr: payload.as_ptr(),
                        len: payload.len(),
                    },
                }),
                NnrpFfiStatus::ok()
            );

            let mut result = empty_poll_result();
            assert_eq!(
                nnrp_client_await_event(connection, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.has_event, 1);
            assert_eq!(result.event.kind, NnrpEventKind::ResultPushed as u32);
            assert_eq!(result.event.operation, completed_operation);
            assert_eq!(result.event.frame_id, 55);
            assert_eq!(
                benchmark_client_complete_operation(NnrpClientCompleteOperationRequest {
                    operation: completed_operation,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32)
            );

            let mut dropped_operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_submit(
                    NnrpSubmitRequest {
                        session,
                        operation_id: 91_404,
                        frame_id: 56,
                        payload: NnrpBufferView::empty(),
                    },
                    &mut dropped_operation
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);
            assert_eq!(
                benchmark_client_drop_operation(NnrpClientDropOperationRequest {
                    operation: dropped_operation,
                }),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_client_await_event(connection, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::ResultDropped as u32);
            assert_eq!(result.event.operation, dropped_operation);
            assert_eq!(result.event.frame_id, 56);
        }
    }

    #[test]
    fn benchmark_ffi_client_submit_result_coalesces_hot_path() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_421,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_422,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let submit_payload = [1u8, 2, 3];
            let result_payload = [4u8, 5, 6];
            let mut operation = NnrpHandle::invalid();
            let mut result = empty_poll_result();
            assert_eq!(
                benchmark_client_submit_result(
                    NnrpClientSubmitResultRequest {
                        session,
                        operation_id: 91_423,
                        frame_id: 58,
                        submit_payload: NnrpBufferView {
                            ptr: submit_payload.as_ptr(),
                            len: submit_payload.len(),
                        },
                        result_payload: NnrpBufferView {
                            ptr: result_payload.as_ptr(),
                            len: result_payload.len(),
                        },
                        max_events: 2,
                    },
                    &mut operation,
                    &mut result
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(operation.kind, NnrpHandleKind::Operation as u32);
            assert_ne!(operation.id, 91_423);
            assert_eq!(result.has_event, 1);
            assert_eq!(result.event.kind, NnrpEventKind::ResultPushed as u32);
            assert_eq!(result.event.session, session);
            assert_eq!(result.event.operation, operation);
            assert_eq!(result.event.frame_id, 58);
            assert_eq!(
                benchmark_client_complete_operation(NnrpClientCompleteOperationRequest {
                    operation,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32)
            );
        }
    }

    #[test]
    fn benchmark_ffi_client_submit_result_compact_coalesces_hot_path() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_426,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_427,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let submit_payload = [1u8, 2, 3];
            let result_payload = [4u8, 5, 6];
            let mut result = NnrpCompactResult::none(NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_benchmark_client_submit_result_compact(
                    NnrpClientSubmitResultRequest {
                        session,
                        operation_id: 91_428,
                        frame_id: 60,
                        submit_payload: NnrpBufferView {
                            ptr: submit_payload.as_ptr(),
                            len: submit_payload.len(),
                        },
                        result_payload: NnrpBufferView {
                            ptr: result_payload.as_ptr(),
                            len: result_payload.len(),
                        },
                        max_events: 2,
                    },
                    &mut result
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.status, NnrpFfiStatus::ok());
            assert_eq!(result.has_result, 1);
            assert_eq!(result.event_kind, NnrpEventKind::ResultPushed as u32);
            assert_eq!(result.result_state, NNRP_RESULT_STATE_COMPLETED);
            assert_eq!(result.operation.kind, NnrpHandleKind::Operation as u32);
            assert_ne!(result.operation.id, 91_428);
            assert_eq!(result.operation_id, 91_428);
            assert_eq!(result.frame_id, 60);
            assert_eq!(result.payload.len, result_payload.len());
            assert_eq!(
                core::slice::from_raw_parts(result.payload.ptr, result.payload.len),
                result_payload
            );
            assert_eq!(result.diagnostic.status, NnrpFfiStatus::ok());
        }
    }

    #[test]
    fn benchmark_ffi_client_submit_result_compact_batch_amortizes_hot_path() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_429,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_430,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let submit_payload = [1u8, 2, 3];
            let result_payload = [4u8, 5, 6];
            let mut result = NnrpCompactResult::none(NnrpFfiStatus::ok());
            let mut completed = 0usize;
            assert_eq!(
                nnrp_benchmark_client_submit_result_compact_batch(
                    NnrpClientSubmitResultBatchRequest {
                        session,
                        operation_id_start: 91_431,
                        frame_id_start: 60,
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
                        iterations: 4,
                    },
                    &mut result,
                    &mut completed,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(completed, 4);
            assert_eq!(result.status, NnrpFfiStatus::ok());
            assert_eq!(result.has_result, 1);
            assert_eq!(result.event_kind, NnrpEventKind::ResultPushed as u32);
            assert_ne!(result.operation.id, 91_434);
            assert_eq!(result.operation_id, 91_434);
            assert_eq!(result.frame_id, 63);
            assert_eq!(result.payload.len, result_payload.len());

            completed = usize::MAX;
            result = NnrpCompactResult::none(NnrpFfiStatus::invalid_argument(1));
            assert_eq!(
                nnrp_benchmark_client_submit_result_compact_batch(
                    NnrpClientSubmitResultBatchRequest {
                        session,
                        operation_id_start: 91_500,
                        frame_id_start: 80,
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
                        iterations: 0,
                    },
                    &mut result,
                    &mut completed,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(completed, 0);
            assert_eq!(result.status, NnrpFfiStatus::ok());
            assert_eq!(result.has_result, 0);

            completed = usize::MAX;
            assert_eq!(
                nnrp_benchmark_client_submit_result_compact_batch(
                    NnrpClientSubmitResultBatchRequest {
                        session,
                        operation_id_start: 91_600,
                        frame_id_start: 90,
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
                        iterations: 1,
                    },
                    core::ptr::null_mut(),
                    &mut completed,
                ),
                NnrpFfiStatus::invalid_argument(126)
            );
            assert_eq!(completed, usize::MAX);

            let mut null_completed_result = NnrpCompactResult::none(NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_benchmark_client_submit_result_compact_batch(
                    NnrpClientSubmitResultBatchRequest {
                        session,
                        operation_id_start: 91_601,
                        frame_id_start: 91,
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
                        iterations: 1,
                    },
                    &mut null_completed_result,
                    core::ptr::null_mut(),
                ),
                NnrpFfiStatus::invalid_argument(126)
            );
        }
    }

    #[test]
    fn benchmark_ffi_client_runtime_object_loop_compact_coalesces_metadata_validation_and_result() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_650,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_651,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let submit_payload = [1u8, 2, 3];
            let object_metadata = [0xa1u8, 0xa2];
            let cache_metadata = [0xb1u8, 0xb2, 0xb3];
            let progress_body = [0xc1u8];
            let partial_body = [0xd1u8, 0xd2];
            let release_diagnostics = [0xe1u8];
            let result_payload = [4u8, 5, 6, 7];
            let mut result = NnrpCompactResult::none(NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_benchmark_client_runtime_object_loop_compact(
                    NnrpClientRuntimeObjectLoopRequest {
                        session,
                        operation_id: 91_652,
                        frame_id: 70,
                        submit_payload: NnrpBufferView {
                            ptr: submit_payload.as_ptr(),
                            len: submit_payload.len(),
                        },
                        object_descriptor: NnrpRuntimeObjectDescriptor {
                            object_id: 91_653,
                            object_kind: RuntimeObjectKind::Tensor as u16,
                            producer_role: RuntimeRole::Server as u8,
                            consumer_role: RuntimeRole::Client as u8,
                            session_id: 91_651,
                            byte_size: 4096,
                            compute_cost_units: 17,
                            memory_location_hint: MemoryLocationHint::DeviceMemory as u16,
                            ownership_hint: OwnershipHint::Borrowed as u16,
                            lifetime_hint_ms: 250,
                            metadata_bytes: object_metadata.len() as u32,
                        },
                        object_metadata: NnrpBufferView {
                            ptr: object_metadata.as_ptr(),
                            len: object_metadata.len(),
                        },
                        cache_reference: NnrpCacheReferenceDescriptor {
                            cache_namespace: 7,
                            cache_key_hi: 0xaaa,
                            cache_key_lo: 0xbbb,
                            profile_id: 2,
                            reuse_scope: CacheReuseScope::Session as u16,
                            lease_id: 91_654,
                            producer_trace_id: 91_655,
                            expiration_hint_ms: 1_000,
                            metadata_bytes: cache_metadata.len() as u32,
                            flags: 0,
                        },
                        cache_reference_metadata: NnrpBufferView {
                            ptr: cache_metadata.as_ptr(),
                            len: cache_metadata.len(),
                        },
                        progress: NnrpProgressDescriptor {
                            operation_id: 91_652,
                            progress_sequence: 1,
                            stage_code: 2,
                            percent_x100: 5_000,
                            object_id: 91_653,
                            body_bytes: progress_body.len() as u32,
                        },
                        progress_body: NnrpBufferView {
                            ptr: progress_body.as_ptr(),
                            len: progress_body.len(),
                        },
                        partial_result: NnrpPartialResultDescriptor {
                            operation_id: 91_652,
                            result_sequence: 1,
                            object_id: 91_653,
                            delta_sequence: 1,
                            body_bytes: partial_body.len() as u32,
                            flags: 0x0000_0002,
                        },
                        partial_body: NnrpBufferView {
                            ptr: partial_body.as_ptr(),
                            len: partial_body.len(),
                        },
                        object_release: NnrpObjectReleaseDescriptor {
                            object_id: 91_653,
                            operation_id: 91_652,
                            release_reason: ObjectReleaseReason::Completed as u16,
                            source_role: RuntimeRole::Server as u8,
                            flags: 0,
                            diagnostic_bytes: release_diagnostics.len() as u32,
                        },
                        release_diagnostics: NnrpBufferView {
                            ptr: release_diagnostics.as_ptr(),
                            len: release_diagnostics.len(),
                        },
                        result_payload: NnrpBufferView {
                            ptr: result_payload.as_ptr(),
                            len: result_payload.len(),
                        },
                        max_events: 2,
                    },
                    &mut result
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.status, NnrpFfiStatus::ok());
            assert_eq!(result.has_result, 1);
            assert_eq!(result.event_kind, NnrpEventKind::ResultPushed as u32);
            assert_eq!(result.result_state, NNRP_RESULT_STATE_COMPLETED);
            assert_eq!(result.operation.kind, NnrpHandleKind::Operation as u32);
            assert_ne!(result.operation.id, 91_652);
            assert_eq!(result.operation_id, 91_652);
            assert_eq!(result.frame_id, 70);
            assert_eq!(result.payload.len, result_payload.len());
            assert_eq!(
                core::slice::from_raw_parts(result.payload.ptr, result.payload.len),
                result_payload
            );
        }
    }

    #[test]
    fn benchmark_ffi_client_runtime_object_loop_compact_rejects_declared_length_mismatch() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_656,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_657,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let payload = [1u8];
            let mut result = NnrpCompactResult::none(NnrpFfiStatus::ok());
            let status = nnrp_benchmark_client_runtime_object_loop_compact(
                NnrpClientRuntimeObjectLoopRequest {
                    session,
                    operation_id: 91_658,
                    frame_id: 71,
                    submit_payload: NnrpBufferView {
                        ptr: payload.as_ptr(),
                        len: payload.len(),
                    },
                    object_descriptor: NnrpRuntimeObjectDescriptor {
                        object_id: 91_659,
                        object_kind: RuntimeObjectKind::Tensor as u16,
                        producer_role: RuntimeRole::Server as u8,
                        consumer_role: RuntimeRole::Client as u8,
                        session_id: 91_657,
                        byte_size: 64,
                        compute_cost_units: 1,
                        memory_location_hint: MemoryLocationHint::DeviceMemory as u16,
                        ownership_hint: OwnershipHint::Borrowed as u16,
                        lifetime_hint_ms: 250,
                        metadata_bytes: 2,
                    },
                    object_metadata: NnrpBufferView {
                        ptr: payload.as_ptr(),
                        len: payload.len(),
                    },
                    cache_reference: NnrpCacheReferenceDescriptor {
                        cache_namespace: 7,
                        cache_key_hi: 1,
                        cache_key_lo: 2,
                        profile_id: 2,
                        reuse_scope: CacheReuseScope::Session as u16,
                        lease_id: 0,
                        producer_trace_id: 0,
                        expiration_hint_ms: 1_000,
                        metadata_bytes: 0,
                        flags: 0,
                    },
                    cache_reference_metadata: NnrpBufferView::empty(),
                    progress: NnrpProgressDescriptor {
                        operation_id: 91_658,
                        progress_sequence: 1,
                        stage_code: 0,
                        percent_x100: 1,
                        object_id: 91_659,
                        body_bytes: 0,
                    },
                    progress_body: NnrpBufferView::empty(),
                    partial_result: NnrpPartialResultDescriptor {
                        operation_id: 91_658,
                        result_sequence: 1,
                        object_id: 91_659,
                        delta_sequence: 0,
                        body_bytes: 0,
                        flags: 0x0000_0002,
                    },
                    partial_body: NnrpBufferView::empty(),
                    object_release: NnrpObjectReleaseDescriptor {
                        object_id: 91_659,
                        operation_id: 91_658,
                        release_reason: ObjectReleaseReason::Completed as u16,
                        source_role: RuntimeRole::Server as u8,
                        flags: 0,
                        diagnostic_bytes: 0,
                    },
                    release_diagnostics: NnrpBufferView::empty(),
                    result_payload: NnrpBufferView {
                        ptr: payload.as_ptr(),
                        len: payload.len(),
                    },
                    max_events: 2,
                },
                &mut result,
            );
            assert_eq!(status.status_code, NnrpFfiStatusCode::ProtocolError as u32);
            assert_eq!(status.error_family, NnrpErrorFamily::RuntimeObject as u32);
            assert_eq!(result.status, status);
            assert_eq!(result.has_result, 0);

            let mut poll = empty_poll_result();
            assert_eq!(
                nnrp_client_await_event(connection, &mut poll).status_code,
                NnrpFfiStatusCode::WouldBlock as u32
            );
            assert_eq!(poll.has_event, 0);
        }
    }

    #[test]
    fn benchmark_ffi_client_submit_result_reports_argument_and_poll_failures() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 92_431,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 92_432,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let mut operation = NnrpHandle::invalid();
            let mut result = empty_poll_result();
            let request = NnrpClientSubmitResultRequest {
                session,
                operation_id: 92_433,
                frame_id: 59,
                submit_payload: NnrpBufferView::empty(),
                result_payload: NnrpBufferView::empty(),
                max_events: 1,
            };
            assert_eq!(
                benchmark_client_submit_result(request, ptr::null_mut(), &mut result),
                NnrpFfiStatus::invalid_argument(47)
            );
            let status = benchmark_client_submit_result(request, &mut operation, &mut result);
            assert_eq!(status.status_code, NnrpFfiStatusCode::WouldBlock as u32);
            assert_eq!(operation.kind, NnrpHandleKind::Operation as u32);
            assert_ne!(operation.id, 92_433);
            assert_eq!(result.has_event, 0);
            assert_eq!(
                benchmark_client_submit_result(
                    NnrpClientSubmitResultRequest {
                        operation_id: 0,
                        ..request
                    },
                    &mut operation,
                    &mut result
                ),
                NnrpFfiStatus::invalid_argument(12)
            );
            let invalid_payload_request = NnrpClientSubmitResultRequest {
                operation_id: 92_434,
                result_payload: NnrpBufferView {
                    ptr: ptr::null(),
                    len: 1,
                },
                ..request
            };
            let previous_operation = operation;
            assert_eq!(
                benchmark_client_submit_result(
                    invalid_payload_request,
                    &mut operation,
                    &mut result
                ),
                NnrpFfiStatus::invalid_argument(1)
            );
            assert_eq!(operation, previous_operation);
            drain_events(connection);
            let mut compact_result = NnrpCompactResult::none(NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_benchmark_client_submit_result_compact(request, ptr::null_mut()),
                NnrpFfiStatus::invalid_argument(48)
            );
            assert_eq!(
                nnrp_benchmark_client_submit_result_compact(
                    NnrpClientSubmitResultRequest {
                        operation_id: 92_435,
                        result_payload: NnrpBufferView {
                            ptr: ptr::null(),
                            len: 1,
                        },
                        ..request
                    },
                    &mut compact_result
                ),
                NnrpFfiStatus::invalid_argument(1)
            );
            assert_eq!(
                nnrp_benchmark_client_submit_result_compact(
                    NnrpClientSubmitResultRequest {
                        operation_id: 0,
                        ..request
                    },
                    &mut compact_result
                ),
                NnrpFfiStatus::invalid_argument(12)
            );
            assert_eq!(compact_result.status, NnrpFfiStatus::invalid_argument(12));
            assert_eq!(compact_result.has_result, 0);
            assert_eq!(
                nnrp_benchmark_client_submit_result_compact(
                    NnrpClientSubmitResultRequest {
                        session: NnrpHandle::new(NnrpHandleKind::Operation, 92_436, 1),
                        operation_id: 92_436,
                        ..request
                    },
                    &mut compact_result
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
            );
            assert_eq!(
                compact_result.status,
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
            );
            assert_eq!(compact_result.has_result, 0);
            let status = nnrp_benchmark_client_submit_result_compact(
                NnrpClientSubmitResultRequest {
                    operation_id: 92_437,
                    max_events: 1,
                    ..request
                },
                &mut compact_result,
            );
            assert_eq!(status.status_code, NnrpFfiStatusCode::WouldBlock as u32);
            assert_eq!(
                compact_result.status.status_code,
                NnrpFfiStatusCode::WouldBlock as u32
            );
            assert_eq!(compact_result.has_result, 0);
            assert_eq!(compact_result.result_state, NNRP_RESULT_STATE_NONE);
            let invalid_session = NnrpHandle::new(NnrpHandleKind::Session, 92_438, 1);
            assert_eq!(
                poll_matching_operation_compact_result(
                    invalid_session,
                    NnrpHandle::new(NnrpHandleKind::Operation, 92_438, 1),
                    92_438,
                    62,
                    NnrpBufferView::empty(),
                    1,
                    &mut compact_result
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
            );
            assert_eq!(
                compact_result.status,
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
            );
        }
    }

    #[test]
    fn ffi_client_control_aliases_emit_flow_update_and_result_hint() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_501,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_502,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);
            let flow_update = flow_update_payload();
            assert_eq!(
                nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                    handle: session,
                    message_type: MessageType::FlowUpdate as u32,
                    frame_id: 57,
                    payload: NnrpBufferView {
                        ptr: flow_update.as_ptr(),
                        len: flow_update.len(),
                    },
                }),
                NnrpFfiStatus::ok()
            );

            let hint = ResultHintMetadata {
                applied_budget_policy: nnrp_core::ResultHintBudgetPolicy::Partial,
                congestion_state: nnrp_core::ResultHintCongestionState::Elevated,
                reason: nnrp_core::ResultHintReason::ServerBusy,
                retry_after_ms: 8,
            }
            .to_bytes()
            .expect("result hint metadata should encode");
            assert_eq!(
                test_client_send_result_hint(NnrpControlRequest {
                    handle: session,
                    control_code: MessageType::ResultHint as u32,
                    payload: NnrpBufferView {
                        ptr: hint.as_ptr(),
                        len: hint.len(),
                    },
                }),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                test_client_send_result_hint(NnrpControlRequest {
                    handle: session,
                    control_code: MessageType::FlowUpdate as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_argument(34)
            );

            let mut events = [NnrpEvent::none(); 2];
            let mut event_count = 0usize;
            assert_eq!(
                nnrp_client_await_events(
                    connection,
                    events.as_mut_ptr(),
                    events.len(),
                    &mut event_count
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(event_count, 2);
            assert_eq!(events[0].kind, NnrpEventKind::FlowUpdated as u32);
            assert_eq!(events[0].frame_id, 57);
            assert_eq!(events[1].kind, NnrpEventKind::ResultHint as u32);
            assert_eq!(events[1].session, session);
        }
    }

    #[test]
    fn ffi_client_submit_control_validates_and_polls_matching_event() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_511,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_512,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let progress_body = [1u8, 2, 3, 4];
            let progress = ProgressMetadata {
                operation_id: 91_513,
                progress_sequence: 1,
                stage_code: 2,
                percent_x100: 2_500,
                object_id: 0,
                body_bytes: progress_body.len() as u32,
            }
            .to_vec_with_body(&progress_body)
            .expect("progress metadata should encode");
            let mut progress_result = empty_poll_result();
            assert_eq!(
                test_client_submit_control(
                    NnrpClientSubmitControlRequest {
                        control: NnrpControlRequest {
                            handle: session,
                            control_code: MessageType::Progress as u32,
                            payload: NnrpBufferView {
                                ptr: progress.as_ptr(),
                                len: progress.len(),
                            },
                        },
                        max_events: 0,
                    },
                    &mut progress_result
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(progress_result.status, NnrpFfiStatus::ok());
            assert_eq!(progress_result.has_event, 1);
            assert_eq!(progress_result.event.kind, NnrpEventKind::Control as u32);
            assert_eq!(progress_result.event.connection, connection);
            assert_eq!(progress_result.event.session, session);
            assert_eq!(progress_result.event.operation, NnrpHandle::invalid());

            let hint = ResultHintMetadata {
                applied_budget_policy: nnrp_core::ResultHintBudgetPolicy::Full,
                congestion_state: nnrp_core::ResultHintCongestionState::Steady,
                reason: nnrp_core::ResultHintReason::None,
                retry_after_ms: 0,
            }
            .to_bytes()
            .expect("result hint metadata should encode");
            let mut hint_result = empty_poll_result();
            assert_eq!(
                test_client_submit_control(
                    NnrpClientSubmitControlRequest {
                        control: NnrpControlRequest {
                            handle: session,
                            control_code: MessageType::ResultHint as u32,
                            payload: NnrpBufferView {
                                ptr: hint.as_ptr(),
                                len: hint.len(),
                            },
                        },
                        max_events: 0,
                    },
                    &mut hint_result
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(hint_result.has_event, 1);
            assert_eq!(hint_result.event.kind, NnrpEventKind::ResultHint as u32);
            assert_eq!(hint_result.event.session, session);
        }
    }

    #[test]
    fn ffi_client_submit_control_rejects_invalid_metadata_and_buffers() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_521,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_522,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let invalid_progress = ProgressMetadata {
                operation_id: 0,
                progress_sequence: 1,
                stage_code: 2,
                percent_x100: 2_500,
                object_id: 0,
                body_bytes: 0,
            }
            .to_bytes()
            .expect("invalid semantic progress should still encode");
            let request = NnrpClientSubmitControlRequest {
                control: NnrpControlRequest {
                    handle: session,
                    control_code: MessageType::Progress as u32,
                    payload: NnrpBufferView {
                        ptr: invalid_progress.as_ptr(),
                        len: invalid_progress.len(),
                    },
                },
                max_events: 0,
            };
            let mut result = empty_poll_result();
            let status = test_client_submit_control(request, &mut result);
            assert_eq!(status.status_code, NnrpFfiStatusCode::ProtocolError as u32);
            assert_eq!(status.error_family, NnrpErrorFamily::Lifecycle as u32);
            assert_eq!(result.status, status);
            assert_eq!(result.has_event, 0);

            let mut invalid_code_result = empty_poll_result();
            assert_eq!(
                test_client_submit_control(
                    NnrpClientSubmitControlRequest {
                        control: NnrpControlRequest {
                            handle: session,
                            control_code: 256,
                            payload: NnrpBufferView::empty(),
                        },
                        max_events: 0,
                    },
                    &mut invalid_code_result
                ),
                NnrpFfiStatus::invalid_argument(128)
            );
            assert_eq!(
                invalid_code_result.status,
                NnrpFfiStatus::invalid_argument(128)
            );
            assert_eq!(invalid_code_result.has_event, 0);

            assert_eq!(
                test_client_submit_control(request, core::ptr::null_mut()),
                NnrpFfiStatus::invalid_argument(127)
            );
        }
    }

    #[test]
    fn ffi_client_submit_control_validator_accepts_preview4_metadata_matrix() {
        fn assert_valid(control_code: MessageType, payload: &[u8]) {
            assert_eq!(
                validate_control_metadata_payload(
                    control_code as u32,
                    NnrpBufferView {
                        ptr: payload.as_ptr(),
                        len: payload.len(),
                    }
                ),
                Ok(())
            );
        }

        let control = ControlRequestMetadata {
            operation_id: 11,
            control_sequence: 12,
            reason_code: 1,
            source_role: 4,
            flags: nnrp_core::CONTROL_REQUEST_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 2,
        }
        .to_vec_with_diagnostics(&[1, 2])
        .expect("control request metadata should encode");
        assert_valid(MessageType::Cancel, &control);
        assert_valid(MessageType::Abort, &control);

        let scheduling = SchedulingMetadata {
            operation_id: 21,
            control_sequence: 22,
            priority_class: 2,
            priority_delta: -1,
            deadline_unix_ms: 99,
            flags: nnrp_core::SCHEDULING_FLAGS_KNOWN_MASK,
        }
        .to_bytes()
        .expect("scheduling metadata should encode");
        assert_valid(MessageType::PriorityUpdate, &scheduling);
        assert_valid(MessageType::Deadline, &scheduling);
        assert_valid(MessageType::ExpireAt, &scheduling);

        let supersede = SupersedeMetadata {
            old_operation_id: 31,
            new_operation_id: 32,
            control_sequence: 33,
            drop_reason_code: 4,
            flags: nnrp_core::SUPERSEDE_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 1,
        }
        .to_vec_with_diagnostics(&[3])
        .expect("supersede metadata should encode");
        assert_valid(MessageType::Supersede, &supersede);

        let budget = BudgetMetadata {
            operation_id: 41,
            compute_budget_units: 42,
            memory_budget_bytes: 43,
            bandwidth_budget_bytes: 44,
            token_budget: 45,
            flags: nnrp_core::BUDGET_FLAGS_KNOWN_MASK,
        }
        .to_bytes()
        .expect("budget metadata should encode");
        assert_valid(MessageType::BudgetUpdate, &budget);

        let progress = ProgressMetadata {
            operation_id: 51,
            progress_sequence: 52,
            stage_code: 5,
            percent_x100: 8_750,
            object_id: 53,
            body_bytes: 2,
        }
        .to_vec_with_body(&[9, 8])
        .expect("progress metadata should encode");
        assert_valid(MessageType::Progress, &progress);

        let partial = PartialResultMetadata {
            operation_id: 61,
            result_sequence: 62,
            object_id: 63,
            delta_sequence: 64,
            body_bytes: 3,
            flags: nnrp_core::PARTIAL_RESULT_FLAGS_KNOWN_MASK,
        }
        .to_vec_with_body(&[7, 8, 9])
        .expect("partial result metadata should encode");
        assert_valid(MessageType::PartialResult, &partial);

        let pressure = PressureMetadata {
            scope_id: 71,
            credit_window: 72,
            pressure_level: 2,
            pressure_reason: 3,
            retry_after_ms: 4,
            flags: nnrp_core::PRESSURE_FLAGS_KNOWN_MASK,
        }
        .to_bytes()
        .expect("pressure metadata should encode");
        assert_valid(MessageType::Backpressure, &pressure);
        assert_valid(MessageType::CreditUpdate, &pressure);

        let capability = CapabilityMetadata {
            profile_id: 0x0100,
            capability_count: 3,
            cost_model_id: 2,
            preference_rank: 1,
            limit_bytes: 81,
            limit_units: 82,
            body_bytes: 2,
            flags: nnrp_core::CAPABILITY_FLAGS_KNOWN_MASK,
        }
        .to_vec_with_body(&[1, 0])
        .expect("capability metadata should encode");
        assert_valid(MessageType::CapabilityNegotiation, &capability);
        assert_valid(MessageType::DegradeProfile, &capability);

        let route = RouteHintMetadata {
            operation_id: 91,
            route_id: 92,
            executor_class: 3,
            affinity_class: 4,
            deadline_unix_ms: 93,
            body_bytes: 4,
            flags: nnrp_core::ROUTE_HINT_FLAGS_KNOWN_MASK,
        }
        .to_vec_with_body(&[1, 2, 3, 4])
        .expect("route hint metadata should encode");
        assert_valid(MessageType::RouteHint, &route);
        assert_valid(MessageType::ExecutionHint, &route);

        let trace = TraceContextMetadata {
            trace_id: 101,
            span_id: 102,
            parent_span_id: 103,
            stage_code: 6,
            flags: nnrp_core::TRACE_CONTEXT_FLAGS_KNOWN_MASK,
            body_bytes: 1,
        }
        .to_vec_with_body(&[5])
        .expect("trace context metadata should encode");
        assert_valid(MessageType::TraceContext, &trace);

        let drop_reason = ResultDropReasonMetadata {
            operation_id: 111,
            result_sequence: 112,
            drop_reason_code: nnrp_core::RESULT_DROP_REASON_DEADLINE_EXPIRED,
            source_role: 6,
            flags: nnrp_core::RESULT_DROP_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 2,
        }
        .to_vec_with_diagnostics(&[6, 7])
        .expect("drop reason metadata should encode");
        assert_valid(MessageType::ResultDropReason, &drop_reason);

        let recoverable = RecoverableErrorMetadata {
            error_code: 121,
            error_scope: ErrorScope::Frame,
            recovery_action: 3,
            source_role: 6,
            flags: nnrp_core::RECOVERABLE_ERROR_FLAGS_KNOWN_MASK,
            retry_after_ms: 122,
            related_session_id: 123,
            related_frame_id: 124,
            related_view_id: 125,
            diagnostic_bytes: 2,
        }
        .to_vec_with_diagnostics(&[8, 9])
        .expect("recoverable error metadata should encode");
        assert_valid(MessageType::ErrorRecoverable, &recoverable);

        let retry_after = RetryAfterMetadata {
            scope_id: 131,
            control_sequence: 132,
            retry_after_ms: 133,
            jitter_ms: 134,
            reason_code: 4,
            source_role: 6,
            flags: nnrp_core::RETRY_AFTER_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 3,
        }
        .to_vec_with_diagnostics(&[1, 3, 5])
        .expect("retry-after metadata should encode");
        assert_valid(MessageType::RetryAfter, &retry_after);
    }

    #[test]
    fn ffi_runtime_frame_validator_accepts_preview4_object_cache_matrix() {
        fn assert_valid(message_type: MessageType, payload: &[u8]) {
            assert_eq!(
                validate_runtime_frame_payload(
                    message_type as u32,
                    NnrpBufferView {
                        ptr: payload.as_ptr(),
                        len: payload.len(),
                    },
                ),
                Ok(message_type)
            );
        }

        let object = ObjectDescriptorMetadata {
            object_id: 1,
            object_kind: RuntimeObjectKind::Tensor,
            producer_role: RuntimeRole::Runtime,
            consumer_role: RuntimeRole::Client,
            session_id: 2,
            byte_size: 4096,
            compute_cost_units: 7,
            memory_location_hint: MemoryLocationHint::DeviceMemory,
            ownership_hint: OwnershipHint::Borrowed,
            lifetime_hint_ms: 500,
            metadata_bytes: 1,
        }
        .to_vec_with_extension(&[1])
        .expect("object descriptor should encode");
        assert_valid(MessageType::ObjectDeclare, &object);

        let reference = ObjectReferenceMetadata {
            object_id: 1,
            operation_id: 3,
            object_version: 4,
            offset: 8,
            length: 16,
            flags: nnrp_core::OBJECT_REFERENCE_FLAGS_KNOWN_MASK,
            metadata_bytes: 1,
        }
        .to_vec_with_extension(&[2])
        .expect("object reference should encode");
        assert_valid(MessageType::ObjectRef, &reference);

        let release = ObjectReleaseMetadata {
            object_id: 1,
            operation_id: 3,
            release_reason: ObjectReleaseReason::Cancelled,
            source_role: RuntimeRole::Scheduler,
            flags: nnrp_core::OBJECT_RELEASE_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 1,
        }
        .to_vec_with_diagnostics(&[3])
        .expect("object release should encode");
        assert_valid(MessageType::ObjectRelease, &release);

        let mut delta = ObjectDeltaMetadata {
            object_id: 1,
            delta_sequence: 2,
            region_offset: 64,
            region_bytes: 32,
            delta_bytes: 16,
            flags: nnrp_core::OBJECT_DELTA_FLAGS_KNOWN_MASK,
            metadata_bytes: 1,
        }
        .to_vec_with_extension(&[4])
        .expect("object delta should encode");
        delta.extend_from_slice(&[5; 16]);
        assert_valid(MessageType::ObjectPatch, &delta);
        assert_valid(MessageType::ObjectDelta, &delta);

        let cache_reference = CacheReferenceMetadata {
            cache_namespace: 7,
            cache_key_hi: 0x1122_3344_5566_7788,
            cache_key_lo: 0x99aa_bbcc_ddee_ff00,
            profile_id: 3,
            reuse_scope: CacheReuseScope::Session,
            lease_id: 5,
            producer_trace_id: 6,
            expiration_hint_ms: 700,
            metadata_bytes: 1,
            flags: nnrp_core::CACHE_REFERENCE_FLAGS_KNOWN_MASK,
        }
        .to_vec_with_extension(&[5])
        .expect("cache reference should encode");
        assert_valid(MessageType::CacheReference, &cache_reference);

        let cache_miss = CacheMissMetadata {
            cache_namespace: 7,
            cache_key_hi: 0x1122_3344_5566_7788,
            cache_key_lo: 0x99aa_bbcc_ddee_ff00,
            miss_reason: CacheMissReason::SchemaMismatch,
            profile_id: 3,
            diagnostic_bytes: 1,
        }
        .to_vec_with_diagnostics(&[6])
        .expect("cache miss should encode");
        assert_valid(MessageType::CacheMiss, &cache_miss);

        let cache_invalidate = CacheInvalidateMetadata {
            invalidate_scope: nnrp_core::CacheInvalidateScope::ObjectKey,
            cache_namespace: 7,
            cache_key_hi: 8,
            cache_key_lo: 9,
            reason_code: 10,
        }
        .to_bytes()
        .expect("cache invalidate should encode");
        assert_valid(MessageType::CacheInvalidate, &cache_invalidate);

        let unsupported = validate_runtime_frame_payload(
            MessageType::FrameSubmit as u32,
            NnrpBufferView::empty(),
        )
        .expect_err("data-plane frame must not use runtime frame send");
        assert_eq!(
            unsupported.status_code,
            NnrpFfiStatusCode::ProtocolError as u32
        );
        assert_eq!(
            validate_runtime_frame_payload(256, NnrpBufferView::empty()),
            Err(NnrpFfiStatus::invalid_argument(140))
        );
    }

    #[test]
    fn ffi_client_submit_control_polling_covers_scope_and_limit_edges() {
        unsafe {
            let mut invalid_result = empty_poll_result();
            assert_eq!(
                poll_matching_control_event(
                    NnrpHandle {
                        id: 1,
                        generation: 1,
                        kind: 99,
                        flags: 0,
                    },
                    NnrpEventKind::Control,
                    0,
                    &mut invalid_result
                ),
                NnrpFfiStatus::invalid_handle(99)
            );
            assert_eq!(invalid_result.status, NnrpFfiStatus::invalid_handle(99));

            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_531,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 91_532,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let mut empty_connection_result = empty_poll_result();
            assert_eq!(
                poll_matching_control_event(
                    connection,
                    NnrpEventKind::Control,
                    0,
                    &mut empty_connection_result
                )
                .status_code,
                NnrpFfiStatusCode::WouldBlock as u32
            );
            assert_eq!(empty_connection_result.has_event, 0);

            let flow_update = flow_update_payload();
            assert_eq!(
                nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                    handle: session,
                    message_type: MessageType::FlowUpdate as u32,
                    frame_id: 7,
                    payload: NnrpBufferView {
                        ptr: flow_update.as_ptr(),
                        len: flow_update.len(),
                    },
                }),
                NnrpFfiStatus::ok()
            );
            let mut limited_result = empty_poll_result();
            assert_eq!(
                poll_matching_control_event(
                    session,
                    NnrpEventKind::Control,
                    1,
                    &mut limited_result
                )
                .status_code,
                NnrpFfiStatusCode::WouldBlock as u32
            );
            assert_eq!(limited_result.has_event, 0);

            let mut operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_submit(
                    NnrpSubmitRequest {
                        session,
                        operation_id: 91_533,
                        frame_id: 8,
                        payload: NnrpBufferView::empty(),
                    },
                    &mut operation
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let progress = ProgressMetadata {
                operation_id: 91_533,
                progress_sequence: 2,
                stage_code: 3,
                percent_x100: 5_000,
                object_id: 0,
                body_bytes: 0,
            }
            .to_bytes()
            .expect("operation-scoped progress should encode");
            let mut operation_result = empty_poll_result();
            assert_eq!(
                test_client_submit_control(
                    NnrpClientSubmitControlRequest {
                        control: NnrpControlRequest {
                            handle: operation,
                            control_code: MessageType::Progress as u32,
                            payload: NnrpBufferView {
                                ptr: progress.as_ptr(),
                                len: progress.len(),
                            },
                        },
                        max_events: 0,
                    },
                    &mut operation_result
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(operation_result.has_event, 1);
            assert_eq!(operation_result.event.operation, operation);
        }
    }

    #[test]
    fn ffi_client_abi_batch_poll_rejects_invalid_buffers() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_201,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );
            let mut events = [NnrpEvent::none(); 1];
            let mut event_count = 99usize;

            assert_eq!(
                nnrp_client_await_events(
                    connection,
                    events.as_mut_ptr(),
                    events.len(),
                    core::ptr::null_mut()
                ),
                NnrpFfiStatus::invalid_argument(31)
            );
            assert_eq!(
                nnrp_client_await_events(
                    connection,
                    core::ptr::null_mut(),
                    events.len(),
                    &mut event_count
                ),
                NnrpFfiStatus::invalid_argument(32)
            );
            assert_eq!(event_count, 0);
            event_count = 99;
            assert_eq!(
                nnrp_client_await_events(connection, events.as_mut_ptr(), 0, &mut event_count),
                NnrpFfiStatus::invalid_argument(32)
            );
            assert_eq!(event_count, 0);
        }
    }

    #[test]
    fn ffi_client_abi_batch_poll_reports_full_capacity_and_invalid_handles() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_301,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );

            let mut events = [NnrpEvent::none(); 1];
            let mut event_count = 0usize;
            assert_eq!(
                nnrp_client_await_events(
                    connection,
                    events.as_mut_ptr(),
                    events.len(),
                    &mut event_count
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(event_count, 1);
            assert_eq!(events[0].kind, NnrpEventKind::ConnectionOpened as u32);

            assert_eq!(
                nnrp_client_await_events(
                    NnrpHandle::invalid(),
                    events.as_mut_ptr(),
                    events.len(),
                    &mut event_count
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
        }
    }

    #[test]
    fn ffi_schema_descriptor_helpers_round_trip_frozen_layouts() {
        unsafe {
            let schema_bytes =
                hex_to_bytes("011000000300000002000f000101000040000000020002008877665544332211");
            let mut schema = NnrpSchemaDescriptorHeader {
                schema_id: 0,
                schema_version: 0,
                profile_id: 0,
                schema_flags: 0,
                min_version_major: 0,
                max_version_major: 0,
                reserved0: 0,
                body_bytes: 0,
                dependency_count: 0,
                default_stream_semantics: 0,
                schema_hash: 0,
            };
            assert_eq!(
                nnrp_schema_descriptor_parse(
                    NnrpBufferView {
                        ptr: schema_bytes.as_ptr(),
                        len: schema_bytes.len(),
                    },
                    &mut schema,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(schema.schema_id, 0x0000_1001);
            assert_eq!(schema.schema_version, 3);
            assert_eq!(schema.profile_id, 2);
            assert_eq!(schema.reserved0, 0);
            assert_eq!(schema.schema_hash, 0x1122_3344_5566_7788);

            let mut round_trip = [0u8; nnrp_core::SCHEMA_DESCRIPTOR_HEADER_LEN];
            assert_eq!(
                nnrp_schema_descriptor_write(
                    schema,
                    NnrpBufferViewMut {
                        ptr: round_trip.as_mut_ptr(),
                        len: round_trip.len(),
                    }
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(round_trip.as_slice(), schema_bytes.as_slice());

            let typed_bytes = hex_to_bytes("020002000110000003000000020000000000000018000000");
            let mut typed = NnrpTypedPayloadDescriptor {
                profile_id: 0,
                descriptor_flags: 0,
                schema_id: 0,
                schema_version: 0,
                stream_semantics: 0,
                reserved0: 0,
                offset: 0,
                length: 0,
            };
            assert_eq!(
                nnrp_typed_payload_descriptor_parse(
                    NnrpBufferView {
                        ptr: typed_bytes.as_ptr(),
                        len: typed_bytes.len(),
                    },
                    &mut typed,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(typed.profile_id, 2);
            assert_eq!(typed.descriptor_flags, 2);
            assert_eq!(typed.schema_id, 0x0000_1001);
            assert_eq!(typed.reserved0, 0);

            let mut typed_round_trip = [0u8; nnrp_core::TYPED_PAYLOAD_DESCRIPTOR_LEN];
            assert_eq!(
                nnrp_typed_payload_descriptor_write(
                    typed,
                    NnrpBufferViewMut {
                        ptr: typed_round_trip.as_mut_ptr(),
                        len: typed_round_trip.len(),
                    }
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(typed_round_trip.as_slice(), typed_bytes.as_slice());
        }
    }

    #[test]
    fn ffi_schema_helpers_expose_standard_token_descriptor_and_binding_validation() {
        unsafe {
            let mut schema = NnrpSchemaDescriptorHeader {
                schema_id: 0,
                schema_version: 0,
                profile_id: 0,
                schema_flags: 0,
                min_version_major: 0,
                max_version_major: 0,
                reserved0: 0,
                body_bytes: 0,
                dependency_count: 0,
                default_stream_semantics: 0,
                schema_hash: 0,
            };
            assert_eq!(
                nnrp_token_delta_schema_descriptor(&mut schema),
                NnrpFfiStatus::ok()
            );
            assert_eq!(schema.schema_id, nnrp_core::TOKEN_DELTA_SCHEMA_ID);
            assert_eq!(schema.schema_version, nnrp_core::TOKEN_DELTA_SCHEMA_VERSION);
            assert_eq!(schema.profile_id, nnrp_core::PROFILE_TOKEN);

            let descriptor = NnrpTypedPayloadDescriptor {
                profile_id: nnrp_core::PROFILE_TOKEN,
                descriptor_flags: 0,
                schema_id: nnrp_core::TOKEN_DELTA_SCHEMA_ID,
                schema_version: nnrp_core::TOKEN_DELTA_SCHEMA_VERSION,
                stream_semantics: nnrp_core::STREAM_SEMANTICS_TOKEN_DELTA,
                reserved0: 0,
                offset: 0,
                length: 128,
            };
            assert_eq!(
                nnrp_typed_payload_validate_binding(&schema, 1, descriptor),
                NnrpFfiStatus::ok()
            );

            let incompatible = NnrpTypedPayloadDescriptor {
                profile_id: nnrp_core::PROFILE_TENSOR,
                ..descriptor
            };
            assert_eq!(
                nnrp_typed_payload_validate_binding(&schema, 1, incompatible),
                schema_registry_failure_status(SchemaRegistryFailure::Incompatible)
            );

            assert_eq!(
                nnrp_typed_payload_validate_binding(core::ptr::null(), 1, descriptor),
                NnrpFfiStatus::invalid_argument(36)
            );
            assert_eq!(
                nnrp_token_delta_schema_descriptor(core::ptr::null_mut()),
                NnrpFfiStatus::invalid_argument(34)
            );
        }
    }

    #[test]
    fn ffi_schema_helpers_reject_invalid_buffers_and_reserved_bits() {
        unsafe {
            let mut schema = NnrpSchemaDescriptorHeader {
                schema_id: 0,
                schema_version: 0,
                profile_id: 0,
                schema_flags: 0,
                min_version_major: 0,
                max_version_major: 0,
                reserved0: 0,
                body_bytes: 0,
                dependency_count: 0,
                default_stream_semantics: 0,
                schema_hash: 0,
            };
            let mut typed = NnrpTypedPayloadDescriptor {
                profile_id: 0,
                descriptor_flags: 0,
                schema_id: 0,
                schema_version: 0,
                stream_semantics: 0,
                reserved0: 0,
                offset: 0,
                length: 0,
            };
            assert_eq!(
                nnrp_schema_descriptor_parse(NnrpBufferView::empty(), &mut schema),
                NnrpFfiStatus::invalid_argument(0)
            );
            assert_eq!(
                nnrp_schema_descriptor_parse(
                    NnrpBufferView {
                        ptr: core::ptr::null(),
                        len: 1,
                    },
                    &mut schema
                ),
                NnrpFfiStatus::invalid_argument(1)
            );
            assert_eq!(
                nnrp_schema_descriptor_parse(NnrpBufferView::empty(), core::ptr::null_mut()),
                NnrpFfiStatus::invalid_argument(33)
            );

            schema.schema_flags = nnrp_core::SCHEMA_FLAGS_KNOWN_MASK + 1;
            let mut schema_out = [0u8; nnrp_core::SCHEMA_DESCRIPTOR_HEADER_LEN];
            assert_eq!(
                nnrp_schema_descriptor_write(
                    schema,
                    NnrpBufferViewMut {
                        ptr: schema_out.as_mut_ptr(),
                        len: schema_out.len(),
                    }
                ),
                NnrpFfiStatus {
                    status_code: NnrpFfiStatusCode::ProtocolError as u32,
                    error_family: NnrpErrorFamily::Transport as u32,
                    protocol_error_code: 0,
                    detail_code: 0,
                }
            );
            schema.schema_flags = 0;
            schema.reserved0 = 1;
            assert_eq!(
                nnrp_schema_descriptor_write(
                    schema,
                    NnrpBufferViewMut {
                        ptr: schema_out.as_mut_ptr(),
                        len: schema_out.len(),
                    }
                ),
                NnrpFfiStatus {
                    status_code: NnrpFfiStatusCode::ProtocolError as u32,
                    error_family: NnrpErrorFamily::Transport as u32,
                    protocol_error_code: 0,
                    detail_code: 0,
                }
            );

            assert_eq!(
                nnrp_typed_payload_descriptor_parse(NnrpBufferView::empty(), &mut typed),
                NnrpFfiStatus::invalid_argument(0)
            );
            assert_eq!(
                nnrp_typed_payload_descriptor_parse(NnrpBufferView::empty(), core::ptr::null_mut()),
                NnrpFfiStatus::invalid_argument(35)
            );
            typed.descriptor_flags = nnrp_core::DESCRIPTOR_FLAGS_KNOWN_MASK + 1;
            let mut typed_out = [0u8; nnrp_core::TYPED_PAYLOAD_DESCRIPTOR_LEN];
            assert_eq!(
                nnrp_typed_payload_descriptor_write(
                    typed,
                    NnrpBufferViewMut {
                        ptr: typed_out.as_mut_ptr(),
                        len: typed_out.len(),
                    }
                ),
                NnrpFfiStatus {
                    status_code: NnrpFfiStatusCode::ProtocolError as u32,
                    error_family: NnrpErrorFamily::Transport as u32,
                    protocol_error_code: 0,
                    detail_code: 0,
                }
            );
            typed.descriptor_flags = 0;
            typed.reserved0 = 1;
            assert_eq!(
                nnrp_typed_payload_descriptor_write(
                    typed,
                    NnrpBufferViewMut {
                        ptr: typed_out.as_mut_ptr(),
                        len: typed_out.len(),
                    }
                ),
                NnrpFfiStatus {
                    status_code: NnrpFfiStatusCode::ProtocolError as u32,
                    error_family: NnrpErrorFamily::Transport as u32,
                    protocol_error_code: 0,
                    detail_code: 0,
                }
            );

            typed.reserved0 = 0;
            assert_eq!(
                nnrp_typed_payload_descriptor_write(
                    typed,
                    NnrpBufferViewMut {
                        ptr: core::ptr::null_mut(),
                        len: 0,
                    }
                ),
                NnrpFfiStatus::invalid_argument(0)
            );

            let unspecified = NnrpTypedPayloadDescriptor {
                profile_id: nnrp_core::PROFILE_UNSPECIFIED,
                descriptor_flags: 0,
                schema_id: 0,
                schema_version: 0,
                stream_semantics: 0,
                reserved0: 0,
                offset: 0,
                length: 0,
            };
            assert_eq!(
                nnrp_typed_payload_validate_binding(core::ptr::null(), 0, unspecified),
                NnrpFfiStatus::ok()
            );
        }
    }

    #[test]
    fn ffi_schema_registry_handles_install_lookup_validate_and_release() {
        unsafe {
            let mut registry = NnrpHandle::invalid();
            assert_eq!(
                nnrp_schema_registry_create(&mut registry),
                NnrpFfiStatus::ok()
            );
            assert_eq!(registry.kind, NnrpHandleKind::SchemaRegistry as u32);

            let mut token_schema = NnrpSchemaDescriptorHeader {
                schema_id: 0,
                schema_version: 0,
                profile_id: 0,
                schema_flags: 0,
                min_version_major: 0,
                max_version_major: 0,
                reserved0: 0,
                body_bytes: 0,
                dependency_count: 0,
                default_stream_semantics: 0,
                schema_hash: 0,
            };
            assert_eq!(
                nnrp_token_delta_schema_descriptor(&mut token_schema),
                NnrpFfiStatus::ok()
            );

            let mut action = u32::MAX;
            assert_eq!(
                nnrp_schema_registry_install(registry, token_schema, &mut action),
                NnrpFfiStatus::ok()
            );
            assert_eq!(action, NNRP_SCHEMA_REGISTRY_ACTION_ALREADY_INSTALLED);

            let mut looked_up = NnrpSchemaDescriptorHeader {
                schema_id: 0,
                schema_version: 0,
                profile_id: 0,
                schema_flags: 0,
                min_version_major: 0,
                max_version_major: 0,
                reserved0: 0,
                body_bytes: 0,
                dependency_count: 0,
                default_stream_semantics: 0,
                schema_hash: 0,
            };
            assert_eq!(
                nnrp_schema_registry_lookup(
                    registry,
                    token_schema.schema_id,
                    token_schema.schema_version,
                    &mut looked_up,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(looked_up.schema_hash, token_schema.schema_hash);

            let descriptor = NnrpTypedPayloadDescriptor {
                profile_id: nnrp_core::PROFILE_TOKEN,
                descriptor_flags: 0,
                schema_id: token_schema.schema_id,
                schema_version: token_schema.schema_version,
                stream_semantics: nnrp_core::STREAM_SEMANTICS_TOKEN_DELTA,
                reserved0: 0,
                offset: 0,
                length: 16,
            };
            assert_eq!(
                nnrp_schema_registry_validate_binding(registry, descriptor),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_schema_registry_invalidate(
                    registry,
                    token_schema.schema_id,
                    token_schema.schema_version,
                    &mut action,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(action, NNRP_SCHEMA_REGISTRY_ACTION_INVALIDATED);
            assert_eq!(
                nnrp_schema_registry_validate_binding(registry, descriptor),
                schema_registry_failure_status(SchemaRegistryFailure::Unknown)
            );
            assert_eq!(nnrp_schema_registry_release(registry), NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_schema_registry_lookup(
                    registry,
                    token_schema.schema_id,
                    token_schema.schema_version,
                    &mut looked_up,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::SchemaRegistry as u32)
            );
        }
    }

    #[test]
    fn ffi_schema_registry_handles_cover_update_and_error_paths() {
        unsafe {
            let mut registry = NnrpHandle::invalid();
            assert_eq!(
                nnrp_schema_registry_create(core::ptr::null_mut()),
                NnrpFfiStatus::invalid_argument(40)
            );
            assert_eq!(
                nnrp_schema_registry_create(&mut registry),
                NnrpFfiStatus::ok()
            );

            let schema_v1 = NnrpSchemaDescriptorHeader {
                schema_id: 0x50,
                schema_version: 1,
                profile_id: nnrp_core::PROFILE_TENSOR,
                schema_flags: 0,
                min_version_major: 1,
                max_version_major: 1,
                reserved0: 0,
                body_bytes: 64,
                dependency_count: 0,
                default_stream_semantics: 0,
                schema_hash: 0x1111,
            };
            let mut action = u32::MAX;
            assert_eq!(
                nnrp_schema_registry_install(registry, schema_v1, core::ptr::null_mut()),
                NnrpFfiStatus::invalid_argument(41)
            );
            assert_eq!(
                nnrp_schema_registry_install(registry, schema_v1, &mut action),
                NnrpFfiStatus::ok()
            );
            assert_eq!(action, NNRP_SCHEMA_REGISTRY_ACTION_INSTALLED);

            let schema_v2 = NnrpSchemaDescriptorHeader {
                schema_version: 2,
                schema_hash: 0x2222,
                ..schema_v1
            };
            assert_eq!(
                nnrp_schema_registry_install(registry, schema_v2, &mut action),
                NnrpFfiStatus::ok()
            );
            assert_eq!(action, NNRP_SCHEMA_REGISTRY_ACTION_UPDATED);

            let conflict = NnrpSchemaDescriptorHeader {
                schema_hash: 0x3333,
                ..schema_v2
            };
            assert_eq!(
                nnrp_schema_registry_install(registry, conflict, &mut action),
                schema_registry_failure_status(SchemaRegistryFailure::HashConflict)
            );

            let reserved = NnrpSchemaDescriptorHeader {
                reserved0: 1,
                ..schema_v1
            };
            assert_eq!(
                nnrp_schema_registry_install(registry, reserved, &mut action).status_code,
                NnrpFfiStatusCode::ProtocolError as u32
            );

            let mut looked_up = schema_v1;
            assert_eq!(
                nnrp_schema_registry_lookup(registry, schema_v1.schema_id, 99, &mut looked_up),
                schema_registry_failure_status(SchemaRegistryFailure::VersionUnknown)
            );
            assert_eq!(
                nnrp_schema_registry_lookup(
                    registry,
                    schema_v1.schema_id,
                    schema_v1.schema_version,
                    core::ptr::null_mut(),
                ),
                NnrpFfiStatus::invalid_argument(42)
            );
            assert_eq!(
                nnrp_schema_registry_invalidate(registry, schema_v1.schema_id, 99, &mut action,),
                schema_registry_failure_status(SchemaRegistryFailure::VersionUnknown)
            );
            assert_eq!(
                nnrp_schema_registry_invalidate(
                    registry,
                    schema_v1.schema_id,
                    schema_v1.schema_version,
                    core::ptr::null_mut(),
                ),
                NnrpFfiStatus::invalid_argument(43)
            );

            let bad_descriptor = NnrpTypedPayloadDescriptor {
                profile_id: nnrp_core::PROFILE_TENSOR,
                descriptor_flags: 0,
                schema_id: schema_v1.schema_id,
                schema_version: schema_v1.schema_version,
                stream_semantics: 0,
                reserved0: 1,
                offset: 0,
                length: 8,
            };
            assert_eq!(
                nnrp_schema_registry_validate_binding(registry, bad_descriptor).status_code,
                NnrpFfiStatusCode::ProtocolError as u32
            );
            assert_eq!(
                nnrp_schema_registry_release(NnrpHandle::invalid()),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::SchemaRegistry as u32)
            );
        }
    }

    #[test]
    fn ffi_buffer_handles_keep_stable_copied_views_until_release() {
        unsafe {
            let source = [1u8, 2, 3, 4];
            let mut buffer = NnrpHandle::invalid();
            let mut view = NnrpBufferView::empty();
            assert_eq!(
                nnrp_buffer_acquire_copy(
                    NnrpBufferView {
                        ptr: source.as_ptr(),
                        len: source.len(),
                    },
                    &mut buffer,
                    &mut view,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(buffer.kind, NnrpHandleKind::Buffer as u32);
            assert_eq!(ffi_read_slice(view), source);

            let mut second_view = NnrpBufferView::empty();
            assert_eq!(
                nnrp_buffer_view(buffer, &mut second_view),
                NnrpFfiStatus::ok()
            );
            assert_eq!(ffi_read_slice(second_view), source);
            assert_eq!(nnrp_buffer_release(buffer), NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_buffer_view(buffer, &mut second_view),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)
            );
        }
    }

    #[test]
    fn ffi_object_metadata_buffer_aliases_use_owned_buffer_lifecycle() {
        unsafe {
            let metadata = br#"{"object":"tile-cache","version":2}"#;
            let mut buffer = NnrpHandle::invalid();
            let mut view = NnrpBufferView::empty();
            assert_eq!(
                nnrp_object_metadata_buffer_acquire_copy(
                    NnrpBufferView {
                        ptr: metadata.as_ptr(),
                        len: metadata.len(),
                    },
                    &mut buffer,
                    &mut view,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(buffer.kind, NnrpHandleKind::Buffer as u32);
            assert_eq!(ffi_read_slice(view), metadata);

            let mut object_view = NnrpBufferView::empty();
            assert_eq!(
                nnrp_object_metadata_buffer_view(buffer, &mut object_view),
                NnrpFfiStatus::ok()
            );
            assert_eq!(ffi_read_slice(object_view), metadata);
            assert_eq!(
                nnrp_object_metadata_buffer_release(buffer),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_buffer_view(buffer, &mut object_view),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)
            );
            assert_eq!(
                nnrp_object_metadata_buffer_release(buffer),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)
            );
        }
    }

    #[test]
    fn ffi_object_and_cache_descriptor_handles_preserve_metadata_until_release() {
        unsafe {
            let object_metadata = br#"{"object":"render-target"}"#;
            let object_descriptor = NnrpRuntimeObjectDescriptor {
                object_id: 77,
                object_kind: RuntimeObjectKind::Tensor as u16,
                producer_role: RuntimeRole::Server as u8,
                consumer_role: RuntimeRole::Client as u8,
                session_id: 9,
                byte_size: 4096,
                compute_cost_units: 3,
                memory_location_hint: MemoryLocationHint::DeviceMemory as u16,
                ownership_hint: OwnershipHint::ProducerOwned as u16,
                lifetime_hint_ms: 250,
                metadata_bytes: object_metadata.len() as u32,
            };
            let mut object_handle = NnrpHandle::invalid();
            assert_eq!(
                nnrp_object_descriptor_create(
                    object_descriptor,
                    NnrpBufferView {
                        ptr: object_metadata.as_ptr(),
                        len: object_metadata.len(),
                    },
                    &mut object_handle,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(object_handle.kind, NnrpHandleKind::ObjectDescriptor as u32);

            let mut observed_object = NnrpRuntimeObjectDescriptor {
                metadata_bytes: 0,
                ..object_descriptor
            };
            let mut observed_metadata = NnrpBufferView::empty();
            assert_eq!(
                nnrp_object_descriptor_view(
                    object_handle,
                    &mut observed_object,
                    &mut observed_metadata,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(observed_object, object_descriptor);
            assert_eq!(ffi_read_slice(observed_metadata), object_metadata);
            assert_eq!(
                nnrp_object_descriptor_release(object_handle),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_object_descriptor_view(
                    object_handle,
                    &mut observed_object,
                    &mut observed_metadata,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::ObjectDescriptor as u32)
            );

            let cache_metadata = br#"{"reuse":"same-scene"}"#;
            let cache_descriptor = NnrpCacheReferenceDescriptor {
                cache_namespace: 7,
                cache_key_hi: 1,
                cache_key_lo: 2,
                profile_id: 0x1001,
                reuse_scope: CacheReuseScope::Session as u16,
                lease_id: 44,
                producer_trace_id: 55,
                expiration_hint_ms: 1_000,
                metadata_bytes: cache_metadata.len() as u32,
                flags: 0,
            };
            let mut cache_handle = NnrpHandle::invalid();
            assert_eq!(
                nnrp_cache_reference_descriptor_create(
                    cache_descriptor,
                    NnrpBufferView {
                        ptr: cache_metadata.as_ptr(),
                        len: cache_metadata.len(),
                    },
                    &mut cache_handle,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                cache_handle.kind,
                NnrpHandleKind::CacheReferenceDescriptor as u32
            );

            let mut observed_cache = NnrpCacheReferenceDescriptor {
                metadata_bytes: 0,
                ..cache_descriptor
            };
            let mut observed_cache_metadata = NnrpBufferView::empty();
            assert_eq!(
                nnrp_cache_reference_descriptor_view(
                    cache_handle,
                    &mut observed_cache,
                    &mut observed_cache_metadata,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(observed_cache, cache_descriptor);
            assert_eq!(ffi_read_slice(observed_cache_metadata), cache_metadata);
            assert_eq!(
                nnrp_cache_reference_descriptor_release(cache_handle),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_cache_reference_descriptor_release(cache_handle),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::CacheReferenceDescriptor as u32)
            );
        }
    }

    #[test]
    fn ffi_descriptor_handles_reject_invalid_metadata_shapes() {
        unsafe {
            let metadata = [1u8, 2, 3];
            let object_descriptor = NnrpRuntimeObjectDescriptor {
                object_id: 88,
                object_kind: RuntimeObjectKind::Tensor as u16,
                producer_role: RuntimeRole::Server as u8,
                consumer_role: RuntimeRole::Client as u8,
                session_id: 1,
                byte_size: 4,
                compute_cost_units: 1,
                memory_location_hint: MemoryLocationHint::HostMemory as u16,
                ownership_hint: OwnershipHint::ProducerOwned as u16,
                lifetime_hint_ms: 10,
                metadata_bytes: 9,
            };
            let mut handle = NnrpHandle::invalid();
            assert_eq!(
                nnrp_object_descriptor_create(
                    object_descriptor,
                    NnrpBufferView {
                        ptr: metadata.as_ptr(),
                        len: metadata.len(),
                    },
                    &mut handle,
                ),
                NnrpFfiStatus::invalid_argument(51)
            );
            assert_eq!(
                nnrp_object_descriptor_create(
                    NnrpRuntimeObjectDescriptor {
                        metadata_bytes: 1,
                        ..object_descriptor
                    },
                    NnrpBufferView {
                        ptr: core::ptr::null(),
                        len: 1,
                    },
                    &mut handle,
                ),
                NnrpFfiStatus::invalid_argument(1)
            );

            let cache_descriptor = NnrpCacheReferenceDescriptor {
                cache_namespace: 7,
                cache_key_hi: 1,
                cache_key_lo: 2,
                profile_id: 0x1001,
                reuse_scope: CacheReuseScope::Session as u16,
                lease_id: 1,
                producer_trace_id: 2,
                expiration_hint_ms: 10,
                metadata_bytes: 9,
                flags: 0,
            };
            assert_eq!(
                nnrp_cache_reference_descriptor_create(
                    cache_descriptor,
                    NnrpBufferView {
                        ptr: metadata.as_ptr(),
                        len: metadata.len(),
                    },
                    &mut handle,
                ),
                NnrpFfiStatus::invalid_argument(54)
            );
            assert_eq!(
                nnrp_cache_reference_descriptor_view(
                    NnrpHandle::invalid(),
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                ),
                NnrpFfiStatus::invalid_argument(55)
            );
        }
    }

    #[test]
    fn ffi_descriptor_metadata_snapshots_outlive_descriptor_handles() {
        unsafe {
            let object_metadata = br#"{"object":"snapshot-target"}"#;
            let object_descriptor = NnrpRuntimeObjectDescriptor {
                object_id: 91,
                object_kind: RuntimeObjectKind::Tensor as u16,
                producer_role: RuntimeRole::Server as u8,
                consumer_role: RuntimeRole::Client as u8,
                session_id: 7,
                byte_size: 8192,
                compute_cost_units: 8,
                memory_location_hint: MemoryLocationHint::DeviceMemory as u16,
                ownership_hint: OwnershipHint::ProducerOwned as u16,
                lifetime_hint_ms: 333,
                metadata_bytes: object_metadata.len() as u32,
            };
            let mut object_handle = NnrpHandle::invalid();
            assert_eq!(
                nnrp_object_descriptor_create(
                    object_descriptor,
                    NnrpBufferView {
                        ptr: object_metadata.as_ptr(),
                        len: object_metadata.len(),
                    },
                    &mut object_handle,
                ),
                NnrpFfiStatus::ok()
            );

            let mut object_snapshot = NnrpHandle::invalid();
            let mut object_snapshot_view = NnrpBufferView::empty();
            assert_eq!(
                nnrp_object_descriptor_metadata_snapshot(
                    object_handle,
                    &mut object_snapshot,
                    &mut object_snapshot_view,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(object_snapshot.kind, NnrpHandleKind::Buffer as u32);
            assert_eq!(ffi_read_slice(object_snapshot_view), object_metadata);
            assert_eq!(
                nnrp_object_descriptor_release(object_handle),
                NnrpFfiStatus::ok()
            );

            let mut object_buffer_view = NnrpBufferView::empty();
            assert_eq!(
                nnrp_buffer_view(object_snapshot, &mut object_buffer_view),
                NnrpFfiStatus::ok()
            );
            assert_eq!(ffi_read_slice(object_buffer_view), object_metadata);
            assert_eq!(nnrp_buffer_release(object_snapshot), NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_buffer_view(object_snapshot, &mut object_buffer_view),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)
            );

            let cache_metadata = br#"{"cache":"snapshot-ref"}"#;
            let cache_descriptor = NnrpCacheReferenceDescriptor {
                cache_namespace: 7,
                cache_key_hi: 3,
                cache_key_lo: 4,
                profile_id: 0x1001,
                reuse_scope: CacheReuseScope::Operation as u16,
                lease_id: 66,
                producer_trace_id: 77,
                expiration_hint_ms: 2_000,
                metadata_bytes: cache_metadata.len() as u32,
                flags: 0,
            };
            let mut cache_handle = NnrpHandle::invalid();
            assert_eq!(
                nnrp_cache_reference_descriptor_create(
                    cache_descriptor,
                    NnrpBufferView {
                        ptr: cache_metadata.as_ptr(),
                        len: cache_metadata.len(),
                    },
                    &mut cache_handle,
                ),
                NnrpFfiStatus::ok()
            );

            let mut cache_snapshot = NnrpHandle::invalid();
            let mut cache_snapshot_view = NnrpBufferView::empty();
            assert_eq!(
                nnrp_cache_reference_descriptor_metadata_snapshot(
                    cache_handle,
                    &mut cache_snapshot,
                    &mut cache_snapshot_view,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(cache_snapshot.kind, NnrpHandleKind::Buffer as u32);
            assert_eq!(ffi_read_slice(cache_snapshot_view), cache_metadata);
            assert_eq!(
                nnrp_cache_reference_descriptor_release(cache_handle),
                NnrpFfiStatus::ok()
            );

            let mut cache_buffer_view = NnrpBufferView::empty();
            assert_eq!(
                nnrp_buffer_view(cache_snapshot, &mut cache_buffer_view),
                NnrpFfiStatus::ok()
            );
            assert_eq!(ffi_read_slice(cache_buffer_view), cache_metadata);
            assert_eq!(nnrp_buffer_release(cache_snapshot), NnrpFfiStatus::ok());
        }
    }

    #[test]
    fn ffi_descriptor_metadata_snapshots_reject_invalid_inputs() {
        unsafe {
            let mut snapshot = NnrpHandle::invalid();
            let mut snapshot_view = NnrpBufferView::empty();
            assert_eq!(
                nnrp_object_descriptor_metadata_snapshot(
                    NnrpHandle::invalid(),
                    &mut snapshot,
                    &mut snapshot_view,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::ObjectDescriptor as u32)
            );
            assert_eq!(
                nnrp_object_descriptor_metadata_snapshot(
                    NnrpHandle::invalid(),
                    core::ptr::null_mut(),
                    &mut snapshot_view,
                ),
                NnrpFfiStatus::invalid_argument(56)
            );
            assert_eq!(
                nnrp_cache_reference_descriptor_metadata_snapshot(
                    NnrpHandle::invalid(),
                    &mut snapshot,
                    &mut snapshot_view,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::CacheReferenceDescriptor as u32)
            );
            assert_eq!(
                nnrp_cache_reference_descriptor_metadata_snapshot(
                    NnrpHandle::invalid(),
                    &mut snapshot,
                    core::ptr::null_mut(),
                ),
                NnrpFfiStatus::invalid_argument(57)
            );
        }
    }

    #[test]
    fn ffi_buffer_handles_cover_invalid_inputs() {
        unsafe {
            let source = [5u8, 6];
            let mut buffer = NnrpHandle::invalid();
            let mut view = NnrpBufferView::empty();
            assert_eq!(
                nnrp_buffer_acquire_copy(
                    NnrpBufferView {
                        ptr: source.as_ptr(),
                        len: source.len(),
                    },
                    core::ptr::null_mut(),
                    &mut view,
                ),
                NnrpFfiStatus::invalid_argument(44)
            );
            assert_eq!(
                nnrp_buffer_acquire_copy(
                    NnrpBufferView {
                        ptr: core::ptr::null(),
                        len: 1,
                    },
                    &mut buffer,
                    &mut view,
                ),
                NnrpFfiStatus::invalid_argument(1)
            );
            assert_eq!(
                nnrp_buffer_acquire_copy(NnrpBufferView::empty(), &mut buffer, &mut view),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_buffer_view(buffer, core::ptr::null_mut()),
                NnrpFfiStatus::invalid_argument(45)
            );
            assert_eq!(
                nnrp_buffer_view(NnrpHandle::invalid(), &mut view),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)
            );
            assert_eq!(
                nnrp_buffer_release(NnrpHandle::invalid()),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)
            );
            assert_eq!(nnrp_buffer_release(buffer), NnrpFfiStatus::ok());
        }
    }

    #[test]
    fn ffi_cache_lease_handles_query_touch_prefetch_and_release() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 481_000,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection,
                ),
                NnrpFfiStatus::ok()
            );

            let object_id = NnrpCacheObjectId {
                cache_namespace: 7,
                cache_key_hi: 11,
                cache_key_lo: 13,
                object_kind: CacheObjectKind::ReusableResultObject as u32,
            };
            let request = NnrpCacheLeaseRequest {
                owner: connection,
                object_id,
                expected_version: 9,
                now_ms: 1_000,
                ttl_ms: 500,
            };
            let mut result = NnrpCacheLeaseResult {
                outcome_code: 0,
                lease_handle: NnrpHandle::invalid(),
                object_id,
                object_version: 0,
                lease_id: 0,
                expires_at_ms: 0,
            };
            assert_eq!(nnrp_cache_query(request, &mut result), NnrpFfiStatus::ok());
            assert_eq!(result.outcome_code, NNRP_CACHE_LEASE_OUTCOME_VALID);
            assert_eq!(result.lease_handle.kind, NnrpHandleKind::CacheLease as u32);
            assert_eq!(result.object_version, 9);
            assert_eq!(result.expires_at_ms, 31_000);

            let mut touched = result;
            assert_eq!(
                nnrp_cache_touch(
                    NnrpCacheLeaseRequest {
                        now_ms: 1_100,
                        ttl_ms: 900,
                        ..request
                    },
                    &mut touched,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(touched.lease_handle, result.lease_handle);
            assert_eq!(touched.expires_at_ms, 1_900);

            let mut mismatch = result;
            assert_eq!(
                nnrp_cache_query(
                    NnrpCacheLeaseRequest {
                        expected_version: 10,
                        ..request
                    },
                    &mut mismatch,
                ),
                cache_validation_failure_status(CacheValidationFailure::VersionMismatch)
            );
            assert_eq!(mismatch.object_version, 9);

            let objects = [
                NnrpCacheObjectId {
                    cache_namespace: 7,
                    cache_key_hi: 11,
                    cache_key_lo: 14,
                    object_kind: CacheObjectKind::PromptSegment as u32,
                },
                NnrpCacheObjectId {
                    cache_namespace: 7,
                    cache_key_hi: 11,
                    cache_key_lo: 15,
                    object_kind: CacheObjectKind::ToolSchema as u32,
                },
            ];
            let mut results = [result; 2];
            assert_eq!(
                nnrp_cache_prefetch(
                    connection,
                    objects.as_ptr(),
                    objects.len(),
                    2_000,
                    100,
                    results.as_mut_ptr(),
                ),
                NnrpFfiStatus::ok()
            );
            assert!(results
                .iter()
                .all(|item| item.outcome_code == NNRP_CACHE_LEASE_OUTCOME_VALID));
            assert_eq!(
                nnrp_cache_prefetch(
                    connection,
                    core::ptr::null(),
                    0,
                    2_000,
                    100,
                    core::ptr::null_mut(),
                ),
                NnrpFfiStatus::ok()
            );

            let mut released = result;
            assert_eq!(
                nnrp_cache_release(result.lease_handle, &mut released),
                NnrpFfiStatus::ok()
            );
            assert_eq!(released.outcome_code, NNRP_CACHE_LEASE_OUTCOME_RELEASED);
        }
    }

    #[test]
    fn ffi_cache_lease_handles_cover_invalid_expired_and_owned_cleanup_paths() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 481_010,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection,
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 481,
                        generation: 1,
                        profile_id: nnrp_core::PROFILE_TOKEN,
                        schema_id: nnrp_core::TOKEN_DELTA_SCHEMA_ID,
                        schema_version: nnrp_core::TOKEN_DELTA_SCHEMA_VERSION,
                    },
                    &mut session,
                ),
                NnrpFfiStatus::ok()
            );
            let payload = [1u8];
            let mut operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_submit(
                    NnrpSubmitRequest {
                        session,
                        operation_id: 481_011,
                        frame_id: 91,
                        payload: NnrpBufferView {
                            ptr: payload.as_ptr(),
                            len: payload.len(),
                        },
                    },
                    &mut operation,
                ),
                NnrpFfiStatus::ok()
            );

            let object_id = NnrpCacheObjectId {
                cache_namespace: 9,
                cache_key_hi: 10,
                cache_key_lo: 11,
                object_kind: CacheObjectKind::CameraBlock as u32,
            };
            let request = NnrpCacheLeaseRequest {
                owner: session,
                object_id,
                expected_version: 0,
                now_ms: 100,
                ttl_ms: 1,
            };
            let mut result = NnrpCacheLeaseResult {
                outcome_code: 0,
                lease_handle: NnrpHandle::invalid(),
                object_id,
                object_version: 0,
                lease_id: 0,
                expires_at_ms: 0,
            };
            assert_eq!(
                nnrp_cache_query(request, core::ptr::null_mut()),
                NnrpFfiStatus::invalid_argument(49)
            );
            assert_eq!(
                nnrp_cache_query(
                    NnrpCacheLeaseRequest {
                        object_id: NnrpCacheObjectId {
                            object_kind: u32::MAX,
                            ..object_id
                        },
                        ..request
                    },
                    &mut result,
                )
                .status_code,
                NnrpFfiStatusCode::ProtocolError as u32
            );
            assert_eq!(
                nnrp_cache_query(
                    NnrpCacheLeaseRequest {
                        owner: NnrpHandle::invalid(),
                        ..request
                    },
                    &mut result,
                ),
                cache_validation_failure_status(CacheValidationFailure::Miss)
            );
            assert_eq!(
                nnrp_cache_query(
                    NnrpCacheLeaseRequest {
                        owner: NnrpHandle::new(NnrpHandleKind::Buffer, 99, 1),
                        ..request
                    },
                    &mut result,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)
            );
            assert_eq!(
                nnrp_cache_touch(
                    NnrpCacheLeaseRequest {
                        ttl_ms: 0,
                        ..request
                    },
                    &mut result,
                ),
                NnrpFfiStatus::invalid_argument(46)
            );
            assert_eq!(nnrp_cache_query(request, &mut result), NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_cache_query(
                    NnrpCacheLeaseRequest {
                        now_ms: result.expires_at_ms.saturating_add(1),
                        ttl_ms: 0,
                        ..request
                    },
                    &mut result,
                ),
                cache_validation_failure_status(CacheValidationFailure::LeaseExpired)
            );
            assert_eq!(result.outcome_code, NNRP_CACHE_LEASE_OUTCOME_EXPIRED);
            assert_eq!(
                nnrp_cache_prefetch(session, core::ptr::null(), 1, 0, 1, core::ptr::null_mut(),),
                NnrpFfiStatus::invalid_argument(47)
            );

            let operation_request = NnrpCacheLeaseRequest {
                owner: operation,
                object_id: NnrpCacheObjectId {
                    cache_key_lo: 12,
                    ..object_id
                },
                expected_version: 0,
                now_ms: 200,
                ttl_ms: 1,
            };
            assert_eq!(
                nnrp_cache_query(operation_request, &mut result),
                NnrpFfiStatus::ok()
            );
            let operation_lease = result.lease_handle;
            assert_eq!(
                nnrp_cache_release(result.lease_handle, core::ptr::null_mut()),
                NnrpFfiStatus::invalid_argument(48)
            );
            assert_eq!(
                nnrp_cache_release(NnrpHandle::invalid(), &mut result),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::CacheLease as u32)
            );
            assert_eq!(nnrp_client_close(session), NnrpFfiStatus::ok());
            let store = handle_store();
            assert!(!store.entries.values().any(|entry| matches!(
                &entry.resource,
                NnrpFfiResource::Operation { session: owner, .. } if *owner == session
            )));
            assert!(!store.entries.values().any(|entry| matches!(
                &entry.resource,
                NnrpFfiResource::CacheLease { owner, .. }
                    if *owner == session || *owner == operation
            )));
            drop(store);
            assert_eq!(operation_lease.kind, NnrpHandleKind::CacheLease as u32);
            assert_eq!(
                nnrp_client_close_connection(connection),
                NnrpFfiStatus::ok()
            );
        }
    }

    #[test]
    fn ffi_client_resume_session_opens_session_with_recovery_outcome() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 481_100,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection,
                ),
                NnrpFfiStatus::ok()
            );

            let mut session = NnrpHandle::invalid();
            let mut outcome = NnrpSessionRecoveryOutcome {
                outcome_code: 0,
                resume_window_ms: 0,
            };
            assert_eq!(
                nnrp_client_resume_session(
                    NnrpSessionResumeRequest {
                        connection,
                        requested_session_id: 88,
                        generation: 1,
                        profile_id: nnrp_core::PROFILE_TOKEN,
                        schema_id: nnrp_core::TOKEN_DELTA_SCHEMA_ID,
                        schema_version: nnrp_core::TOKEN_DELTA_SCHEMA_VERSION,
                        resume_token_bytes: 16,
                    },
                    &mut session,
                    &mut outcome,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(session.kind, NnrpHandleKind::Session as u32);
            assert_eq!(session.id, 88);
            assert_eq!(outcome.outcome_code, NNRP_SESSION_RECOVERY_OUTCOME_RESUMED);

            assert_eq!(
                nnrp_client_resume_session(
                    NnrpSessionResumeRequest {
                        connection,
                        requested_session_id: 0,
                        generation: 1,
                        profile_id: nnrp_core::PROFILE_TOKEN,
                        schema_id: nnrp_core::TOKEN_DELTA_SCHEMA_ID,
                        schema_version: nnrp_core::TOKEN_DELTA_SCHEMA_VERSION,
                        resume_token_bytes: 16,
                    },
                    &mut session,
                    &mut outcome,
                ),
                NnrpFfiStatus::invalid_argument(39)
            );

            assert_eq!(
                nnrp_client_resume_session(
                    NnrpSessionResumeRequest {
                        connection,
                        requested_session_id: 90,
                        generation: 1,
                        profile_id: nnrp_core::PROFILE_TOKEN,
                        schema_id: nnrp_core::TOKEN_DELTA_SCHEMA_ID,
                        schema_version: nnrp_core::TOKEN_DELTA_SCHEMA_VERSION,
                        resume_token_bytes: 16,
                    },
                    core::ptr::null_mut(),
                    &mut outcome,
                ),
                NnrpFfiStatus::invalid_argument(39)
            );
            assert_eq!(
                nnrp_client_resume_session(
                    NnrpSessionResumeRequest {
                        connection: NnrpHandle::invalid(),
                        requested_session_id: 90,
                        generation: 1,
                        profile_id: nnrp_core::PROFILE_TOKEN,
                        schema_id: nnrp_core::TOKEN_DELTA_SCHEMA_ID,
                        schema_version: nnrp_core::TOKEN_DELTA_SCHEMA_VERSION,
                        resume_token_bytes: 16,
                    },
                    &mut session,
                    &mut outcome,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
        }
    }

    #[test]
    fn ffi_control_emits_distinct_result_hint_events() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 481_200,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection,
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 89,
                        generation: 1,
                        profile_id: nnrp_core::PROFILE_TOKEN,
                        schema_id: nnrp_core::TOKEN_DELTA_SCHEMA_ID,
                        schema_version: nnrp_core::TOKEN_DELTA_SCHEMA_VERSION,
                    },
                    &mut session,
                ),
                NnrpFfiStatus::ok()
            );
            let payload = [0u8; 1];
            let mut operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_submit(
                    NnrpSubmitRequest {
                        session,
                        operation_id: 481_201,
                        frame_id: 77,
                        payload: NnrpBufferView {
                            ptr: payload.as_ptr(),
                            len: payload.len(),
                        },
                    },
                    &mut operation,
                ),
                NnrpFfiStatus::ok()
            );

            let hint = ResultHintMetadata {
                applied_budget_policy: nnrp_core::ResultHintBudgetPolicy::Partial,
                congestion_state: nnrp_core::ResultHintCongestionState::Elevated,
                reason: nnrp_core::ResultHintReason::ServerBusy,
                retry_after_ms: 250,
            }
            .to_bytes()
            .unwrap();
            assert_eq!(
                test_control_event_injection(NnrpControlRequest {
                    handle: operation,
                    control_code: MessageType::ResultHint as u32,
                    payload: NnrpBufferView {
                        ptr: hint.as_ptr(),
                        len: hint.len(),
                    },
                }),
                NnrpFfiStatus::ok()
            );

            let mut events = [NnrpEvent::none(); 4];
            let mut event_count = 0usize;
            assert_eq!(
                nnrp_client_await_events(
                    connection,
                    events.as_mut_ptr(),
                    events.len(),
                    &mut event_count,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(event_count, 4);
            assert_eq!(events[3].kind, NnrpEventKind::ResultHint as u32);
            assert_eq!(events[3].session, session);
            assert_eq!(events[3].operation, operation);

            assert_eq!(
                test_control_event_injection(NnrpControlRequest {
                    handle: operation,
                    control_code: MessageType::ResultHint as u32,
                    payload: NnrpBufferView::empty(),
                })
                .status_code,
                NnrpFfiStatusCode::InvalidArgument as u32
            );

            assert_eq!(
                test_control_event_injection(NnrpControlRequest {
                    handle: connection,
                    control_code: MessageType::Ping as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                test_control_event_injection(NnrpControlRequest {
                    handle: session,
                    control_code: MessageType::Pong as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                test_control_event_injection(NnrpControlRequest {
                    handle: NnrpHandle::invalid(),
                    control_code: MessageType::Pong as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Invalid as u32)
            );
            assert_eq!(
                test_control_event_injection(NnrpControlRequest {
                    handle: operation,
                    control_code: MessageType::Pong as u32,
                    payload: NnrpBufferView {
                        ptr: core::ptr::null(),
                        len: 1,
                    },
                }),
                NnrpFfiStatus::invalid_argument(1)
            );
        }
    }

    #[test]
    fn ffi_native_gap_helpers_cover_internal_defensive_branches() {
        unsafe {
            let registry_handle = NnrpHandle::new(NnrpHandleKind::SchemaRegistry, 881_000, 1);
            let buffer_handle = NnrpHandle::new(NnrpHandleKind::Buffer, 881_001, 1);
            let lease_handle = NnrpHandle::new(NnrpHandleKind::CacheLease, 881_002, 1);
            let connection_handle = NnrpHandle::new(NnrpHandleKind::Connection, 881_003, 1);
            let session_handle = NnrpHandle::new(NnrpHandleKind::Session, 881_004, 1);
            let operation_handle = NnrpHandle::new(NnrpHandleKind::Operation, 881_005, 1);
            {
                let mut store = handle_store();
                store
                    .insert(
                        registry_handle,
                        NnrpFfiResource::Buffer {
                            bytes: vec![1, 2, 3],
                        },
                    )
                    .unwrap();
                store
                    .insert(
                        buffer_handle,
                        NnrpFfiResource::SchemaRegistry {
                            registry: SchemaRegistry::new(),
                        },
                    )
                    .unwrap();
                store
                    .insert(lease_handle, NnrpFfiResource::Buffer { bytes: vec![4, 5] })
                    .unwrap();
                store
                    .insert(connection_handle, NnrpFfiResource::Buffer { bytes: vec![] })
                    .unwrap();
                store
                    .insert(session_handle, NnrpFfiResource::Buffer { bytes: vec![] })
                    .unwrap();
                store
                    .insert(operation_handle, NnrpFfiResource::Buffer { bytes: vec![] })
                    .unwrap();
            }

            let schema = NnrpSchemaDescriptorHeader {
                schema_id: 0x60,
                schema_version: 1,
                profile_id: nnrp_core::PROFILE_TENSOR,
                schema_flags: 0,
                min_version_major: 1,
                max_version_major: 1,
                reserved0: 0,
                body_bytes: 32,
                dependency_count: 0,
                default_stream_semantics: 0,
                schema_hash: 0x6060,
            };
            let mut action = 0;
            let mut schema_out = schema;
            let descriptor = NnrpTypedPayloadDescriptor {
                profile_id: nnrp_core::PROFILE_TENSOR,
                descriptor_flags: 0,
                schema_id: schema.schema_id,
                schema_version: schema.schema_version,
                stream_semantics: 0,
                reserved0: 0,
                offset: 0,
                length: 8,
            };
            assert_eq!(
                nnrp_schema_registry_install(registry_handle, schema, &mut action),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::SchemaRegistry as u32)
            );
            assert_eq!(
                nnrp_schema_registry_lookup(
                    registry_handle,
                    schema.schema_id,
                    schema.schema_version,
                    &mut schema_out,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::SchemaRegistry as u32)
            );
            assert_eq!(
                nnrp_schema_registry_invalidate(
                    registry_handle,
                    schema.schema_id,
                    schema.schema_version,
                    &mut action,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::SchemaRegistry as u32)
            );
            assert_eq!(
                nnrp_schema_registry_validate_binding(registry_handle, descriptor),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::SchemaRegistry as u32)
            );

            let mut view = NnrpBufferView::empty();
            assert_eq!(
                nnrp_buffer_view(buffer_handle, &mut view),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)
            );
            assert_eq!(
                nnrp_buffer_release(buffer_handle),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)
            );

            let object_id = NnrpCacheObjectId {
                cache_namespace: 1,
                cache_key_hi: 2,
                cache_key_lo: 3,
                object_kind: CacheObjectKind::CameraBlock as u32,
            };
            let mut lease_result = NnrpCacheLeaseResult {
                outcome_code: 0,
                lease_handle: NnrpHandle::invalid(),
                object_id,
                object_version: 0,
                lease_id: 0,
                expires_at_ms: 0,
            };
            assert_eq!(
                nnrp_cache_release(lease_handle, &mut lease_result),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::CacheLease as u32)
            );
            assert_eq!(
                nnrp_cache_query(
                    NnrpCacheLeaseRequest {
                        owner: NnrpHandle {
                            generation: 2,
                            ..connection_handle
                        },
                        object_id,
                        expected_version: 0,
                        now_ms: 0,
                        ttl_ms: 1,
                    },
                    &mut lease_result,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
            let objects = [NnrpCacheObjectId {
                object_kind: u32::MAX,
                ..object_id
            }];
            let mut results = [lease_result];
            assert_eq!(
                nnrp_cache_prefetch(
                    connection_handle,
                    objects.as_ptr(),
                    objects.len(),
                    0,
                    1,
                    results.as_mut_ptr(),
                )
                .status_code,
                NnrpFfiStatusCode::ProtocolError as u32
            );

            assert_eq!(
                test_control_event_injection(NnrpControlRequest {
                    handle: connection_handle,
                    control_code: MessageType::Ping as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
            assert_eq!(
                test_control_event_injection(NnrpControlRequest {
                    handle: session_handle,
                    control_code: MessageType::Ping as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
            );
            assert_eq!(
                test_control_event_injection(NnrpControlRequest {
                    handle: operation_handle,
                    control_code: MessageType::Ping as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32)
            );
            assert_eq!(
                test_control_event_injection(NnrpControlRequest {
                    handle: NnrpHandle::new(NnrpHandleKind::CacheLease, 991, 1),
                    control_code: MessageType::Ping as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::CacheLease as u32)
            );
        }
    }

    #[test]
    fn ffi_recovery_helpers_validate_resume_and_migration_bytes() {
        unsafe {
            let request = recovery_session_open(42, 16, nnrp_core::SESSION_FLAG_ALLOW_RESUME);
            let request_bytes = request.to_bytes().unwrap();
            assert_eq!(
                nnrp_session_recovery_request_validate(NnrpBufferView {
                    ptr: request_bytes.as_ptr(),
                    len: request_bytes.len(),
                }),
                NnrpFfiStatus::ok()
            );

            let ack = recovery_session_ack(
                nnrp_core::SessionStatus::Resumed,
                nnrp_core::SESSION_ACK_FLAG_RESUME_ENABLED,
                10_000,
                24,
                nnrp_core::SESSION_ERROR_NONE,
            );
            let ack_bytes = ack.to_bytes().unwrap();
            let mut outcome = NnrpSessionRecoveryOutcome {
                outcome_code: 0,
                resume_window_ms: 0,
            };
            assert_eq!(
                nnrp_session_recovery_ack_validate(
                    NnrpBufferView {
                        ptr: request_bytes.as_ptr(),
                        len: request_bytes.len(),
                    },
                    NnrpBufferView {
                        ptr: ack_bytes.as_ptr(),
                        len: ack_bytes.len(),
                    },
                    &mut outcome,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                outcome,
                NnrpSessionRecoveryOutcome {
                    outcome_code: NNRP_SESSION_RECOVERY_OUTCOME_RESUMED,
                    resume_window_ms: 10_000,
                }
            );

            let rejected = recovery_session_ack(
                nnrp_core::SessionStatus::Rejected,
                0,
                0,
                0,
                nnrp_core::SESSION_ERROR_RESUME_REJECTED,
            );
            let rejected_bytes = rejected.to_bytes().unwrap();
            assert_eq!(
                nnrp_session_recovery_ack_validate(
                    NnrpBufferView {
                        ptr: request_bytes.as_ptr(),
                        len: request_bytes.len(),
                    },
                    NnrpBufferView {
                        ptr: rejected_bytes.as_ptr(),
                        len: rejected_bytes.len(),
                    },
                    &mut outcome,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                outcome.outcome_code,
                NNRP_SESSION_RECOVERY_OUTCOME_RESUME_REJECTED
            );

            let migrate = nnrp_core::SessionMigrateMetadata {
                old_transport_id: nnrp_core::TransportId::Tcp,
                new_transport_id: nnrp_core::TransportId::Quic,
                last_result_frame_id: 10,
                client_migrate_ts_us: 100,
            };
            let migrate_ack = nnrp_core::SessionMigrateAckMetadata {
                accept_code: 0,
                resume_from_frame_id: 12,
                grace_window_ms: 250,
                server_migrate_ts_us: 200,
            };
            let migrate_bytes = migrate.to_bytes().unwrap();
            let migrate_ack_bytes = migrate_ack.to_bytes().unwrap();
            assert_eq!(
                nnrp_migration_recovery_validate(
                    NnrpBufferView {
                        ptr: migrate_bytes.as_ptr(),
                        len: migrate_bytes.len(),
                    },
                    NnrpBufferView {
                        ptr: migrate_ack_bytes.as_ptr(),
                        len: migrate_ack_bytes.len(),
                    },
                ),
                NnrpFfiStatus::ok()
            );

            let mut should_replay = 0u8;
            assert_eq!(
                nnrp_migration_should_replay_frame(
                    NnrpBufferView {
                        ptr: migrate_ack_bytes.as_ptr(),
                        len: migrate_ack_bytes.len(),
                    },
                    11,
                    &mut should_replay,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(should_replay, 0);
            assert_eq!(
                nnrp_migration_should_replay_frame(
                    NnrpBufferView {
                        ptr: migrate_ack_bytes.as_ptr(),
                        len: migrate_ack_bytes.len(),
                    },
                    12,
                    &mut should_replay,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(should_replay, 1);
        }
    }

    #[test]
    fn ffi_recovery_helpers_reject_invalid_protocol_inputs() {
        unsafe {
            let missing_flag = recovery_session_open(42, 16, 0).to_bytes().unwrap();
            assert_eq!(
                nnrp_session_recovery_request_validate(NnrpBufferView {
                    ptr: missing_flag.as_ptr(),
                    len: missing_flag.len(),
                })
                .status_code,
                NnrpFfiStatusCode::ProtocolError as u32
            );

            let request = recovery_session_open(42, 16, nnrp_core::SESSION_FLAG_ALLOW_RESUME)
                .to_bytes()
                .unwrap();
            let bad_ack = recovery_session_ack(
                nnrp_core::SessionStatus::Resumed,
                0,
                0,
                0,
                nnrp_core::SESSION_ERROR_NONE,
            )
            .to_bytes()
            .unwrap();
            let mut outcome = NnrpSessionRecoveryOutcome {
                outcome_code: 0,
                resume_window_ms: 0,
            };
            assert_eq!(
                nnrp_session_recovery_ack_validate(
                    NnrpBufferView {
                        ptr: request.as_ptr(),
                        len: request.len(),
                    },
                    NnrpBufferView {
                        ptr: bad_ack.as_ptr(),
                        len: bad_ack.len(),
                    },
                    &mut outcome,
                )
                .status_code,
                NnrpFfiStatusCode::ProtocolError as u32
            );
            assert_eq!(
                nnrp_session_recovery_ack_validate(
                    NnrpBufferView {
                        ptr: request.as_ptr(),
                        len: request.len(),
                    },
                    NnrpBufferView {
                        ptr: bad_ack.as_ptr(),
                        len: bad_ack.len(),
                    },
                    core::ptr::null_mut(),
                ),
                NnrpFfiStatus::invalid_argument(37)
            );

            let migrate = nnrp_core::SessionMigrateMetadata {
                old_transport_id: nnrp_core::TransportId::Quic,
                new_transport_id: nnrp_core::TransportId::Quic,
                last_result_frame_id: 10,
                client_migrate_ts_us: 100,
            }
            .to_bytes()
            .unwrap();
            let migrate_ack = nnrp_core::SessionMigrateAckMetadata {
                accept_code: 0,
                resume_from_frame_id: 9,
                grace_window_ms: 250,
                server_migrate_ts_us: 200,
            }
            .to_bytes()
            .unwrap();
            assert_eq!(
                nnrp_migration_recovery_validate(
                    NnrpBufferView {
                        ptr: migrate.as_ptr(),
                        len: migrate.len(),
                    },
                    NnrpBufferView {
                        ptr: migrate_ack.as_ptr(),
                        len: migrate_ack.len(),
                    },
                )
                .status_code,
                NnrpFfiStatusCode::ProtocolError as u32
            );
            assert_eq!(
                nnrp_migration_should_replay_frame(
                    NnrpBufferView {
                        ptr: migrate_ack.as_ptr(),
                        len: migrate_ack.len(),
                    },
                    12,
                    core::ptr::null_mut(),
                ),
                NnrpFfiStatus::invalid_argument(38)
            );
        }
    }

    #[test]
    fn ffi_recovery_helpers_cover_fresh_enabled_and_bad_buffers() {
        unsafe {
            let fresh_request = recovery_session_open(0, 0, 0).to_bytes().unwrap();
            let fresh_ack = recovery_session_ack(
                nnrp_core::SessionStatus::Opened,
                0,
                0,
                0,
                nnrp_core::SESSION_ERROR_NONE,
            )
            .to_bytes()
            .unwrap();
            let mut outcome = NnrpSessionRecoveryOutcome {
                outcome_code: 99,
                resume_window_ms: 99,
            };
            assert_eq!(
                session_recovery_ack_validate_impl(
                    NnrpBufferView {
                        ptr: fresh_request.as_ptr(),
                        len: fresh_request.len(),
                    },
                    NnrpBufferView {
                        ptr: fresh_ack.as_ptr(),
                        len: fresh_ack.len(),
                    },
                    &mut outcome,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                outcome,
                NnrpSessionRecoveryOutcome {
                    outcome_code: NNRP_SESSION_RECOVERY_OUTCOME_FRESH,
                    resume_window_ms: 0,
                }
            );

            let enabled_request =
                recovery_session_open(42, 0, nnrp_core::SESSION_FLAG_ALLOW_RESUME)
                    .to_bytes()
                    .unwrap();
            let enabled_ack = recovery_session_ack(
                nnrp_core::SessionStatus::Opened,
                nnrp_core::SESSION_ACK_FLAG_RESUME_ENABLED,
                15_000,
                32,
                nnrp_core::SESSION_ERROR_NONE,
            )
            .to_bytes()
            .unwrap();
            assert_eq!(
                session_recovery_ack_validate_impl(
                    NnrpBufferView {
                        ptr: enabled_request.as_ptr(),
                        len: enabled_request.len(),
                    },
                    NnrpBufferView {
                        ptr: enabled_ack.as_ptr(),
                        len: enabled_ack.len(),
                    },
                    &mut outcome,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                outcome,
                NnrpSessionRecoveryOutcome {
                    outcome_code: NNRP_SESSION_RECOVERY_OUTCOME_RESUME_ENABLED,
                    resume_window_ms: 15_000,
                }
            );

            let null_non_empty = NnrpBufferView {
                ptr: core::ptr::null(),
                len: 1,
            };
            assert_eq!(
                session_recovery_request_validate_impl(null_non_empty),
                NnrpFfiStatus::invalid_argument(1)
            );
            assert_eq!(
                session_recovery_request_validate_impl(NnrpBufferView::empty()),
                NnrpFfiStatus::invalid_argument(0)
            );
            assert_eq!(
                session_recovery_ack_validate_impl(
                    null_non_empty,
                    NnrpBufferView {
                        ptr: fresh_ack.as_ptr(),
                        len: fresh_ack.len(),
                    },
                    &mut outcome,
                ),
                NnrpFfiStatus::invalid_argument(1)
            );
            assert_eq!(
                session_recovery_ack_validate_impl(
                    NnrpBufferView {
                        ptr: fresh_request.as_ptr(),
                        len: fresh_request.len(),
                    },
                    NnrpBufferView::empty(),
                    &mut outcome,
                ),
                NnrpFfiStatus::invalid_argument(0)
            );

            let migrate = nnrp_core::SessionMigrateMetadata {
                old_transport_id: nnrp_core::TransportId::Tcp,
                new_transport_id: nnrp_core::TransportId::Quic,
                last_result_frame_id: 10,
                client_migrate_ts_us: 100,
            }
            .to_bytes()
            .unwrap();
            let migrate_ack = nnrp_core::SessionMigrateAckMetadata {
                accept_code: 0,
                resume_from_frame_id: 10,
                grace_window_ms: 250,
                server_migrate_ts_us: 200,
            }
            .to_bytes()
            .unwrap();
            assert_eq!(
                migration_recovery_validate_impl(
                    null_non_empty,
                    NnrpBufferView {
                        ptr: migrate_ack.as_ptr(),
                        len: migrate_ack.len(),
                    },
                ),
                NnrpFfiStatus::invalid_argument(1)
            );
            assert_eq!(
                migration_recovery_validate_impl(
                    NnrpBufferView {
                        ptr: migrate.as_ptr(),
                        len: migrate.len(),
                    },
                    NnrpBufferView::empty(),
                ),
                NnrpFfiStatus::invalid_argument(0)
            );
            let mut should_replay = 0u8;
            assert_eq!(
                migration_should_replay_frame_impl(null_non_empty, 10, &mut should_replay),
                NnrpFfiStatus::invalid_argument(1)
            );
            assert_eq!(
                migration_should_replay_frame_impl(NnrpBufferView::empty(), 10, &mut should_replay,),
                NnrpFfiStatus::invalid_argument(0)
            );
        }
    }

    #[test]
    fn ffi_connection_close_cascades_owned_client_handles() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 95_101,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection,
                ),
                NnrpFfiStatus::ok()
            );

            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 95_102,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session,
                ),
                NnrpFfiStatus::ok()
            );

            let payload = [1u8, 2, 3];
            let mut operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_submit(
                    NnrpSubmitRequest {
                        session,
                        operation_id: 95_103,
                        frame_id: 7,
                        payload: NnrpBufferView {
                            ptr: payload.as_ptr(),
                            len: payload.len(),
                        },
                    },
                    &mut operation,
                ),
                NnrpFfiStatus::ok()
            );

            assert_eq!(
                nnrp_client_close_connection(connection),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_client_close_connection(connection),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
            assert_eq!(
                nnrp_client_close(session),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
            );
            assert_eq!(
                nnrp_client_cancel(NnrpClientCancelRequest {
                    session,
                    frame_id: 7,
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
            );
            let mut result = empty_poll_result();
            assert_eq!(
                nnrp_client_await_event(connection, &mut result),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
        }
    }

    #[test]
    fn ffi_connection_close_alias_matches_client_close_connection() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_connection_bootstrap(
                    NnrpConnectionBootstrap {
                        connection_id: 91_201,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection,
                ),
                NnrpFfiStatus::ok()
            );

            assert_eq!(nnrp_connection_close(connection), NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_connection_close(connection),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
        }
    }

    #[test]
    fn ffi_runtime_frame_send_snapshots_payload_and_transfers_ownership() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 193_001,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection,
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 93_002,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 4,
                    },
                    &mut session,
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let mut payload = TraceContextMetadata {
                trace_id: 101,
                span_id: 102,
                parent_span_id: 103,
                stage_code: 6,
                flags: nnrp_core::TRACE_CONTEXT_FLAGS_KNOWN_MASK,
                body_bytes: 3,
            }
            .to_vec_with_body(&[5, 6, 7])
            .expect("trace context metadata should encode");
            let expected = payload.clone();
            assert_eq!(
                nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                    handle: session,
                    message_type: MessageType::TraceContext as u32,
                    frame_id: 93,
                    payload: NnrpBufferView {
                        ptr: payload.as_ptr(),
                        len: payload.len(),
                    },
                }),
                NnrpFfiStatus::ok()
            );
            payload.fill(0);

            let mut result = empty_poll_result();
            assert_eq!(
                nnrp_client_await_event(connection, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.has_event, 1);
            assert_eq!(result.event.kind, NnrpEventKind::RuntimeFrame as u32);
            assert_eq!(result.event.message_type, MessageType::TraceContext as u32);
            assert_eq!(result.event.connection, connection);
            assert_eq!(result.event.session, session);
            assert_eq!(result.event.operation, NnrpHandle::invalid());
            assert_eq!(result.event.frame_id, 93);
            assert_eq!(ffi_read_slice(result.event.payload), expected.as_slice());
            assert_eq!(
                result.event.payload_owner.kind,
                NnrpHandleKind::Buffer as u32
            );
            assert_eq!(
                nnrp_buffer_release(result.event.payload_owner),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_buffer_release(result.event.payload_owner),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)
            );
            assert_eq!(nnrp_connection_close(connection), NnrpFfiStatus::ok());
        }
    }

    #[test]
    fn ffi_runtime_frame_send_enforces_handle_and_payload_contracts() {
        unsafe {
            let mut client = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 94_001,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut client,
                ),
                NnrpFfiStatus::ok()
            );
            let mut client_session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection: client,
                        requested_session_id: 94_002,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 4,
                    },
                    &mut client_session,
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(client);

            let trace = TraceContextMetadata {
                trace_id: 201,
                span_id: 202,
                parent_span_id: 203,
                stage_code: 6,
                flags: nnrp_core::TRACE_CONTEXT_FLAGS_KNOWN_MASK,
                body_bytes: 0,
            }
            .to_vec_with_body(&[])
            .expect("trace context metadata should encode");
            assert_eq!(
                nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                    handle: client,
                    message_type: MessageType::TraceContext as u32,
                    frame_id: 93,
                    payload: NnrpBufferView {
                        ptr: trace.as_ptr(),
                        len: trace.len(),
                    },
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
            assert_eq!(
                nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                    handle: NnrpHandle::new(NnrpHandleKind::Session, 999_999, 1),
                    message_type: MessageType::TraceContext as u32,
                    frame_id: 93,
                    payload: NnrpBufferView {
                        ptr: trace.as_ptr(),
                        len: trace.len(),
                    },
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
            );

            let progress = ProgressMetadata {
                operation_id: 51,
                progress_sequence: 52,
                stage_code: 5,
                percent_x100: 8_750,
                object_id: 53,
                body_bytes: 2,
            }
            .to_vec_with_body(&[9, 8])
            .expect("progress metadata should encode");
            let client_direction = nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                handle: client_session,
                message_type: MessageType::Progress as u32,
                frame_id: 94,
                payload: NnrpBufferView {
                    ptr: progress.as_ptr(),
                    len: progress.len(),
                },
            });
            assert_eq!(client_direction, NnrpFfiStatus::ok());

            let malformed = [1u8, 2, 3];
            let malformed_status = nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                handle: client_session,
                message_type: MessageType::TraceContext as u32,
                frame_id: 95,
                payload: NnrpBufferView {
                    ptr: malformed.as_ptr(),
                    len: malformed.len(),
                },
            });
            assert_eq!(
                malformed_status.status_code,
                NnrpFfiStatusCode::InvalidArgument as u32
            );

            let object_delta_metadata = ObjectDeltaMetadata {
                object_id: 9,
                delta_sequence: 2,
                region_offset: 128,
                region_bytes: 64,
                delta_bytes: 4,
                flags: nnrp_core::OBJECT_DELTA_FLAGS_KNOWN_MASK,
                metadata_bytes: 2,
            };
            let mut object_delta = object_delta_metadata
                .to_bytes()
                .expect("object delta metadata should encode")
                .to_vec();
            object_delta.extend_from_slice(b"md");
            object_delta.extend_from_slice(b"xxxx");
            for (frame_id, message_type) in [
                (96, MessageType::ObjectPatch),
                (97, MessageType::ObjectDelta),
            ] {
                assert_eq!(
                    nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                        handle: client_session,
                        message_type: message_type as u32,
                        frame_id,
                        payload: NnrpBufferView {
                            ptr: object_delta.as_ptr(),
                            len: object_delta.len(),
                        },
                    }),
                    NnrpFfiStatus::ok()
                );
            }
            let truncated_object_delta = &object_delta[..object_delta.len() - 1];
            let truncated_status = nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                handle: client_session,
                message_type: MessageType::ObjectDelta as u32,
                frame_id: 98,
                payload: NnrpBufferView {
                    ptr: truncated_object_delta.as_ptr(),
                    len: truncated_object_delta.len(),
                },
            });
            assert_eq!(
                truncated_status.status_code,
                NnrpFfiStatusCode::ProtocolError as u32
            );

            let mut server = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_bind(
                    NnrpServerBindRequest {
                        server_id: 94_101,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut server,
                ),
                NnrpFfiStatus::ok()
            );
            let mut server_session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_accept(
                    NnrpServerAcceptRequest {
                        server,
                        session_id: 94_102,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 4,
                    },
                    &mut server_session,
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(server);
            assert_eq!(
                nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                    handle: server_session,
                    message_type: MessageType::Progress as u32,
                    frame_id: 96,
                    payload: NnrpBufferView {
                        ptr: progress.as_ptr(),
                        len: progress.len(),
                    },
                }),
                NnrpFfiStatus::ok()
            );

            let cancel = ControlRequestMetadata {
                operation_id: 11,
                control_sequence: 12,
                reason_code: 1,
                source_role: 4,
                flags: nnrp_core::CONTROL_REQUEST_FLAGS_KNOWN_MASK,
                diagnostic_bytes: 0,
            }
            .to_vec_with_diagnostics(&[])
            .expect("cancel metadata should encode");
            let server_direction = nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                handle: server_session,
                message_type: MessageType::Cancel as u32,
                frame_id: 97,
                payload: NnrpBufferView {
                    ptr: cancel.as_ptr(),
                    len: cancel.len(),
                },
            });
            assert_eq!(server_direction, NnrpFfiStatus::ok());

            let mut result = empty_poll_result();
            assert_eq!(
                nnrp_client_await_event(server, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::RuntimeFrame as u32);
            assert_eq!(result.event.message_type, MessageType::Progress as u32);
            assert_eq!(ffi_read_slice(result.event.payload), progress.as_slice());
            assert_eq!(
                nnrp_buffer_release(result.event.payload_owner),
                NnrpFfiStatus::ok()
            );

            assert_eq!(nnrp_connection_close(client), NnrpFfiStatus::ok());
            assert_eq!(nnrp_connection_close(server), NnrpFfiStatus::ok());
        }
    }

    #[test]
    fn ffi_connection_close_releases_queued_runtime_frame_payloads() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 95_001,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection,
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 95_002,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 4,
                    },
                    &mut session,
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(connection);

            let trace = TraceContextMetadata {
                trace_id: 301,
                span_id: 302,
                parent_span_id: 303,
                stage_code: 6,
                flags: nnrp_core::TRACE_CONTEXT_FLAGS_KNOWN_MASK,
                body_bytes: 0,
            }
            .to_vec_with_body(&[])
            .expect("trace context metadata should encode");
            assert_eq!(
                nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                    handle: session,
                    message_type: MessageType::TraceContext as u32,
                    frame_id: 95,
                    payload: NnrpBufferView {
                        ptr: trace.as_ptr(),
                        len: trace.len(),
                    },
                }),
                NnrpFfiStatus::ok()
            );
            let payload_owner = {
                let store = handle_store();
                store
                    .events
                    .iter()
                    .find(|event| event.connection == connection)
                    .expect("runtime frame should be queued")
                    .payload_owner
            };
            assert_eq!(payload_owner.kind, NnrpHandleKind::Buffer as u32);

            assert_eq!(nnrp_connection_close(connection), NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_buffer_release(payload_owner),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32)
            );
        }
    }

    #[test]
    fn ffi_server_abi_emits_pollable_runtime_events() {
        unsafe {
            let mut server = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_bind(
                    NnrpServerBindRequest {
                        server_id: 92_001,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut server,
                ),
                NnrpFfiStatus::ok()
            );

            let mut result = empty_poll_result();
            assert_eq!(
                nnrp_client_await_event(server, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::ConnectionOpened as u32);
            assert_eq!(result.event.connection, server);

            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_accept(
                    NnrpServerAcceptRequest {
                        server,
                        session_id: 92_002,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_client_await_event(server, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::SessionOpened as u32);
            assert_eq!(result.event.session, session);

            let payload = [4u8, 5, 6];
            let mut operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_receive_submit(
                    NnrpServerReceiveSubmitRequest {
                        session,
                        operation_id: 92_003,
                        frame_id: 55,
                        payload: NnrpBufferView {
                            ptr: payload.as_ptr(),
                            len: payload.len(),
                        },
                    },
                    &mut operation,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_client_await_event(server, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::SubmitAccepted as u32);
            assert_eq!(result.event.operation, operation);
            assert_eq!(result.event.frame_id, 55);

            assert_eq!(
                nnrp_server_send_result(NnrpServerSendResultRequest {
                    operation,
                    payload: NnrpBufferView {
                        ptr: payload.as_ptr(),
                        len: payload.len(),
                    },
                }),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_client_await_event(server, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::ResultPushed as u32);
            assert_eq!(result.event.frame_id, 55);

            let flow_update = flow_update_payload();
            assert_eq!(
                nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                    handle: session,
                    message_type: MessageType::FlowUpdate as u32,
                    frame_id: 55,
                    payload: NnrpBufferView {
                        ptr: flow_update.as_ptr(),
                        len: flow_update.len(),
                    },
                }),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_client_await_event(server, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::FlowUpdated as u32);

            assert_eq!(nnrp_server_close(session), NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_client_await_event(server, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::SessionClosed as u32);
        }
    }

    #[test]
    fn ffi_server_hot_path_helpers_emit_partial_and_stale_drop_events() {
        unsafe {
            let mut server = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_bind(
                    NnrpServerBindRequest {
                        server_id: 92_101,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut server,
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_accept(
                    NnrpServerAcceptRequest {
                        server,
                        session_id: 92_102,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session,
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(server);

            let submit_payload = [1u8, 2, 3];
            let mut partial_operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_receive_submit(
                    NnrpServerReceiveSubmitRequest {
                        session,
                        operation_id: 92_103,
                        frame_id: 77,
                        payload: NnrpBufferView {
                            ptr: submit_payload.as_ptr(),
                            len: submit_payload.len(),
                        },
                    },
                    &mut partial_operation,
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(server);

            let partial_body = [0xa1u8, 0xa2, 0xa3];
            let mut result = empty_poll_result();
            assert_eq!(
                nnrp_server_send_partial_result(
                    NnrpServerSendPartialResultRequest {
                        operation: partial_operation,
                        partial_result: NnrpPartialResultDescriptor {
                            operation_id: 92_103,
                            result_sequence: 1,
                            object_id: 92_104,
                            delta_sequence: 1,
                            body_bytes: partial_body.len() as u32,
                            flags: nnrp_core::PARTIAL_RESULT_FLAGS_KNOWN_MASK,
                        },
                        partial_body: NnrpBufferView {
                            ptr: partial_body.as_ptr(),
                            len: partial_body.len(),
                        },
                        max_events: 1,
                    },
                    &mut result,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.has_event, 1);
            assert_eq!(result.event.kind, NnrpEventKind::PartialResult as u32);
            assert_eq!(result.event.operation, partial_operation);
            assert_eq!(result.event.frame_id, 77);
            assert_eq!(result.event.payload.len, partial_body.len());
            assert_eq!(
                core::slice::from_raw_parts(result.event.payload.ptr, result.event.payload.len),
                partial_body
            );

            let result_payload = [9u8, 8, 7];
            assert_eq!(
                nnrp_server_send_result(NnrpServerSendResultRequest {
                    operation: partial_operation,
                    payload: NnrpBufferView {
                        ptr: result_payload.as_ptr(),
                        len: result_payload.len(),
                    },
                }),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_client_await_event(server, &mut result),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.event.kind, NnrpEventKind::ResultPushed as u32);

            let mut stale_operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_receive_submit(
                    NnrpServerReceiveSubmitRequest {
                        session,
                        operation_id: 92_105,
                        frame_id: 78,
                        payload: NnrpBufferView {
                            ptr: submit_payload.as_ptr(),
                            len: submit_payload.len(),
                        },
                    },
                    &mut stale_operation,
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(server);

            let diagnostics = [0xd1u8, 0xd2];
            assert_eq!(
                nnrp_server_drop_stale_result(
                    NnrpServerDropStaleResultRequest {
                        operation: stale_operation,
                        drop_reason: NnrpResultDropReasonDescriptor {
                            operation_id: 92_105,
                            result_sequence: 2,
                            drop_reason_code: nnrp_core::RESULT_DROP_REASON_DEADLINE_EXPIRED,
                            source_role: RuntimeRole::Server as u8,
                            flags: nnrp_core::RESULT_DROP_FLAGS_KNOWN_MASK,
                            diagnostic_bytes: diagnostics.len() as u32,
                        },
                        diagnostics: NnrpBufferView {
                            ptr: diagnostics.as_ptr(),
                            len: diagnostics.len(),
                        },
                        max_events: 1,
                    },
                    &mut result,
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(result.has_event, 1);
            assert_eq!(result.event.kind, NnrpEventKind::ResultDropped as u32);
            assert_eq!(result.event.operation, stale_operation);
            assert_eq!(result.event.frame_id, 78);
            assert_eq!(result.event.payload.len, diagnostics.len());
            assert_eq!(
                core::slice::from_raw_parts(result.event.payload.ptr, result.event.payload.len),
                diagnostics
            );
            assert_eq!(
                nnrp_server_send_result(NnrpServerSendResultRequest {
                    operation: stale_operation,
                    payload: NnrpBufferView {
                        ptr: result_payload.as_ptr(),
                        len: result_payload.len(),
                    },
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32)
            );

            let invalid_partial_body = [0xeeu8];
            assert_eq!(
                nnrp_server_send_partial_result(
                    NnrpServerSendPartialResultRequest {
                        operation: stale_operation,
                        partial_result: NnrpPartialResultDescriptor {
                            operation_id: 92_999,
                            result_sequence: 1,
                            object_id: 92_104,
                            delta_sequence: 1,
                            body_bytes: invalid_partial_body.len() as u32,
                            flags: nnrp_core::PARTIAL_RESULT_FLAGS_KNOWN_MASK,
                        },
                        partial_body: NnrpBufferView {
                            ptr: invalid_partial_body.as_ptr(),
                            len: invalid_partial_body.len(),
                        },
                        max_events: 1,
                    },
                    &mut result,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32)
            );
        }
    }

    #[test]
    fn ffi_server_hot_path_helpers_cover_partial_and_drop_error_paths() {
        unsafe {
            let mut server = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_bind(
                    NnrpServerBindRequest {
                        server_id: 92_201,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut server,
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_accept(
                    NnrpServerAcceptRequest {
                        server,
                        session_id: 92_202,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session,
                ),
                NnrpFfiStatus::ok()
            );
            let submit_payload = [1u8];
            let mut operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_receive_submit(
                    NnrpServerReceiveSubmitRequest {
                        session,
                        operation_id: 92_203,
                        frame_id: 79,
                        payload: NnrpBufferView {
                            ptr: submit_payload.as_ptr(),
                            len: submit_payload.len(),
                        },
                    },
                    &mut operation,
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(server);

            let partial_body = [0x11u8, 0x12];
            let valid_partial = NnrpPartialResultDescriptor {
                operation_id: 92_203,
                result_sequence: 1,
                object_id: 92_204,
                delta_sequence: 1,
                body_bytes: partial_body.len() as u32,
                flags: nnrp_core::PARTIAL_RESULT_FLAGS_KNOWN_MASK,
            };
            let mut result = empty_poll_result();
            assert_eq!(
                nnrp_server_send_partial_result(
                    NnrpServerSendPartialResultRequest {
                        operation,
                        partial_result: valid_partial,
                        partial_body: NnrpBufferView {
                            ptr: partial_body.as_ptr(),
                            len: partial_body.len(),
                        },
                        max_events: 1,
                    },
                    core::ptr::null_mut(),
                ),
                NnrpFfiStatus::invalid_argument(132)
            );
            assert_eq!(
                nnrp_server_send_partial_result(
                    NnrpServerSendPartialResultRequest {
                        operation,
                        partial_result: valid_partial,
                        partial_body: NnrpBufferView {
                            ptr: core::ptr::null(),
                            len: 1,
                        },
                        max_events: 1,
                    },
                    &mut result,
                ),
                NnrpFfiStatus::invalid_argument(1)
            );
            assert_eq!(result.has_event, 0);
            assert_eq!(
                nnrp_server_send_partial_result(
                    NnrpServerSendPartialResultRequest {
                        operation,
                        partial_result: NnrpPartialResultDescriptor {
                            operation_id: 92_999,
                            ..valid_partial
                        },
                        partial_body: NnrpBufferView {
                            ptr: partial_body.as_ptr(),
                            len: partial_body.len(),
                        },
                        max_events: 1,
                    },
                    &mut result,
                ),
                NnrpFfiStatus::invalid_argument(133)
            );
            assert_ne!(
                nnrp_server_send_partial_result(
                    NnrpServerSendPartialResultRequest {
                        operation,
                        partial_result: NnrpPartialResultDescriptor {
                            object_id: 0,
                            ..valid_partial
                        },
                        partial_body: NnrpBufferView {
                            ptr: partial_body.as_ptr(),
                            len: partial_body.len(),
                        },
                        max_events: 1,
                    },
                    &mut result,
                )
                .status_code,
                NnrpFfiStatusCode::Ok as u32
            );
            assert_ne!(
                nnrp_server_send_partial_result(
                    NnrpServerSendPartialResultRequest {
                        operation,
                        partial_result: NnrpPartialResultDescriptor {
                            body_bytes: (partial_body.len() + 1) as u32,
                            ..valid_partial
                        },
                        partial_body: NnrpBufferView {
                            ptr: partial_body.as_ptr(),
                            len: partial_body.len(),
                        },
                        max_events: 1,
                    },
                    &mut result,
                )
                .status_code,
                NnrpFfiStatusCode::Ok as u32
            );
            assert_eq!(
                nnrp_server_send_partial_result(
                    NnrpServerSendPartialResultRequest {
                        operation: NnrpHandle::new(NnrpHandleKind::Operation, 92_888, 1),
                        partial_result: NnrpPartialResultDescriptor {
                            operation_id: 92_888,
                            ..valid_partial
                        },
                        partial_body: NnrpBufferView {
                            ptr: partial_body.as_ptr(),
                            len: partial_body.len(),
                        },
                        max_events: 1,
                    },
                    &mut result,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32)
            );

            let diagnostics = [0x21u8, 0x22];
            let valid_drop = NnrpResultDropReasonDescriptor {
                operation_id: 92_203,
                result_sequence: 1,
                drop_reason_code: nnrp_core::RESULT_DROP_REASON_DEADLINE_EXPIRED,
                source_role: RuntimeRole::Server as u8,
                flags: nnrp_core::RESULT_DROP_FLAGS_KNOWN_MASK,
                diagnostic_bytes: diagnostics.len() as u32,
            };
            assert_eq!(
                nnrp_server_drop_stale_result(
                    NnrpServerDropStaleResultRequest {
                        operation,
                        drop_reason: valid_drop,
                        diagnostics: NnrpBufferView {
                            ptr: diagnostics.as_ptr(),
                            len: diagnostics.len(),
                        },
                        max_events: 1,
                    },
                    core::ptr::null_mut(),
                ),
                NnrpFfiStatus::invalid_argument(134)
            );
            assert_eq!(
                nnrp_server_drop_stale_result(
                    NnrpServerDropStaleResultRequest {
                        operation,
                        drop_reason: valid_drop,
                        diagnostics: NnrpBufferView {
                            ptr: core::ptr::null(),
                            len: 1,
                        },
                        max_events: 1,
                    },
                    &mut result,
                ),
                NnrpFfiStatus::invalid_argument(1)
            );
            assert_eq!(
                nnrp_server_drop_stale_result(
                    NnrpServerDropStaleResultRequest {
                        operation,
                        drop_reason: NnrpResultDropReasonDescriptor {
                            operation_id: 92_999,
                            ..valid_drop
                        },
                        diagnostics: NnrpBufferView {
                            ptr: diagnostics.as_ptr(),
                            len: diagnostics.len(),
                        },
                        max_events: 1,
                    },
                    &mut result,
                ),
                NnrpFfiStatus::invalid_argument(135)
            );
            assert_ne!(
                nnrp_server_drop_stale_result(
                    NnrpServerDropStaleResultRequest {
                        operation,
                        drop_reason: NnrpResultDropReasonDescriptor {
                            drop_reason_code: 0,
                            ..valid_drop
                        },
                        diagnostics: NnrpBufferView {
                            ptr: diagnostics.as_ptr(),
                            len: diagnostics.len(),
                        },
                        max_events: 1,
                    },
                    &mut result,
                )
                .status_code,
                NnrpFfiStatusCode::Ok as u32
            );
            assert_ne!(
                nnrp_server_drop_stale_result(
                    NnrpServerDropStaleResultRequest {
                        operation,
                        drop_reason: NnrpResultDropReasonDescriptor {
                            diagnostic_bytes: (diagnostics.len() + 1) as u32,
                            ..valid_drop
                        },
                        diagnostics: NnrpBufferView {
                            ptr: diagnostics.as_ptr(),
                            len: diagnostics.len(),
                        },
                        max_events: 1,
                    },
                    &mut result,
                )
                .status_code,
                NnrpFfiStatusCode::Ok as u32
            );
        }
    }

    #[test]
    fn ffi_operation_event_polling_covers_empty_invalid_and_limited_paths() {
        unsafe {
            let mut server = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_bind(
                    NnrpServerBindRequest {
                        server_id: 92_301,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut server,
                ),
                NnrpFfiStatus::ok()
            );
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_accept(
                    NnrpServerAcceptRequest {
                        server,
                        session_id: 92_302,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session,
                ),
                NnrpFfiStatus::ok()
            );
            let mut operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_receive_submit(
                    NnrpServerReceiveSubmitRequest {
                        session,
                        operation_id: 92_303,
                        frame_id: 80,
                        payload: NnrpBufferView::empty(),
                    },
                    &mut operation,
                ),
                NnrpFfiStatus::ok()
            );
            drain_events(server);

            let scope = OperationEventScope {
                connection: server,
                session,
                frame_id: 80,
            };
            let mut result = empty_poll_result();
            assert_eq!(
                poll_matching_operation_event_from_scope(
                    scope,
                    operation,
                    operation.id,
                    NnrpEventKind::PartialResult,
                    NnrpBufferView::empty(),
                    1,
                    &mut result,
                )
                .status_code,
                NnrpFfiStatusCode::WouldBlock as u32
            );
            assert_eq!(result.has_event, 0);

            let flow_update = flow_update_payload();
            assert_eq!(
                nnrp_runtime_frame_send_impl(NnrpRuntimeFrameSendRequest {
                    handle: session,
                    message_type: MessageType::FlowUpdate as u32,
                    frame_id: 80,
                    payload: NnrpBufferView {
                        ptr: flow_update.as_ptr(),
                        len: flow_update.len(),
                    },
                }),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                poll_matching_operation_event_from_scope(
                    scope,
                    operation,
                    operation.id,
                    NnrpEventKind::PartialResult,
                    NnrpBufferView::empty(),
                    1,
                    &mut result,
                )
                .status_code,
                NnrpFfiStatusCode::WouldBlock as u32
            );

            assert_eq!(
                poll_matching_operation_event_from_scope(
                    OperationEventScope {
                        connection: NnrpHandle::new(NnrpHandleKind::Connection, 92_999, 1),
                        session,
                        frame_id: 80,
                    },
                    operation,
                    operation.id,
                    NnrpEventKind::PartialResult,
                    NnrpBufferView::empty(),
                    1,
                    &mut result,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
        }
    }

    #[test]
    fn ffi_rejects_cross_role_client_and_server_handles() {
        unsafe {
            let mut client = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 93_001,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut client,
                ),
                NnrpFfiStatus::ok()
            );
            let mut server = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_bind(
                    NnrpServerBindRequest {
                        server_id: 193_002,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut server,
                ),
                NnrpFfiStatus::ok()
            );

            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_open_session(
                    NnrpSessionOpenRequest {
                        connection: server,
                        requested_session_id: 193_003,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
            assert_eq!(
                nnrp_server_accept(
                    NnrpServerAcceptRequest {
                        server: client,
                        session_id: 193_004,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
            assert_eq!(nnrp_connection_close(client), NnrpFfiStatus::ok());
            assert_eq!(nnrp_connection_close(server), NnrpFfiStatus::ok());
        }
    }

    #[test]
    fn ffi_entrypoints_reject_invalid_arguments_and_empty_poll_would_block() {
        unsafe {
            assert_eq!(
                nnrp_connection_bootstrap(
                    NnrpConnectionBootstrap {
                        connection_id: 0,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    ptr::null_mut()
                ),
                NnrpFfiStatus::invalid_argument(10)
            );

            let mut result = NnrpPollResult {
                status: NnrpFfiStatus::ok(),
                has_event: 1,
                event: NnrpEvent::none(),
            };
            let status = nnrp_poll_empty(&mut result);
            assert_eq!(status.status_code, NnrpFfiStatusCode::WouldBlock as u32);
            assert_eq!(result.has_event, 0);
        }
    }

    #[test]
    fn ffi_runtime_handle_store_rejects_unregistered_and_stale_handles() {
        unsafe {
            let unregistered_connection = NnrpHandle::new(NnrpHandleKind::Connection, 90_001, 1);
            let mut session = NnrpHandle::invalid();
            assert_eq!(
                nnrp_session_open(
                    NnrpSessionOpenRequest {
                        connection: unregistered_connection,
                        requested_session_id: 90_002,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );

            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_connection_bootstrap(
                    NnrpConnectionBootstrap {
                        connection_id: 90_003,
                        generation: 1,
                        transport_id: test_transport_id(),
                    },
                    &mut connection
                ),
                NnrpFfiStatus::ok()
            );

            let mut stale_connection = connection;
            stale_connection.generation += 1;
            assert_eq!(
                nnrp_session_open(
                    NnrpSessionOpenRequest {
                        connection: stale_connection,
                        requested_session_id: 90_004,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );

            assert_eq!(
                nnrp_session_open(
                    NnrpSessionOpenRequest {
                        connection,
                        requested_session_id: 90_005,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session
                ),
                NnrpFfiStatus::ok()
            );

            let mut stale_session = session;
            stale_session.generation += 1;
            let payload = [1u8];
            let mut operation = NnrpHandle::invalid();
            assert_eq!(
                nnrp_submit(
                    NnrpSubmitRequest {
                        session: stale_session,
                        operation_id: 90_006,
                        frame_id: 1,
                        payload: NnrpBufferView {
                            ptr: payload.as_ptr(),
                            len: payload.len(),
                        },
                    },
                    &mut operation
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
            );

            assert_eq!(
                nnrp_submit(
                    NnrpSubmitRequest {
                        session,
                        operation_id: 90_006,
                        frame_id: 9,
                        payload: NnrpBufferView {
                            ptr: payload.as_ptr(),
                            len: payload.len(),
                        },
                    },
                    &mut operation
                ),
                NnrpFfiStatus::ok()
            );

            let store = handle_store();
            assert!(matches!(
                store
                    .get(connection, NnrpHandleKind::Connection)
                    .expect("connection should be registered"),
                NnrpFfiResource::Connection {
                    transport_id,
                    role: NnrpFfiConnectionRole::Client
                } if *transport_id == test_transport_id()
            ));
            assert!(matches!(
                store
                    .get(session, NnrpHandleKind::Session)
                    .expect("session should be registered"),
                NnrpFfiResource::Session {
                    profile_id: 2,
                    schema_id: 0x1001,
                    schema_version: 3,
                    ..
                }
            ));
            assert!(matches!(
                store
                    .get(operation, NnrpHandleKind::Operation)
                    .expect("operation should be registered"),
                NnrpFfiResource::Operation {
                    frame_id: 9,
                    payload_len: 1,
                    ..
                }
            ));
        }
    }

    #[test]
    fn ffi_handle_store_rejects_invalid_resource_kinds() {
        let mut store = NnrpFfiHandleStore::default();
        assert_eq!(
            store.insert(
                NnrpHandle {
                    kind: 99,
                    id: 1,
                    generation: 1,
                    flags: 0,
                },
                NnrpFfiResource::Connection {
                    transport_id: test_transport_id(),
                    role: NnrpFfiConnectionRole::Client,
                },
            ),
            Err(NnrpFfiStatus::invalid_handle(99))
        );
        assert_eq!(
            store.insert(
                NnrpHandle::new(NnrpHandleKind::Connection, 1, 1),
                NnrpFfiResource::Connection {
                    transport_id: test_transport_id(),
                    role: NnrpFfiConnectionRole::Client,
                },
            ),
            Ok(())
        );
        assert_eq!(
            store.remove(
                NnrpHandle::new(NnrpHandleKind::Connection, 1, 1),
                NnrpHandleKind::Session,
            ),
            Err(NnrpFfiStatus::invalid_handle(
                NnrpHandleKind::Session as u32
            ))
        );
    }

    #[test]
    fn ffi_dispatch_supports_callback_and_rejection_status() {
        extern "C" fn ok_callback(_: *mut c_void, event: *const NnrpEvent) -> u32 {
            assert!(!event.is_null());
            NnrpFfiStatusCode::Ok as u32
        }
        extern "C" fn reject_callback(_: *mut c_void, _: *const NnrpEvent) -> u32 {
            99
        }

        unsafe {
            let event = NnrpEvent::none();
            assert_eq!(
                nnrp_dispatch_event(
                    NnrpCallbackSink {
                        user_data: ptr::null_mut(),
                        on_event: Some(ok_callback),
                    },
                    &event
                ),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_dispatch_event(
                    NnrpCallbackSink {
                        user_data: ptr::null_mut(),
                        on_event: Some(reject_callback),
                    },
                    &event
                ),
                NnrpFfiStatus {
                    status_code: NnrpFfiStatusCode::CallbackRejected as u32,
                    error_family: NnrpErrorFamily::None as u32,
                    protocol_error_code: 0,
                    detail_code: 99,
                }
            );
        }
    }

    #[test]
    fn ffi_maps_core_and_protocol_error_families() {
        assert_eq!(
            NnrpFfiStatus::invalid_state(7),
            NnrpFfiStatus {
                status_code: NnrpFfiStatusCode::InvalidState as u32,
                error_family: NnrpErrorFamily::Lifecycle as u32,
                protocol_error_code: 0,
                detail_code: 7,
            }
        );
        assert_eq!(
            session_error_status(SESSION_ERROR_RESUME_REJECTED),
            NnrpFfiStatus::protocol(NnrpErrorFamily::Session, SESSION_ERROR_RESUME_REJECTED)
        );
        assert_eq!(
            NnrpFfiStatus::from_core_error(&NnrpError::InvalidProtocolCombination { rule: "test" })
                .error_family,
            NnrpErrorFamily::Lifecycle as u32
        );
        assert_eq!(
            NnrpFfiStatus::from_core_error(&NnrpError::ReservedBitsSet {
                value: 1,
                allowed: 0
            })
            .error_family,
            NnrpErrorFamily::Transport as u32
        );
        assert_eq!(
            NnrpFfiStatus::from_core_error(&NnrpError::UnknownEnumValue {
                enum_name: "error_scope",
                value: 99
            })
            .error_family,
            NnrpErrorFamily::Control as u32
        );
        assert_eq!(
            NnrpFfiStatus::from_core_error(&NnrpError::UnknownEnumValue {
                enum_name: "runtime_object_kind",
                value: 99
            })
            .error_family,
            NnrpErrorFamily::RuntimeObject as u32
        );
        assert_eq!(
            NnrpFfiStatus::from_core_error(&NnrpError::UnknownEnumValue {
                enum_name: "cache_object_kind",
                value: 99
            })
            .error_family,
            NnrpErrorFamily::Cache as u32
        );
        assert_eq!(
            NnrpFfiStatus::from_core_error(&NnrpError::NonZeroReservedField {
                field: "control_request.reserved"
            })
            .error_family,
            NnrpErrorFamily::Control as u32
        );
        assert_eq!(
            NnrpFfiStatus::from_core_error(&NnrpError::NonZeroReservedField {
                field: "object_descriptor.reserved"
            })
            .error_family,
            NnrpErrorFamily::RuntimeObject as u32
        );
    }

    fn empty_poll_result() -> NnrpPollResult {
        NnrpPollResult {
            status: NnrpFfiStatus::ok(),
            has_event: 0,
            event: NnrpEvent::none(),
        }
    }

    unsafe fn drain_events(connection: NnrpHandle) {
        let mut result = empty_poll_result();
        while nnrp_client_await_event(connection, &mut result).status_code
            == NnrpFfiStatusCode::Ok as u32
        {
            if result.event.payload_owner.kind == NnrpHandleKind::Buffer as u32 {
                assert_eq!(
                    nnrp_buffer_release(result.event.payload_owner),
                    NnrpFfiStatus::ok()
                );
            }
            result = empty_poll_result();
        }
    }

    fn recovery_session_open(
        requested_session_id: u32,
        resume_token_bytes: u32,
        session_flags: u8,
    ) -> nnrp_core::SessionOpenMetadata {
        nnrp_core::SessionOpenMetadata {
            requested_session_id,
            profile_id: 2,
            priority_class: nnrp_core::SessionPriorityClass::Balanced,
            session_flags,
            schema_id: 0x1001,
            schema_version: 3,
            default_deadline_ms: 500,
            max_in_flight_operations: 8,
            lease_ttl_hint_ms: 30_000,
            resume_token_bytes,
            auth_bytes: 0,
            session_extension_bytes: 0,
            client_session_tag: 1,
        }
    }

    fn recovery_session_ack(
        session_status: nnrp_core::SessionStatus,
        session_flags_ack: u32,
        resume_window_ms: u32,
        resume_token_bytes: u32,
        session_error_code: u32,
    ) -> nnrp_core::SessionOpenAckMetadata {
        nnrp_core::SessionOpenAckMetadata {
            session_id: 42,
            accepted_profile_id: 2,
            accepted_priority_class: nnrp_core::SessionPriorityClass::Balanced,
            session_status,
            schema_id: 0x1001,
            schema_version: 3,
            granted_operation_credit: 4,
            max_in_flight_operations: 8,
            lease_ttl_ms: 30_000,
            resume_window_ms,
            resume_token_bytes,
            session_extension_bytes: 0,
            server_session_tag: 7,
            route_scope_id: 0,
            session_error_code,
            session_flags_ack,
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
