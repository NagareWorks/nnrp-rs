use core::ffi::c_void;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, MutexGuard, OnceLock};

use nnrp_core::{
    should_replay_frame_after_migration, token_delta_schema_descriptor,
    validate_migration_recovery, validate_session_recovery_ack, validate_session_recovery_request,
    NnrpError, ProtocolVersion, SchemaDescriptorHeader, SchemaRegistry, SchemaRegistryFailure,
    SessionMigrateAckMetadata, SessionMigrateMetadata, SessionOpenAckMetadata, SessionOpenMetadata,
    SessionRecoveryOutcome as CoreSessionRecoveryOutcome, TransportId, TypedPayloadDescriptor,
    SESSION_ERROR_NONE, SESSION_ERROR_PROFILE_UNSUPPORTED, SESSION_ERROR_RESUME_REJECTED,
    SESSION_ERROR_SCHEMA_UNSUPPORTED,
};

pub const NNRP_FFI_ABI_MAJOR: u16 = 1;
pub const NNRP_FFI_ABI_MINOR: u16 = 1;
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

pub const NNRP_SESSION_RECOVERY_OUTCOME_FRESH: u32 = 0;
pub const NNRP_SESSION_RECOVERY_OUTCOME_RESUME_ENABLED: u32 = 1;
pub const NNRP_SESSION_RECOVERY_OUTCOME_RESUMED: u32 = 2;
pub const NNRP_SESSION_RECOVERY_OUTCOME_RESUME_REJECTED: u32 = 3;

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
        sdk_revision: 1,
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
            | NNRP_RUNTIME_FEATURE_BATCH_POLLING,
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NnrpFfiConnectionRole {
    Client,
    Server,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

        for operation in operations {
            self.entries.remove(&(operation.kind, operation.id));
        }
        for session in sessions {
            self.entries.remove(&(session.kind, session.id));
        }
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
    let mut store = handle_store();
    let (connection, session, frame_id) =
        match store.get(request.operation, NnrpHandleKind::Operation) {
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
    store.push_event(NnrpQueuedEvent {
        kind: NnrpEventKind::ResultPushed as u32,
        connection,
        session,
        operation: request.operation,
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
pub unsafe extern "C" fn nnrp_control(request: NnrpControlRequest) -> NnrpFfiStatus {
    if request.handle.kind == NnrpHandleKind::Invalid as u32 {
        return NnrpFfiStatus::invalid_handle(NnrpHandleKind::Invalid as u32);
    }
    if let Err(status) = request.payload.validate() {
        return status;
    }

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
        assert_eq!(capabilities.sdk_revision, 1);
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
