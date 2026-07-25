use thiserror::Error;

use crate::transport::RuntimeTransportKind;
use nnrp_core::TransportId;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("transport I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("protocol validation failed: {0}")]
    Protocol(#[from] nnrp_core::NnrpError),

    #[error("route configuration failed: {0}")]
    RouteConfiguration(#[from] crate::RouteConfigurationError),

    #[error("transport selection failed: {0}")]
    TransportSelection(#[from] nnrp_transport_provider::TransportSelectionError),

    #[error("more than one client provider uses transport {0:?}")]
    DuplicateTransportProvider(TransportId),

    #[error("more than one client provider uses id {0}")]
    DuplicateClientProviderId(String),

    #[error("selected client provider is unavailable: {0}")]
    SelectedProviderUnavailable(String),

    #[error("unsupported transport: {0}")]
    UnsupportedTransport(&'static str),

    #[error("frame id overflowed")]
    FrameIdOverflow,

    #[error("runtime frame too large: declared {declared} bytes exceeds max {max} bytes")]
    FrameTooLarge { declared: usize, max: usize },

    #[error("runtime transport {transport:?} closed: {detail}")]
    TransportClosed {
        transport: RuntimeTransportKind,
        detail: String,
    },

    #[error("unexpected runtime message: {0}")]
    UnexpectedMessage(&'static str),

    #[error("runtime internal error: {0}")]
    Internal(&'static str),
}
