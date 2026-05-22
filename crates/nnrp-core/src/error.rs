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

    #[error("reserved bits are set: value {value:#x}, allowed mask {allowed:#x}")]
    ReservedBitsSet { value: u64, allowed: u64 },

    #[error("reserved field is non-zero: {field}")]
    NonZeroReservedField { field: &'static str },

    #[error(
        "declared packet length does not match actual length: declared {declared}, actual {actual}"
    )]
    PacketLengthMismatch { declared: usize, actual: usize },

    #[error("message length overflow")]
    MessageLengthOverflow,
}
