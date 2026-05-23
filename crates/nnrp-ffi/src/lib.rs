use core::ffi::c_void;
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use nnrp_core::{
    NnrpError, ProtocolVersion, SESSION_ERROR_NONE, SESSION_ERROR_PROFILE_UNSUPPORTED,
    SESSION_ERROR_RESUME_REJECTED, SESSION_ERROR_SCHEMA_UNSUPPORTED,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct NnrpFfiResourceEntry {
    generation: u32,
    resource: NnrpFfiResource,
}

#[derive(Debug, Default)]
struct NnrpFfiHandleStore {
    entries: BTreeMap<(u32, u64), NnrpFfiResourceEntry>,
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
        },
    ) {
        return status;
    }

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
    if out_session.is_null() || request.requested_session_id == 0 || request.generation == 0 {
        return NnrpFfiStatus::invalid_argument(11);
    }
    let mut store = handle_store();
    if let Err(status) = store.get(request.connection, NnrpHandleKind::Connection) {
        return status;
    };

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
    if out_operation.is_null() || request.operation_id == 0 {
        return NnrpFfiStatus::invalid_argument(12);
    }
    if let Err(status) = request.payload.validate() {
        return status;
    }
    let mut store = handle_store();
    if let Err(status) = store.get(request.session, NnrpHandleKind::Session) {
        return status;
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

    *out_operation = handle;
    NnrpFfiStatus::ok()
}

#[no_mangle]
/// # Safety
///
/// The session handle is copied by value. This function does not dereference
/// caller-provided pointers.
pub unsafe extern "C" fn nnrp_session_close(session: NnrpHandle) -> NnrpFfiStatus {
    let mut store = handle_store();
    store
        .remove(session, NnrpHandleKind::Session)
        .map(|_| NnrpFfiStatus::ok())
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
                NnrpFfiResource::Connection { transport_id: 1 }
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
                NnrpFfiResource::Connection { transport_id: 1 },
            ),
            Err(NnrpFfiStatus::invalid_handle(99))
        );
        assert_eq!(
            store.insert(
                NnrpHandle::new(NnrpHandleKind::Connection, 1, 1),
                NnrpFfiResource::Connection { transport_id: 1 },
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
}
