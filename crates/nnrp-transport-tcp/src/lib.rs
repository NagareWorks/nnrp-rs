use nnrp_core::TransportId;
use nnrp_runtime::{RuntimeError, TcpFramedListener, TcpTransport};
use nnrp_transport_provider::{
    TransportProviderDescriptor, TransportProviderKind, TransportProviderRegistry,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct TcpProvider;

impl TcpProvider {
    pub const NAME: &'static str = "nnrp-transport-tcp";

    pub fn descriptor() -> TransportProviderDescriptor {
        TransportProviderDescriptor::available(
            Self::NAME,
            env!("CARGO_PKG_VERSION"),
            TransportId::Tcp,
            TransportProviderKind::PureRust,
        )
    }

    pub fn register(registry: &mut TransportProviderRegistry) {
        registry.register(Self::descriptor());
    }

    pub async fn connect(
        addr: impl tokio::net::ToSocketAddrs,
    ) -> Result<TcpTransport, RuntimeError> {
        TcpTransport::connect(addr).await
    }

    pub async fn bind(
        addr: impl tokio::net::ToSocketAddrs,
    ) -> Result<TcpFramedListener, RuntimeError> {
        TcpFramedListener::bind(addr).await
    }
}

pub fn register_tcp_provider(registry: &mut TransportProviderRegistry) {
    TcpProvider::register(registry);
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnrp_transport_provider::{RemoteTransportSupport, TransportPolicy};

    #[test]
    fn tcp_provider_registers_available_descriptor() {
        let mut registry = TransportProviderRegistry::new();
        register_tcp_provider(&mut registry);
        assert_eq!(registry.providers().len(), 1);
        assert_eq!(registry.providers()[0].name, TcpProvider::NAME);
        assert_eq!(registry.providers()[0].transport_id, TransportId::Tcp);
        assert!(registry.providers()[0].available);
    }

    #[test]
    fn tcp_provider_participates_in_policy_selection() {
        let registry = TransportProviderRegistry::new().with_provider(TcpProvider::descriptor());
        let remote = RemoteTransportSupport::new([TransportId::Tcp]);
        let selection = registry
            .select(&remote, TransportPolicy::ForceTcp, None)
            .expect("tcp provider should satisfy force tcp");
        assert_eq!(selection.selected.name, TcpProvider::NAME);
    }
}
