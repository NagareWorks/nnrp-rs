use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("transport I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("protocol validation failed: {0}")]
    Protocol(#[from] nnrp_core::NnrpError),

    #[error("unsupported transport: {0}")]
    UnsupportedTransport(&'static str),

    #[error("unexpected runtime message: {0}")]
    UnexpectedMessage(&'static str),
}
