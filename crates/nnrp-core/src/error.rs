use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NnrpError {
    #[error("unsupported preview stage: {0}")]
    UnsupportedPreviewStage(u8),
}
