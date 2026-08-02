use thiserror::Error;

use crate::transport::RuntimeTransportKind;
use nnrp_core::TransportId;
use nnrp_transport_provider::TransportRejectionReason;

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

    #[error("more than one server provider uses transport {0:?}")]
    DuplicateServerTransportProvider(TransportId),

    #[error("more than one client provider uses id {0}")]
    DuplicateClientProviderId(String),

    #[error("more than one server provider uses id {0}")]
    DuplicateServerProviderId(String),

    #[error("server route {transport_id:?} was rejected as {reason:?}: {diagnostic}")]
    ServerRouteRejected {
        transport_id: TransportId,
        reason: TransportRejectionReason,
        diagnostic: String,
    },

    #[error("no server provider listener remains available")]
    ServerListenerSetClosed,

    #[error("server accept timed out")]
    ServerAcceptTimeout,

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

    #[error("invalid session recovery ticket: {0}")]
    InvalidRecoveryTicket(&'static str),

    #[error("session open rejected with code {code}: {diagnostic}")]
    SessionRejected { code: u32, diagnostic: String },

    #[error("runtime internal error: {0}")]
    Internal(&'static str),
}
