use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NnrpError {
    #[error("unsupported wire format: {0}")]
    UnsupportedWireFormat(u8),

    #[error("source is too short: expected at least {expected} bytes, got {actual}")]
    SourceTooShort { expected: usize, actual: usize },

    #[error("destination is too short: expected at least {expected} bytes, got {actual}")]
    DestinationTooShort { expected: usize, actual: usize },

    #[error("invalid packet magic")]
    InvalidMagic,

    #[error("invalid header length: {0}")]
    InvalidHeaderLength(u8),

    #[error("unsupported version major: {0}")]
    UnsupportedVersionMajor(u8),

    #[error("unknown message type: {0:#04x}")]
    UnknownMessageType(u8),

    #[error("unknown {enum_name} value: {value:#x}")]
    UnknownEnumValue { enum_name: &'static str, value: u64 },

    #[error("reserved bits are set: value {value:#x}, allowed mask {allowed:#x}")]
    ReservedBitsSet { value: u64, allowed: u64 },

    #[error("invalid protocol combination: {rule}")]
    InvalidProtocolCombination { rule: &'static str },

    #[error("connection is not open")]
    ConnectionNotOpen,

    #[error("connection is already closed")]
    ConnectionAlreadyClosed,

    #[error("session already exists: {0}")]
    SessionAlreadyExists(u32),

    #[error("session is unknown: {0}")]
    UnknownSession(u32),

    #[error("session is not open: {0}")]
    SessionNotOpen(u32),

    #[error("operation already exists: {0}")]
    OperationAlreadyExists(u64),

    #[error("operation is unknown: {0}")]
    UnknownOperation(u64),

    #[error("invalid operation relationship: {rule}")]
    InvalidOperationRelationship { rule: &'static str },

    #[error("invalid operation transition from {from:?} to {to:?}")]
    InvalidOperationTransition {
        from: crate::OperationState,
        to: crate::OperationState,
    },

    #[error("reserved field is non-zero: {field}")]
    NonZeroReservedField { field: &'static str },

    #[error(
        "declared packet length does not match actual length: declared {declared}, actual {actual}"
    )]
    PacketLengthMismatch { declared: usize, actual: usize },

    #[error("message length overflow")]
    MessageLengthOverflow,
}
