use core::ffi::c_void;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, MutexGuard, OnceLock};

use nnrp_core::{
    should_replay_frame_after_migration, token_delta_schema_descriptor,
    validate_migration_recovery, validate_session_recovery_ack, validate_session_recovery_request,
    CacheLease, CacheLeaseOwnerScope, CacheObjectId, CacheObjectKind, CacheValidationFailure,
    MessageType, NnrpError, ProtocolVersion, ResultHintMetadata, SchemaDescriptorHeader,
    SchemaRegistry, SchemaRegistryAction, SchemaRegistryFailure, SessionMigrateAckMetadata,
    SessionMigrateMetadata, SessionOpenAckMetadata, SessionOpenMetadata,
    SessionRecoveryOutcome as CoreSessionRecoveryOutcome, TransportId, TypedPayloadDescriptor,
    SESSION_ERROR_NONE, SESSION_ERROR_PROFILE_UNSUPPORTED, SESSION_ERROR_RESUME_REJECTED,
    SESSION_ERROR_SCHEMA_UNSUPPORTED, SESSION_FLAG_ALLOW_RESUME,
};

pub const NNRP_FFI_ABI_MAJOR: u16 = 1;
pub const NNRP_FFI_ABI_MINOR: u16 = 6;
pub const NNRP_FFI_ABI_PATCH: u16 = 0;

pub const NNRP_TRANSPORT_SLOT_QUIC: u32 = 0x0000_0001;
pub const NNRP_TRANSPORT_SLOT_TCP: u32 = 0x0000_0002;

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
pub const NNRP_RUNTIME_FEATURE_CLIENT_COMPLETION_HELPERS: u64 = 0x0000_0000_0000_4000;
pub const NNRP_RUNTIME_FEATURE_CLIENT_COARSE_RESULT_HELPERS: u64 = 0x0000_0000_0000_8000;
pub const NNRP_RUNTIME_FEATURE_CLIENT_COMPACT_RESULT_HELPERS: u64 = 0x0000_0000_0001_0000;

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
        sdk_major: 1,
        sdk_minor: 0,
        sdk_patch: 0,
        sdk_preview: 3,
        sdk_revision: 6,
        reserved1: 0,
        transport_slots: transport_slot_bit(TransportId::Quic)
            | transport_slot_bit(TransportId::Tcp),
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
            | NNRP_RUNTIME_FEATURE_CLIENT_COMPLETION_HELPERS
            | NNRP_RUNTIME_FEATURE_CLIENT_COARSE_RESULT_HELPERS
            | NNRP_RUNTIME_FEATURE_CLIENT_COMPACT_RESULT_HELPERS,
    }
}

const fn transport_slot_bit(transport_id: TransportId) -> u32 {
    match transport_id {
        TransportId::Quic => NNRP_TRANSPORT_SLOT_QUIC,
        TransportId::Tcp => NNRP_TRANSPORT_SLOT_TCP,
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
            NnrpError::UnknownEnumValue { .. }
            | NnrpError::ReservedBitsSet { .. }
            | NnrpError::NonZeroReservedField { .. }
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

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum NnrpFfiResource {
    Connection {
        transport_id: u32,
        role: NnrpFfiConnectionRole,
    },
    Session {
        connection: NnrpHandle,
        profile_id: u16,
        schema_id: u32,
        schema_version: u32,
    },
    Operation {
        session: NnrpHandle,
        frame_id: u32,
        payload_len: usize,
    },
    SchemaRegistry {
        registry: SchemaRegistry,
    },
    Buffer {
        bytes: Vec<u8>,
    },
    CacheLease {
        owner: NnrpHandle,
        lease: CacheLease,
        released: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NnrpFfiConnectionRole {
    Client,
    Server,
}

#[derive(Debug, Clone)]
struct NnrpFfiResourceEntry {
    generation: u32,
    resource: NnrpFfiResource,
}

#[derive(Debug, Default)]
struct NnrpFfiHandleStore {
    entries: BTreeMap<(u32, u64), NnrpFfiResourceEntry>,
    events: VecDeque<NnrpQueuedEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NnrpQueuedEvent {
    kind: u32,
    connection: NnrpHandle,
    session: NnrpHandle,
    operation: NnrpHandle,
    frame_id: u32,
}

impl NnrpQueuedEvent {
    fn into_event(self) -> NnrpEvent {
        NnrpEvent {
            kind: self.kind,
            connection: self.connection,
            session: self.session,
            operation: self.operation,
            frame_id: self.frame_id,
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
        self.events.retain(|event| event.connection != connection);
        Ok(())
    }

    fn push_event(&mut self, event: NnrpQueuedEvent) {
        self.events.push_back(event);
    }

    fn poll_event(&mut self, connection: NnrpHandle) -> Result<Option<NnrpEvent>, NnrpFfiStatus> {
        self.get(connection, NnrpHandleKind::Connection)?;
        let Some(index) = self
            .events
            .iter()
            .position(|event| event.connection == connection)
        else {
            return Ok(None);
        };
        Ok(self.events.remove(index).map(NnrpQueuedEvent::into_event))
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
    pub cache_key_hi: u32,
    pub cache_key_lo: u32,
    pub object_kind: u32,
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
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpEvent {
    pub kind: u32,
    pub connection: NnrpHandle,
    pub session: NnrpHandle,
    pub operation: NnrpHandle,
    pub frame_id: u32,
    pub payload: NnrpBufferView,
    pub diagnostic: NnrpFfiDiagnostic,
}

impl NnrpEvent {
    pub const fn none() -> Self {
        Self {
            kind: NnrpEventKind::None as u32,
            connection: NnrpHandle::invalid(),
            session: NnrpHandle::invalid(),
            operation: NnrpHandle::invalid(),
            frame_id: 0,
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

    fn from_event(status: NnrpFfiStatus, event: NnrpEvent) -> Self {
        Self {
            status,
            has_result: 1,
            event_kind: event.kind,
            result_state: compact_result_state(status, event.kind),
            operation: event.operation,
            operation_id: event.operation.id,
            frame_id: event.frame_id,
            payload: event.payload,
            diagnostic: event.diagnostic,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub transport_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpServerBindRequest {
    pub server_id: u64,
    pub generation: u32,
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
    pub session_id: u32,
    pub generation: u32,
    pub profile_id: u16,
    pub schema_id: u32,
    pub schema_version: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub struct NnrpServerFlowUpdateRequest {
    pub session: NnrpHandle,
    pub frame_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NnrpControlRequest {
    pub handle: NnrpHandle,
    pub control_code: u32,
    pub payload: NnrpBufferView,
}

#[no_mangle]
/// # Safety
///
/// `out_connection` must be either null or a valid writable pointer to one
/// `NnrpHandle`. When non-null, the pointed memory must be owned by the caller.
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
/// `out_connection` must be either null or a valid writable pointer to one
/// `NnrpHandle`. When non-null, the pointed memory must be owned by the caller.
pub unsafe extern "C" fn nnrp_client_connect(
    request: NnrpClientConnectRequest,
    out_connection: *mut NnrpHandle,
) -> NnrpFfiStatus {
    if out_connection.is_null() || request.connection_id == 0 || request.generation == 0 {
        return NnrpFfiStatus::invalid_argument(10);
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

    store.push_event(NnrpQueuedEvent {
        kind: NnrpEventKind::ConnectionOpened as u32,
        connection: handle,
        session: NnrpHandle::invalid(),
        operation: NnrpHandle::invalid(),
        frame_id: 0,
    });
    *out_connection = handle;
    NnrpFfiStatus::ok()
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
pub unsafe extern "C" fn nnrp_client_open_session(
    request: NnrpSessionOpenRequest,
    out_session: *mut NnrpHandle,
) -> NnrpFfiStatus {
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

    store.push_event(NnrpQueuedEvent {
        kind: NnrpEventKind::SessionOpened as u32,
        connection: request.connection,
        session: handle,
        operation: NnrpHandle::invalid(),
        frame_id: 0,
    });
    *out_session = handle;
    NnrpFfiStatus::ok()
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

    let handle = NnrpHandle::new(
        NnrpHandleKind::Operation,
        request.operation_id,
        request.session.generation,
    );
    if let Err(status) = store.insert(
        handle,
        NnrpFfiResource::Operation {
            session: request.session,
            frame_id: request.frame_id,
            payload_len: request.payload.len,
        },
    ) {
        return status;
    }

    store.push_event(NnrpQueuedEvent {
        kind: NnrpEventKind::SubmitAccepted as u32,
        connection: session_resource,
        session: request.session,
        operation: handle,
        frame_id: request.frame_id,
    });
    *out_operation = handle;
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
    let mut store = handle_store();
    let connection = match store.get(session, NnrpHandleKind::Session) {
        Ok(NnrpFfiResource::Session { connection, .. }) => *connection,
        Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
        Err(status) => return status,
    };
    store
        .remove(session, NnrpHandleKind::Session)
        .map(|_| {
            store.push_event(NnrpQueuedEvent {
                kind: NnrpEventKind::SessionClosed as u32,
                connection,
                session,
                operation: NnrpHandle::invalid(),
                frame_id: 0,
            });
            NnrpFfiStatus::ok()
        })
        .unwrap_or_else(|status| status)
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
    let mut store = handle_store();
    let connection = match store.get(request.session, NnrpHandleKind::Session) {
        Ok(NnrpFfiResource::Session { connection, .. }) => *connection,
        Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
        Err(status) => return status,
    };
    store.push_event(NnrpQueuedEvent {
        kind: NnrpEventKind::Control as u32,
        connection,
        session: request.session,
        operation: NnrpHandle::invalid(),
        frame_id: request.frame_id,
    });
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// `request.payload` must remain readable for `request.payload.len` bytes for
/// the duration of the call. This helper completes a client-owned operation and
/// queues a `RESULT_PUSHED` event on the owning client connection.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_client_complete_operation(request: NnrpClientCompleteOperationRequest) -> NnrpFfiStatus { nnrp_client_complete_operation_impl(request) }

unsafe fn nnrp_client_complete_operation_impl(
    request: NnrpClientCompleteOperationRequest,
) -> NnrpFfiStatus {
    if let Err(status) = request.payload.validate() {
        return status;
    }
    push_operation_event(request.operation, NnrpEventKind::ResultPushed, true)
}

#[no_mangle]
/// # Safety
///
/// The operation handle is copied by value. This helper drops a client-owned
/// operation and queues a `RESULT_DROPPED` event on the owning client connection.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_client_drop_operation(request: NnrpClientDropOperationRequest) -> NnrpFfiStatus { nnrp_client_drop_operation_impl(request) }

fn nnrp_client_drop_operation_impl(request: NnrpClientDropOperationRequest) -> NnrpFfiStatus {
    push_operation_event(request.operation, NnrpEventKind::ResultDropped, true)
}

#[no_mangle]
/// # Safety
///
/// `request.submit_payload` and `request.result_payload` must remain readable
/// for their declared lengths for the duration of the call. `out_operation`
/// and `out_result` must be either null or valid writable pointers to one
/// value each. The helper submits, completes, and polls the matching result
/// event in one ABI call.
pub unsafe extern "C" fn nnrp_client_submit_result(
    request: NnrpClientSubmitResultRequest,
    out_operation: *mut NnrpHandle,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    nnrp_client_submit_result_impl(request, out_operation, out_result)
}

#[no_mangle]
/// # Safety
///
/// `request.submit_payload` and `request.result_payload` must remain readable
/// for their declared lengths for the duration of the call. `out_result` must
/// point to one caller-owned writable `NnrpCompactResult`. This helper submits,
/// completes, and polls the matching terminal result in one ABI call while
/// returning only the compact fields needed by hot host-language paths.
pub unsafe extern "C" fn nnrp_client_submit_result_compact(
    request: NnrpClientSubmitResultRequest,
    out_result: *mut NnrpCompactResult,
) -> NnrpFfiStatus {
    nnrp_client_submit_result_compact_impl(request, out_result)
}

#[no_mangle]
/// # Safety
///
/// `request.submit_payload` and `request.result_payload` must remain readable
/// for their declared lengths for the duration of the call. `out_last_result`
/// must point to one caller-owned writable `NnrpCompactResult`; `out_completed`
/// must point to one caller-owned writable `uintptr_t`. This helper repeats the
/// compact submit/result operation in one ABI call so host language bindings can
/// amortize FFI boundary overhead without changing protocol semantics.
pub unsafe extern "C" fn nnrp_client_submit_result_compact_batch(
    request: NnrpClientSubmitResultBatchRequest,
    out_last_result: *mut NnrpCompactResult,
    out_completed: *mut usize,
) -> NnrpFfiStatus {
    nnrp_client_submit_result_compact_batch_impl(request, out_last_result, out_completed)
}

unsafe fn nnrp_client_submit_result_impl(
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
    let complete_status = nnrp_client_complete_operation_impl(NnrpClientCompleteOperationRequest {
        operation,
        payload: request.result_payload,
    });
    if complete_status.status_code != NnrpFfiStatusCode::Ok as u32 {
        return complete_status;
    }

    poll_matching_operation_result(
        request.session,
        operation,
        request.operation_id,
        request.frame_id,
        request.max_events,
        out_result,
    )
}

unsafe fn nnrp_client_submit_result_compact_impl(
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
    let complete_status = nnrp_client_complete_operation_impl(NnrpClientCompleteOperationRequest {
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

unsafe fn nnrp_client_submit_result_compact_batch_impl(
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
        let status = nnrp_client_submit_result_compact_impl(
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

#[no_mangle]
/// # Safety
///
/// The session handle is copied by value. This helper queues a `FLOW_UPDATED`
/// event on the owning client connection.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_client_send_flow_update(request: NnrpServerFlowUpdateRequest) -> NnrpFfiStatus { nnrp_client_send_flow_update_impl(request) }

unsafe fn nnrp_client_send_flow_update_impl(request: NnrpServerFlowUpdateRequest) -> NnrpFfiStatus {
    nnrp_server_send_flow_update(request)
}

#[no_mangle]
/// # Safety
///
/// `request.payload` must remain readable for `request.payload.len` bytes for
/// the duration of the call and must contain a valid `RESULT_HINT` metadata
/// payload.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_client_send_result_hint(request: NnrpControlRequest) -> NnrpFfiStatus { nnrp_client_send_result_hint_impl(request) }

unsafe fn nnrp_client_send_result_hint_impl(request: NnrpControlRequest) -> NnrpFfiStatus {
    if request.control_code != MessageType::ResultHint as u32 {
        return NnrpFfiStatus::invalid_argument(34);
    }
    nnrp_control_impl(request)
}

#[no_mangle]
/// # Safety
///
/// `out_result` must be either null or a valid writable pointer to one
/// `NnrpPollResult`. When non-null, the pointed memory must be owned by the caller.
pub unsafe extern "C" fn nnrp_client_await_event(
    connection: NnrpHandle,
    out_result: *mut NnrpPollResult,
) -> NnrpFfiStatus {
    if out_result.is_null() {
        return NnrpFfiStatus::invalid_argument(17);
    }
    let mut store = handle_store();
    match store.poll_event(connection) {
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
    let id = next_handle_id(&store, NnrpHandleKind::SchemaRegistry);
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

unsafe fn poll_matching_operation_result(
    session: NnrpHandle,
    operation: NnrpHandle,
    operation_id: u64,
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
                if event_is_operation_result(event, session, operation, operation_id, frame_id) {
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
                if event_is_operation_result(event, session, operation, operation_id, frame_id) {
                    let mut result = NnrpCompactResult::from_event(NnrpFfiStatus::ok(), event);
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

fn event_is_operation_result(
    event: NnrpEvent,
    session: NnrpHandle,
    operation: NnrpHandle,
    operation_id: u64,
    frame_id: u32,
) -> bool {
    matches!(
        event.kind,
        value if value == NnrpEventKind::ResultPushed as u32
            || value == NnrpEventKind::ResultDropped as u32
            || value == NnrpEventKind::Error as u32
    ) && event.session == session
        && (event.operation.id == operation.id
            || event.operation.id == operation_id
            || event.frame_id == frame_id)
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

fn next_handle_id(store: &NnrpFfiHandleStore, kind: NnrpHandleKind) -> u64 {
    store
        .entries
        .keys()
        .filter_map(|(stored_kind, id)| (*stored_kind == kind as u32).then_some(*id))
        .max()
        .unwrap_or(0)
        .saturating_add(1)
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
    let id = next_handle_id(&store, NnrpHandleKind::Buffer);
    let handle = NnrpHandle::new(NnrpHandleKind::Buffer, id, 1);
    if let Err(status) = store.insert(handle, NnrpFfiResource::Buffer { bytes }) {
        return status;
    }
    let view = match store.get(handle, NnrpHandleKind::Buffer) {
        Ok(NnrpFfiResource::Buffer { bytes }) => NnrpBufferView {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
        },
        _ => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Buffer as u32),
    };
    *out_buffer = handle;
    *out_view = view;
    NnrpFfiStatus::ok()
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

    let id = next_handle_id(&store, NnrpHandleKind::CacheLease);
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
pub unsafe extern "C" fn nnrp_server_bind(
    request: NnrpServerBindRequest,
    out_server: *mut NnrpHandle,
) -> NnrpFfiStatus {
    if out_server.is_null() || request.server_id == 0 || request.generation == 0 {
        return NnrpFfiStatus::invalid_argument(18);
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
    store.push_event(NnrpQueuedEvent {
        kind: NnrpEventKind::ConnectionOpened as u32,
        connection: handle,
        session: NnrpHandle::invalid(),
        operation: NnrpHandle::invalid(),
        frame_id: 0,
    });
    *out_server = handle;
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// `out_session` must be either null or a valid writable pointer to one
/// `NnrpHandle`. The server handle is copied by value and is not retained.
pub unsafe extern "C" fn nnrp_server_accept(
    request: NnrpServerAcceptRequest,
    out_session: *mut NnrpHandle,
) -> NnrpFfiStatus {
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
    store.push_event(NnrpQueuedEvent {
        kind: NnrpEventKind::SessionOpened as u32,
        connection: request.server,
        session: handle,
        operation: NnrpHandle::invalid(),
        frame_id: 0,
    });
    *out_session = handle;
    NnrpFfiStatus::ok()
}

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
            frame_id: request.frame_id,
            payload_len: request.payload.len,
        },
    ) {
        return status;
    }
    store.push_event(NnrpQueuedEvent {
        kind: NnrpEventKind::SubmitAccepted as u32,
        connection,
        session: request.session,
        operation: handle,
        frame_id: request.frame_id,
    });
    *out_operation = handle;
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// `request.payload` must remain readable for `request.payload.len` bytes for
/// the duration of the call.
pub unsafe extern "C" fn nnrp_server_send_result(
    request: NnrpServerSendResultRequest,
) -> NnrpFfiStatus {
    if let Err(status) = request.payload.validate() {
        return status;
    }
    push_operation_event(request.operation, NnrpEventKind::ResultPushed, false)
}

fn push_operation_event(
    operation: NnrpHandle,
    event_kind: NnrpEventKind,
    remove_operation: bool,
) -> NnrpFfiStatus {
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
                Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
                Err(status) => return status,
            }
        }
        Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32),
        Err(status) => return status,
    };
    if remove_operation {
        store.entries.remove(&(operation.kind, operation.id));
    }
    store.push_event(NnrpQueuedEvent {
        kind: event_kind as u32,
        connection,
        session,
        operation,
        frame_id,
    });
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// The session handle is copied by value. This function does not dereference
/// caller-provided pointers.
pub unsafe extern "C" fn nnrp_server_send_flow_update(
    request: NnrpServerFlowUpdateRequest,
) -> NnrpFfiStatus {
    let mut store = handle_store();
    let connection = match store.get(request.session, NnrpHandleKind::Session) {
        Ok(NnrpFfiResource::Session { connection, .. }) => *connection,
        Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
        Err(status) => return status,
    };
    store.push_event(NnrpQueuedEvent {
        kind: NnrpEventKind::FlowUpdated as u32,
        connection,
        session: request.session,
        operation: NnrpHandle::invalid(),
        frame_id: request.frame_id,
    });
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// The session handle is copied by value. This function does not dereference
/// caller-provided pointers.
pub unsafe extern "C" fn nnrp_server_close(session: NnrpHandle) -> NnrpFfiStatus {
    let _ = core::hint::black_box(NnrpFfiConnectionRole::Server);
    let mut store = handle_store();
    let connection = match store.get(session, NnrpHandleKind::Session) {
        Ok(NnrpFfiResource::Session { connection, .. }) => *connection,
        Ok(_) => return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32),
        Err(status) => return status,
    };
    store
        .remove(session, NnrpHandleKind::Session)
        .map(|_| {
            store.push_event(NnrpQueuedEvent {
                kind: NnrpEventKind::SessionClosed as u32,
                connection,
                session,
                operation: NnrpHandle::invalid(),
                frame_id: 0,
            });
            NnrpFfiStatus::ok()
        })
        .unwrap_or_else(|status| status)
}

#[no_mangle]
/// # Safety
///
/// `request.payload` must remain readable for `request.payload.len` bytes for
/// the duration of the call.
#[rustfmt::skip]
pub unsafe extern "C" fn nnrp_control(request: NnrpControlRequest) -> NnrpFfiStatus { nnrp_control_impl(request) }

unsafe fn nnrp_control_impl(request: NnrpControlRequest) -> NnrpFfiStatus {
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
    store.push_event(NnrpQueuedEvent {
        kind: event_kind as u32,
        connection,
        session,
        operation,
        frame_id: 0,
    });
    NnrpFfiStatus::ok()
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

    #[test]
    fn ffi_current_version_stays_aligned() {
        let version = current_protocol_version();
        assert_eq!(version.major, 1);
        assert_eq!(version.wire_format, 0);
        assert_eq!(nnrp_current_protocol_version(), version);
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
        assert_eq!(capabilities.sdk_major, 1);
        assert_eq!(capabilities.sdk_minor, 0);
        assert_eq!(capabilities.sdk_patch, 0);
        assert_eq!(capabilities.sdk_preview, 3);
        assert_eq!(capabilities.sdk_revision, 6);
        assert_eq!(capabilities.reserved1, 0);
        assert_eq!(
            capabilities.transport_slots,
            NNRP_TRANSPORT_SLOT_QUIC | NNRP_TRANSPORT_SLOT_TCP
        );
        assert_eq!(transport_slot_bit(TransportId::Unspecified), 0);
        assert_eq!(
            transport_slot_bit(TransportId::Quic),
            NNRP_TRANSPORT_SLOT_QUIC
        );
        assert_eq!(
            transport_slot_bit(TransportId::Tcp),
            NNRP_TRANSPORT_SLOT_TCP
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
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_CLIENT_COMPLETION_HELPERS,
            0
        );
        assert_ne!(
            capabilities.feature_flags & NNRP_RUNTIME_FEATURE_CLIENT_COARSE_RESULT_HELPERS,
            0
        );
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
                        transport_id: 1,
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
                        transport_id: 1,
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
                        transport_id: 1,
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
                        transport_id: 1,
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
                nnrp_client_complete_operation(NnrpClientCompleteOperationRequest {
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
                nnrp_client_complete_operation(NnrpClientCompleteOperationRequest {
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
                nnrp_client_drop_operation(NnrpClientDropOperationRequest {
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
    fn ffi_client_submit_result_coalesces_hot_path() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_421,
                        generation: 1,
                        transport_id: 1,
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
                nnrp_client_submit_result(
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
            assert_eq!(operation.id, 91_423);
            assert_eq!(result.has_event, 1);
            assert_eq!(result.event.kind, NnrpEventKind::ResultPushed as u32);
            assert_eq!(result.event.session, session);
            assert_eq!(result.event.operation, operation);
            assert_eq!(result.event.frame_id, 58);
            assert_eq!(
                nnrp_client_complete_operation(NnrpClientCompleteOperationRequest {
                    operation,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32)
            );
        }
    }

    #[test]
    fn ffi_client_submit_result_compact_coalesces_hot_path() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_426,
                        generation: 1,
                        transport_id: 1,
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
                nnrp_client_submit_result_compact(
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
            assert_eq!(result.operation.id, 91_428);
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
    fn ffi_client_submit_result_compact_batch_amortizes_hot_path() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_429,
                        generation: 1,
                        transport_id: 1,
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
                nnrp_client_submit_result_compact_batch(
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
            assert_eq!(result.operation.id, 91_434);
            assert_eq!(result.operation_id, 91_434);
            assert_eq!(result.frame_id, 63);
            assert_eq!(result.payload.len, result_payload.len());

            completed = usize::MAX;
            result = NnrpCompactResult::none(NnrpFfiStatus::invalid_argument(1));
            assert_eq!(
                nnrp_client_submit_result_compact_batch(
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
                nnrp_client_submit_result_compact_batch(
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
                nnrp_client_submit_result_compact_batch(
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
    fn ffi_client_submit_result_reports_argument_and_poll_failures() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_431,
                        generation: 1,
                        transport_id: 1,
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
                        requested_session_id: 91_432,
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
                operation_id: 91_433,
                frame_id: 59,
                submit_payload: NnrpBufferView::empty(),
                result_payload: NnrpBufferView::empty(),
                max_events: 1,
            };
            assert_eq!(
                nnrp_client_submit_result(request, ptr::null_mut(), &mut result),
                NnrpFfiStatus::invalid_argument(47)
            );
            let status = nnrp_client_submit_result(request, &mut operation, &mut result);
            assert_eq!(status.status_code, NnrpFfiStatusCode::WouldBlock as u32);
            assert_eq!(operation.kind, NnrpHandleKind::Operation as u32);
            assert_eq!(operation.id, 91_433);
            assert_eq!(result.has_event, 0);
            assert_eq!(
                nnrp_client_submit_result(
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
                operation_id: 91_434,
                result_payload: NnrpBufferView {
                    ptr: ptr::null(),
                    len: 1,
                },
                ..request
            };
            let previous_operation = operation;
            assert_eq!(
                nnrp_client_submit_result(invalid_payload_request, &mut operation, &mut result),
                NnrpFfiStatus::invalid_argument(1)
            );
            assert_eq!(operation, previous_operation);
            drain_events(connection);
            let mut compact_result = NnrpCompactResult::none(NnrpFfiStatus::ok());
            assert_eq!(
                nnrp_client_submit_result_compact(request, ptr::null_mut()),
                NnrpFfiStatus::invalid_argument(48)
            );
            assert_eq!(
                nnrp_client_submit_result_compact(
                    NnrpClientSubmitResultRequest {
                        operation_id: 91_435,
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
                nnrp_client_submit_result_compact(
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
                nnrp_client_submit_result_compact(
                    NnrpClientSubmitResultRequest {
                        session: NnrpHandle::new(NnrpHandleKind::Operation, 91_436, 1),
                        operation_id: 91_436,
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
            let status = nnrp_client_submit_result_compact(
                NnrpClientSubmitResultRequest {
                    operation_id: 91_437,
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
            let invalid_session = NnrpHandle::new(NnrpHandleKind::Session, 91_438, 1);
            assert_eq!(
                poll_matching_operation_compact_result(
                    invalid_session,
                    NnrpHandle::new(NnrpHandleKind::Operation, 91_438, 1),
                    91_438,
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
                        transport_id: 1,
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
            assert_eq!(
                nnrp_client_send_flow_update(NnrpServerFlowUpdateRequest {
                    session,
                    frame_id: 57,
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
                nnrp_client_send_result_hint(NnrpControlRequest {
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
                nnrp_client_send_result_hint(NnrpControlRequest {
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
    fn ffi_client_abi_batch_poll_rejects_invalid_buffers() {
        unsafe {
            let mut connection = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 91_201,
                        generation: 1,
                        transport_id: 1,
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
                        transport_id: 1,
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
                        transport_id: 1,
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
                        transport_id: 1,
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
                        now_ms: result.expires_at_ms,
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
            assert_eq!(
                nnrp_client_close_connection(connection),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_cache_release(operation_lease, &mut result),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::CacheLease as u32)
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
                        transport_id: 1,
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
                        transport_id: 1,
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
                nnrp_control(NnrpControlRequest {
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
                nnrp_control(NnrpControlRequest {
                    handle: operation,
                    control_code: MessageType::ResultHint as u32,
                    payload: NnrpBufferView::empty(),
                })
                .status_code,
                NnrpFfiStatusCode::InvalidArgument as u32
            );

            assert_eq!(
                nnrp_control(NnrpControlRequest {
                    handle: connection,
                    control_code: MessageType::Ping as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_control(NnrpControlRequest {
                    handle: session,
                    control_code: MessageType::Pong as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::ok()
            );
            assert_eq!(
                nnrp_control(NnrpControlRequest {
                    handle: NnrpHandle::invalid(),
                    control_code: MessageType::Pong as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Invalid as u32)
            );
            assert_eq!(
                nnrp_control(NnrpControlRequest {
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
                nnrp_control(NnrpControlRequest {
                    handle: connection_handle,
                    control_code: MessageType::Ping as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
            assert_eq!(
                nnrp_control(NnrpControlRequest {
                    handle: session_handle,
                    control_code: MessageType::Ping as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Session as u32)
            );
            assert_eq!(
                nnrp_control(NnrpControlRequest {
                    handle: operation_handle,
                    control_code: MessageType::Ping as u32,
                    payload: NnrpBufferView::empty(),
                }),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Operation as u32)
            );
            assert_eq!(
                nnrp_control(NnrpControlRequest {
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
                        connection_id: 91_101,
                        generation: 1,
                        transport_id: 1,
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
                        requested_session_id: 91_102,
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
                        operation_id: 91_103,
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
                        transport_id: 1,
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
    fn ffi_server_abi_emits_pollable_runtime_events() {
        unsafe {
            let mut server = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_bind(
                    NnrpServerBindRequest {
                        server_id: 92_001,
                        generation: 1,
                        transport_id: 1,
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

            assert_eq!(
                nnrp_server_send_flow_update(NnrpServerFlowUpdateRequest {
                    session,
                    frame_id: 55,
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
    fn ffi_rejects_cross_role_client_and_server_handles() {
        unsafe {
            let mut client = NnrpHandle::invalid();
            assert_eq!(
                nnrp_client_connect(
                    NnrpClientConnectRequest {
                        connection_id: 93_001,
                        generation: 1,
                        transport_id: 1,
                    },
                    &mut client,
                ),
                NnrpFfiStatus::ok()
            );
            let mut server = NnrpHandle::invalid();
            assert_eq!(
                nnrp_server_bind(
                    NnrpServerBindRequest {
                        server_id: 93_002,
                        generation: 1,
                        transport_id: 1,
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
                        requested_session_id: 93_003,
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
                        session_id: 93_004,
                        generation: 1,
                        profile_id: 2,
                        schema_id: 0x1001,
                        schema_version: 3,
                    },
                    &mut session,
                ),
                NnrpFfiStatus::invalid_handle(NnrpHandleKind::Connection as u32)
            );
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
                        transport_id: 1,
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
                        transport_id: 1,
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
                    transport_id: 1,
                    role: NnrpFfiConnectionRole::Client
                }
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
                    transport_id: 1,
                    role: NnrpFfiConnectionRole::Client,
                },
            ),
            Err(NnrpFfiStatus::invalid_handle(99))
        );
        assert_eq!(
            store.insert(
                NnrpHandle::new(NnrpHandleKind::Connection, 1, 1),
                NnrpFfiResource::Connection {
                    transport_id: 1,
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
