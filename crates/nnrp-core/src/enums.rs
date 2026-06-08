use crate::NnrpError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    ClientHello = 0x01,
    ServerHelloAck = 0x02,
    SessionPatch = 0x03,
    SessionPatchAck = 0x04,
    Close = 0x05,
    Error = 0x06,
    SessionOpen = 0x07,
    SessionOpenAck = 0x08,
    SessionClose = 0x09,
    SessionCloseAck = 0x0a,
    FrameSubmit = 0x10,
    FrameCancel = 0x11,
    ResultPush = 0x12,
    ResultDrop = 0x13,
    CachePut = 0x14,
    CacheAck = 0x15,
    CacheInvalidate = 0x16,
    FlowUpdate = 0x17,
    ResultHint = 0x18,
    TransportProbe = 0x19,
    TransportProbeAck = 0x1a,
    SessionMigrate = 0x1b,
    SessionMigrateAck = 0x1c,
    Ping = 0x20,
    Pong = 0x21,
    Cancel = 0x30,
    Abort = 0x31,
    PriorityUpdate = 0x32,
    Deadline = 0x33,
    ExpireAt = 0x34,
    Supersede = 0x35,
    BudgetUpdate = 0x36,
    Progress = 0x37,
    PartialResult = 0x38,
    Backpressure = 0x39,
    CreditUpdate = 0x3a,
    CapabilityNegotiation = 0x3b,
    DegradeProfile = 0x3c,
    RouteHint = 0x3d,
    ExecutionHint = 0x3e,
    TraceContext = 0x3f,
    ResultDropReason = 0x40,
    ErrorRecoverable = 0x48,
    RetryAfter = 0x49,
}

impl MessageType {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        let message_type = match value {
            0x01 => Self::ClientHello,
            0x02 => Self::ServerHelloAck,
            0x03 => Self::SessionPatch,
            0x04 => Self::SessionPatchAck,
            0x05 => Self::Close,
            0x06 => Self::Error,
            0x07 => Self::SessionOpen,
            0x08 => Self::SessionOpenAck,
            0x09 => Self::SessionClose,
            0x0a => Self::SessionCloseAck,
            0x10 => Self::FrameSubmit,
            0x11 => Self::FrameCancel,
            0x12 => Self::ResultPush,
            0x13 => Self::ResultDrop,
            0x14 => Self::CachePut,
            0x15 => Self::CacheAck,
            0x16 => Self::CacheInvalidate,
            0x17 => Self::FlowUpdate,
            0x18 => Self::ResultHint,
            0x19 => Self::TransportProbe,
            0x1a => Self::TransportProbeAck,
            0x1b => Self::SessionMigrate,
            0x1c => Self::SessionMigrateAck,
            0x20 => Self::Ping,
            0x21 => Self::Pong,
            0x30 => Self::Cancel,
            0x31 => Self::Abort,
            0x32 => Self::PriorityUpdate,
            0x33 => Self::Deadline,
            0x34 => Self::ExpireAt,
            0x35 => Self::Supersede,
            0x36 => Self::BudgetUpdate,
            0x37 => Self::Progress,
            0x38 => Self::PartialResult,
            0x39 => Self::Backpressure,
            0x3a => Self::CreditUpdate,
            0x3b => Self::CapabilityNegotiation,
            0x3c => Self::DegradeProfile,
            0x3d => Self::RouteHint,
            0x3e => Self::ExecutionHint,
            0x3f => Self::TraceContext,
            0x40 => Self::ResultDropReason,
            0x48 => Self::ErrorRecoverable,
            0x49 => Self::RetryAfter,
            _ => return Err(NnrpError::UnknownMessageType(value)),
        };

        Ok(message_type)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SessionPriorityClass {
    Interactive = 0,
    Balanced = 1,
    Background = 2,
}

impl SessionPriorityClass {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Interactive),
            1 => Ok(Self::Balanced),
            2 => Ok(Self::Background),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "session_priority_class",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SessionStatus {
    Opened = 0,
    Rejected = 1,
    RetryLater = 2,
    Resumed = 3,
}

impl SessionStatus {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Opened),
            1 => Ok(Self::Rejected),
            2 => Ok(Self::RetryLater),
            3 => Ok(Self::Resumed),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "session_status",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum SessionCloseReason {
    Normal = 0,
    ClientShutdown = 1,
    ServerShutdown = 2,
    IdleTimeout = 3,
    ProtocolError = 4,
    AuthRevoked = 5,
}

impl SessionCloseReason {
    pub fn try_from_u16(value: u16) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Normal),
            1 => Ok(Self::ClientShutdown),
            2 => Ok(Self::ServerShutdown),
            3 => Ok(Self::IdleTimeout),
            4 => Ok(Self::ProtocolError),
            5 => Ok(Self::AuthRevoked),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "close_reason",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InFlightPolicy {
    Drain = 0,
    Abort = 1,
}

impl InFlightPolicy {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Drain),
            1 => Ok(Self::Abort),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "in_flight_policy",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SessionCloseStatus {
    Acknowledged = 0,
    Draining = 1,
    Closed = 2,
    Rejected = 3,
}

impl SessionCloseStatus {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Acknowledged),
            1 => Ok(Self::Draining),
            2 => Ok(Self::Closed),
            3 => Ok(Self::Rejected),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "close_status",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OperationState {
    Accepted = 0,
    Running = 1,
    Partial = 2,
    WaitingTool = 3,
    Superseded = 4,
    Cancelled = 5,
    Failed = 6,
    Completed = 7,
}

impl OperationState {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Accepted),
            1 => Ok(Self::Running),
            2 => Ok(Self::Partial),
            3 => Ok(Self::WaitingTool),
            4 => Ok(Self::Superseded),
            5 => Ok(Self::Cancelled),
            6 => Ok(Self::Failed),
            7 => Ok(Self::Completed),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "operation_state",
                value: value as u64,
            }),
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Superseded | Self::Cancelled | Self::Failed | Self::Completed
        )
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        if self.is_terminal() {
            return false;
        }

        matches!(
            (self, next),
            (Self::Accepted, Self::Running)
                | (Self::Accepted, Self::Cancelled)
                | (Self::Accepted, Self::Failed)
                | (Self::Accepted, Self::Superseded)
                | (Self::Running, Self::Partial)
                | (Self::Running, Self::WaitingTool)
                | (Self::Running, Self::Cancelled)
                | (Self::Running, Self::Failed)
                | (Self::Running, Self::Completed)
                | (Self::Running, Self::Superseded)
                | (Self::Partial, Self::Partial)
                | (Self::Partial, Self::WaitingTool)
                | (Self::Partial, Self::Cancelled)
                | (Self::Partial, Self::Failed)
                | (Self::Partial, Self::Completed)
                | (Self::Partial, Self::Superseded)
                | (Self::WaitingTool, Self::Running)
                | (Self::WaitingTool, Self::Cancelled)
                | (Self::WaitingTool, Self::Failed)
                | (Self::WaitingTool, Self::Superseded)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CancelScope {
    Operation = 0,
    Subtree = 1,
    Group = 2,
    Session = 3,
}

impl CancelScope {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Operation),
            1 => Ok(Self::Subtree),
            2 => Ok(Self::Group),
            3 => Ok(Self::Session),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "cancel_scope",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FlowScopeKind {
    Connection = 0,
    Session = 1,
    Operation = 2,
}

impl FlowScopeKind {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Connection),
            1 => Ok(Self::Session),
            2 => Ok(Self::Operation),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "scope_kind",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FlowUpdateReason {
    Grant = 0,
    Reduce = 1,
    Pause = 2,
    Resume = 3,
    Congestion = 4,
}

impl FlowUpdateReason {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Grant),
            1 => Ok(Self::Reduce),
            2 => Ok(Self::Pause),
            3 => Ok(Self::Resume),
            4 => Ok(Self::Congestion),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "update_reason",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BackpressureLevel {
    None = 0,
    Soft = 1,
    Hard = 2,
}

impl BackpressureLevel {
    pub fn try_from_u8(value: u8) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Soft),
            2 => Ok(Self::Hard),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "backpressure_level",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderFlags(pub u32);

impl HeaderFlags {
    pub const NONE: Self = Self(0);
    pub const ACK_REQUIRED: Self = Self(0x0000_0001);
    pub const CAN_DROP: Self = Self(0x0000_0002);
    pub const STALE: Self = Self(0x0000_0004);
    pub const EOS: Self = Self(0x0000_0008);
    pub const RETRANSMIT: Self = Self(0x0000_0010);
    pub const KEYFRAME: Self = Self(0x0000_0020);
    pub const KNOWN_MASK: u32 = Self::ACK_REQUIRED.0
        | Self::CAN_DROP.0
        | Self::STALE.0
        | Self::EOS.0
        | Self::RETRANSMIT.0
        | Self::KEYFRAME.0;

    pub fn validate_known(self) -> Result<(), NnrpError> {
        if self.0 & !Self::KNOWN_MASK != 0 {
            return Err(NnrpError::ReservedBitsSet {
                value: self.0 as u64,
                allowed: Self::KNOWN_MASK as u64,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::NnrpError;

    use super::{
        BackpressureLevel, CancelScope, FlowScopeKind, FlowUpdateReason, HeaderFlags,
        InFlightPolicy, MessageType, OperationState, SessionCloseReason, SessionCloseStatus,
        SessionPriorityClass, SessionStatus,
    };

    #[test]
    fn preview3_message_type_assignments_are_frozen() {
        let assignments = [
            (0x01, MessageType::ClientHello),
            (0x02, MessageType::ServerHelloAck),
            (0x03, MessageType::SessionPatch),
            (0x04, MessageType::SessionPatchAck),
            (0x05, MessageType::Close),
            (0x06, MessageType::Error),
            (0x07, MessageType::SessionOpen),
            (0x08, MessageType::SessionOpenAck),
            (0x09, MessageType::SessionClose),
            (0x0a, MessageType::SessionCloseAck),
            (0x10, MessageType::FrameSubmit),
            (0x11, MessageType::FrameCancel),
            (0x12, MessageType::ResultPush),
            (0x13, MessageType::ResultDrop),
            (0x14, MessageType::CachePut),
            (0x15, MessageType::CacheAck),
            (0x16, MessageType::CacheInvalidate),
            (0x17, MessageType::FlowUpdate),
            (0x18, MessageType::ResultHint),
            (0x19, MessageType::TransportProbe),
            (0x1a, MessageType::TransportProbeAck),
            (0x1b, MessageType::SessionMigrate),
            (0x1c, MessageType::SessionMigrateAck),
            (0x20, MessageType::Ping),
            (0x21, MessageType::Pong),
            (0x30, MessageType::Cancel),
            (0x31, MessageType::Abort),
            (0x32, MessageType::PriorityUpdate),
            (0x33, MessageType::Deadline),
            (0x34, MessageType::ExpireAt),
            (0x35, MessageType::Supersede),
            (0x36, MessageType::BudgetUpdate),
            (0x37, MessageType::Progress),
            (0x38, MessageType::PartialResult),
            (0x39, MessageType::Backpressure),
            (0x3a, MessageType::CreditUpdate),
            (0x3b, MessageType::CapabilityNegotiation),
            (0x3c, MessageType::DegradeProfile),
            (0x3d, MessageType::RouteHint),
            (0x3e, MessageType::ExecutionHint),
            (0x3f, MessageType::TraceContext),
            (0x40, MessageType::ResultDropReason),
            (0x48, MessageType::ErrorRecoverable),
            (0x49, MessageType::RetryAfter),
        ];

        for (wire_value, message_type) in assignments {
            assert_eq!(MessageType::try_from_u8(wire_value), Ok(message_type));
            assert_eq!(message_type as u8, wire_value);
        }
    }

    #[test]
    fn message_type_rejects_unknown_values() {
        assert_eq!(
            MessageType::try_from_u8(0xff),
            Err(NnrpError::UnknownMessageType(0xff))
        );
    }

    #[test]
    fn header_flags_accept_known_bits_and_reject_reserved_bits() {
        let all_known = HeaderFlags(
            HeaderFlags::NONE.0
                | HeaderFlags::ACK_REQUIRED.0
                | HeaderFlags::CAN_DROP.0
                | HeaderFlags::STALE.0
                | HeaderFlags::EOS.0
                | HeaderFlags::RETRANSMIT.0
                | HeaderFlags::KEYFRAME.0,
        );

        assert_eq!(HeaderFlags::KNOWN_MASK, 0x0000_003f);
        assert_eq!(all_known.validate_known(), Ok(()));
        assert_eq!(
            HeaderFlags(0x0000_0040).validate_known(),
            Err(NnrpError::ReservedBitsSet {
                value: 0x0000_0040,
                allowed: 0x0000_003f
            })
        );
    }

    #[test]
    fn preview3_session_and_operation_enums_are_frozen() {
        assert_enum_u8(
            "session_priority_class",
            SessionPriorityClass::try_from_u8,
            0,
            2,
        );
        assert_eq!(
            SessionPriorityClass::try_from_u8(3),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "session_priority_class",
                value: 3
            })
        );

        assert_enum_u8("session_status", SessionStatus::try_from_u8, 0, 3);
        assert_enum_u16("close_reason", SessionCloseReason::try_from_u16, 0, 5);
        assert_enum_u8("in_flight_policy", InFlightPolicy::try_from_u8, 0, 1);
        assert_enum_u8("close_status", SessionCloseStatus::try_from_u8, 0, 3);
        assert_enum_u8("operation_state", OperationState::try_from_u8, 0, 7);
        assert_enum_u8("cancel_scope", CancelScope::try_from_u8, 0, 3);
    }

    #[test]
    fn preview3_flow_enums_are_frozen() {
        assert_enum_u8("scope_kind", FlowScopeKind::try_from_u8, 0, 2);
        assert_enum_u8("update_reason", FlowUpdateReason::try_from_u8, 0, 4);
        assert_enum_u8("backpressure_level", BackpressureLevel::try_from_u8, 0, 2);
    }

    #[test]
    fn operation_state_terminal_and_transition_rules_are_stable() {
        assert!(!OperationState::Accepted.is_terminal());
        assert!(OperationState::Completed.is_terminal());
        assert!(OperationState::Running.can_transition_to(OperationState::Partial));
        assert!(OperationState::Partial.can_transition_to(OperationState::Completed));
        assert!(OperationState::WaitingTool.can_transition_to(OperationState::Running));
        assert!(!OperationState::Completed.can_transition_to(OperationState::Running));
        assert!(!OperationState::Accepted.can_transition_to(OperationState::Partial));
    }

    fn assert_enum_u8<T: Copy>(
        enum_name: &'static str,
        parse: fn(u8) -> Result<T, NnrpError>,
        first: u8,
        last: u8,
    ) {
        for value in first..=last {
            assert!(
                parse(value).is_ok(),
                "{enum_name} value {value} should parse"
            );
        }
        assert_eq!(
            parse(last + 1).map(|_| ()),
            Err(NnrpError::UnknownEnumValue {
                enum_name,
                value: (last + 1) as u64
            })
        );
    }

    fn assert_enum_u16<T: Copy>(
        enum_name: &'static str,
        parse: fn(u16) -> Result<T, NnrpError>,
        first: u16,
        last: u16,
    ) {
        for value in first..=last {
            assert!(
                parse(value).is_ok(),
                "{enum_name} value {value} should parse"
            );
        }
        assert_eq!(
            parse(last + 1).map(|_| ()),
            Err(NnrpError::UnknownEnumValue {
                enum_name,
                value: (last + 1) as u64
            })
        );
    }
}
