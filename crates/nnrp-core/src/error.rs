use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NnrpError {
    #[error("unsupported wire format: {0}")]
    UnsupportedWireFormat(u8),
}
