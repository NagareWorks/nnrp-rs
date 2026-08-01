use std::{cmp::Ordering, collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use nnrp_core::{TransportId, TransportPolicy};
use nnrp_transport_provider::{TransportProviderDescriptor, TransportRejectionReason};

use crate::{
    BoxedFramedListener, NnrpEndpoint, NnrpServerConfig, ProviderEndpoint, RuntimeError,
    RuntimeFrameLimits, ServerProviderRoute, ServerProviderRoutes, ServerTransportSecurity,
};

#[derive(Debug, Clone)]
pub struct NnrpServerOptions {
    pub endpoint: NnrpEndpoint,
    pub provider_routes: ServerProviderRoutes,
    pub transport_policy: TransportPolicy,
    pub session: NnrpServerConfig,
}

impl NnrpServerOptions {
    pub fn new(
        endpoint: NnrpEndpoint,
        provider_routes: ServerProviderRoutes,
        transport_policy: TransportPolicy,
        session_defaults: NnrpServerConfig,
    ) -> Self {
        Self {
            endpoint,
            provider_routes,
            transport_policy,
            session: session_defaults,
        }
    }
}

pub struct BoundServerProvider {
    transport_id: TransportId,
    provider_endpoint: Option<ProviderEndpoint>,
    local_addr: Option<std::net::SocketAddr>,
    listener: BoxedFramedListener,
}

impl BoundServerProvider {
    pub fn new(
        provider_endpoint: ProviderEndpoint,
        listener: BoxedFramedListener,
    ) -> Result<Self, RuntimeError> {
        let transport_id = listener.transport_kind().transport_id();
        if !provider_endpoint.matches_transport(transport_id) {
            return Err(RuntimeError::ServerRouteRejected {
                transport_id,
                reason: TransportRejectionReason::RouteUnresolved,
                diagnostic: "bound provider endpoint does not match listener transport".into(),
            });
        }
        let local_addr = listener.local_addr().ok();
        Ok(Self {
            transport_id,
            provider_endpoint: Some(provider_endpoint),
            local_addr,
            listener,
        })
    }

    pub(crate) fn from_listener(listener: BoxedFramedListener) -> Self {
        let transport_id = listener.transport_kind().transport_id();
        let local_addr = listener.local_addr().ok();
        let provider_endpoint = local_addr.and_then(|addr| {
            let scheme = match transport_id {
                TransportId::Tcp => "tcp",
                TransportId::Quic => "quic",
                _ => return None,
            };
            format!("{scheme}://{addr}").parse().ok()
        });
        Self {
            transport_id,
            provider_endpoint,
            local_addr,
            listener,
        }
    }

    pub fn transport_id(&self) -> TransportId {
        self.transport_id
    }

    pub fn provider_endpoint(&self) -> Option<&ProviderEndpoint> {
        self.provider_endpoint.as_ref()
    }

    pub fn local_addr(&self) -> Option<std::net::SocketAddr> {
        self.local_addr
    }

    pub(crate) fn listener(&self) -> &BoxedFramedListener {
        &self.listener
    }
}

impl std::fmt::Debug for BoundServerProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BoundServerProvider")
            .field("transport_id", &self.transport_id)
            .field("provider_endpoint", &self.provider_endpoint)
            .field("local_addr", &self.local_addr)
            .finish_non_exhaustive()
    }
}

#[async_trait]
pub trait NnrpServerProvider: Send + Sync {
    fn descriptor(&self) -> TransportProviderDescriptor;

    async fn bind(
        &self,
        endpoint: &ProviderEndpoint,
        security: Option<&ServerTransportSecurity>,
        limits: RuntimeFrameLimits,
    ) -> Result<BoundServerProvider, RuntimeError>;
}

pub(crate) async fn bind_server<I>(
    options: &NnrpServerOptions,
    providers: I,
) -> Result<Vec<BoundServerProvider>, RuntimeError>
where
    I: IntoIterator<Item = Arc<dyn NnrpServerProvider>>,
{
    let mut providers = providers.into_iter().collect::<Vec<_>>();
    reject_duplicate_providers(&providers)?;
    reject_uninstalled_routes(options, &providers)?;
    providers.retain(|provider| {
        options
            .transport_policy
            .allows(provider.descriptor().transport_id)
    });
    providers.sort_by(|left, right| compare_provider_order(options.transport_policy, left, right));

    let mut listeners = Vec::with_capacity(providers.len());
    for provider in providers {
        let descriptor = provider.descriptor();
        if !descriptor.available {
            if options.transport_policy.forced_transport() == Some(descriptor.transport_id) {
                return Err(route_rejected(
                    descriptor.transport_id,
                    TransportRejectionReason::LocalUnavailable,
                    descriptor
                        .diagnostic
                        .unwrap_or_else(|| "forced provider is locally unavailable".into()),
                ));
            }
            continue;
        }
        let route = options
            .provider_routes
            .get(&descriptor.transport_id)
            .cloned()
            .unwrap_or_default();
        let endpoint = resolve_server_endpoint(&options.endpoint, descriptor.transport_id, &route)?;
        let security = validate_server_security(
            &options.endpoint,
            &endpoint,
            route.security.as_ref(),
            descriptor.transport_id,
        )?;
        if let ServerSecurityEligibility::Ineligible(diagnostic) = security {
            if options.transport_policy.forced_transport() == Some(descriptor.transport_id) {
                return Err(route_rejected(
                    descriptor.transport_id,
                    TransportRejectionReason::SecurityUnsatisfied,
                    diagnostic,
                ));
            }
            continue;
        }
        let bound = provider
            .bind(
                &endpoint,
                route.security.as_ref(),
                RuntimeFrameLimits::default(),
            )
            .await?;
        if bound.transport_id() != descriptor.transport_id {
            return Err(route_rejected(
                descriptor.transport_id,
                TransportRejectionReason::RouteUnresolved,
                "provider returned a listener for a different transport",
            ));
        }
        listeners.push(bound);
    }

    if listeners.is_empty() {
        let transport_id = options
            .transport_policy
            .forced_transport()
            .unwrap_or(TransportId::Unspecified);
        return Err(route_rejected(
            transport_id,
            TransportRejectionReason::LocalUnavailable,
            "no policy-eligible server provider is locally available",
        ));
    }
    Ok(listeners)
}

fn reject_duplicate_providers(
    providers: &[Arc<dyn NnrpServerProvider>],
) -> Result<(), RuntimeError> {
    let mut transports = BTreeMap::new();
    let mut provider_ids = BTreeMap::new();
    for provider in providers {
        let descriptor = provider.descriptor();
        if transports.insert(descriptor.transport_id, ()).is_some() {
            return Err(RuntimeError::DuplicateServerTransportProvider(
                descriptor.transport_id,
            ));
        }
        if provider_ids
            .insert(descriptor.metadata.id.clone(), ())
            .is_some()
        {
            return Err(RuntimeError::DuplicateServerProviderId(
                descriptor.metadata.id,
            ));
        }
    }
    Ok(())
}

fn reject_uninstalled_routes(
    options: &NnrpServerOptions,
    providers: &[Arc<dyn NnrpServerProvider>],
) -> Result<(), RuntimeError> {
    for transport_id in options.provider_routes.keys().copied() {
        if !options.transport_policy.allows(transport_id) {
            continue;
        }
        if providers
            .iter()
            .all(|provider| provider.descriptor().transport_id != transport_id)
        {
            return Err(route_rejected(
                transport_id,
                TransportRejectionReason::LocalUnavailable,
                "configured server route has no installed provider",
            ));
        }
    }
    Ok(())
}

fn compare_provider_order(
    policy: TransportPolicy,
    left: &Arc<dyn NnrpServerProvider>,
    right: &Arc<dyn NnrpServerProvider>,
) -> Ordering {
    let left = left.descriptor();
    let right = right.descriptor();
    let preferred = policy.preferred_transport();
    (preferred != Some(left.transport_id))
        .cmp(&(preferred != Some(right.transport_id)))
        .then_with(|| {
            left.metadata
                .preference_rank
                .cmp(&right.metadata.preference_rank)
        })
        .then_with(|| (left.transport_id as u32).cmp(&(right.transport_id as u32)))
        .then_with(|| left.metadata.id.cmp(&right.metadata.id))
}

fn resolve_server_endpoint(
    application: &NnrpEndpoint,
    transport_id: TransportId,
    route: &ServerProviderRoute,
) -> Result<ProviderEndpoint, RuntimeError> {
    route.validate_for(transport_id).map_err(|error| {
        route_rejected(
            transport_id,
            TransportRejectionReason::RouteUnresolved,
            error.to_string(),
        )
    })?;
    if let Some(endpoint) = route.provider_endpoint.clone() {
        return Ok(endpoint);
    }
    if application.port().is_none() {
        return Err(route_rejected(
            transport_id,
            TransportRejectionReason::RouteUnresolved,
            "provider endpoint cannot be derived without an application port",
        ));
    }
    match transport_id {
        TransportId::Tcp => format!("tcp://{}", application.authority()),
        TransportId::Quic => format!("quic://{}", application.authority()),
        _ => {
            return Err(route_rejected(
                transport_id,
                TransportRejectionReason::RouteUnresolved,
                "IPC and WebSocket server providers require an explicit locator",
            ));
        }
    }
    .parse()
    .map_err(RuntimeError::from)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerSecurityEligibility {
    Eligible,
    Ineligible(&'static str),
}

fn validate_server_security(
    application: &NnrpEndpoint,
    endpoint: &ProviderEndpoint,
    security: Option<&ServerTransportSecurity>,
    transport_id: TransportId,
) -> Result<ServerSecurityEligibility, RuntimeError> {
    if let Some(security) = security {
        security.validate().map_err(|error| {
            route_rejected(
                transport_id,
                TransportRejectionReason::SecurityUnsatisfied,
                error.to_string(),
            )
        })?;
    }
    let ineligible = match transport_id {
        TransportId::Quic if security.is_none() => {
            Some("QUIC requires route-local server credentials")
        }
        TransportId::Tcp if application.is_secure() && security.is_none() => {
            Some("nnrps TCP requires route-local server credentials")
        }
        TransportId::Ipc if application.is_secure() => {
            Some("IPC does not satisfy nnrps in Preview4")
        }
        TransportId::WebSocket if application.is_secure() && !endpoint.is_secure() => {
            Some("nnrps WebSocket requires a wss provider endpoint")
        }
        TransportId::WebSocket if endpoint.is_secure() && security.is_none() => {
            Some("native WSS requires route-local server credentials")
        }
        _ => None,
    };
    if transport_id == TransportId::Ipc && security.is_some() {
        return Err(route_rejected(
            transport_id,
            TransportRejectionReason::SecurityUnsatisfied,
            "IPC does not accept transport security credentials",
        ));
    }
    if transport_id == TransportId::WebSocket && !endpoint.is_secure() && security.is_some() {
        return Err(route_rejected(
            transport_id,
            TransportRejectionReason::SecurityUnsatisfied,
            "plain WebSocket does not accept transport security credentials",
        ));
    }
    Ok(match ineligible {
        Some(diagnostic) => ServerSecurityEligibility::Ineligible(diagnostic),
        None => ServerSecurityEligibility::Eligible,
    })
}

fn route_rejected(
    transport_id: TransportId,
    reason: TransportRejectionReason,
    diagnostic: impl Into<String>,
) -> RuntimeError {
    RuntimeError::ServerRouteRejected {
        transport_id,
        reason,
        diagnostic: diagnostic.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    use super::*;
    use crate::{BoxedFramedTransport, FramedListener, RuntimeTransportKind};

    #[derive(Clone)]
    struct ScriptedProvider {
        descriptor: TransportProviderDescriptor,
        fail_bind: bool,
        fail_accept: bool,
        bind_calls: Arc<AtomicUsize>,
        listener_drops: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl NnrpServerProvider for ScriptedProvider {
        fn descriptor(&self) -> TransportProviderDescriptor {
            self.descriptor.clone()
        }

        async fn bind(
            &self,
            endpoint: &ProviderEndpoint,
            _security: Option<&ServerTransportSecurity>,
            _limits: RuntimeFrameLimits,
        ) -> Result<BoundServerProvider, RuntimeError> {
            self.bind_calls.fetch_add(1, AtomicOrdering::SeqCst);
            if self.fail_bind {
                return Err(std::io::Error::other("scripted bind failure").into());
            }
            BoundServerProvider::new(
                endpoint.clone(),
                Box::new(ScriptedListener {
                    transport: RuntimeTransportKind::from_transport_id(
                        self.descriptor.transport_id,
                    )
                    .unwrap(),
                    fail_accept: self.fail_accept,
                    drops: Arc::clone(&self.listener_drops),
                }),
            )
        }
    }

    struct ScriptedListener {
        transport: RuntimeTransportKind,
        fail_accept: bool,
        drops: Arc<AtomicUsize>,
    }

    impl Drop for ScriptedListener {
        fn drop(&mut self) {
            self.drops.fetch_add(1, AtomicOrdering::SeqCst);
        }
    }

    #[async_trait]
    impl FramedListener for ScriptedListener {
        fn transport_kind(&self) -> RuntimeTransportKind {
            self.transport
        }

        fn local_addr(&self) -> Result<std::net::SocketAddr, RuntimeError> {
            Ok("127.0.0.1:4500".parse().unwrap())
        }

        async fn accept(&self) -> Result<BoxedFramedTransport, RuntimeError> {
            if self.fail_accept {
                return Err(std::io::Error::other("scripted accept failure").into());
            }
            std::future::pending().await
        }
    }

    fn provider(transport_id: TransportId, fail_bind: bool) -> ScriptedProvider {
        ScriptedProvider {
            descriptor: TransportProviderDescriptor::available(
                format!("test-{transport_id:?}"),
                "1",
                transport_id,
                nnrp_transport_provider::TransportProviderKind::PureRust,
            ),
            fail_bind,
            fail_accept: false,
            bind_calls: Arc::new(AtomicUsize::new(0)),
            listener_drops: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn options(routes: ServerProviderRoutes) -> NnrpServerOptions {
        NnrpServerOptions::new(
            "nnrp://127.0.0.1:4500/session".parse().unwrap(),
            routes,
            TransportPolicy::Auto,
            NnrpServerConfig::default(),
        )
    }

    #[tokio::test]
    async fn failed_bind_drops_every_listener_opened_by_the_operation() {
        let ipc = provider(TransportId::Ipc, false);
        let drops = Arc::clone(&ipc.listener_drops);
        let tcp = provider(TransportId::Tcp, true);
        let routes = ServerProviderRoutes::from([(
            TransportId::Ipc,
            ServerProviderRoute::at("npipe://nnrp-server-rollback".parse().unwrap()),
        )]);
        let result = bind_server(
            &options(routes),
            [
                Arc::new(tcp) as Arc<dyn NnrpServerProvider>,
                Arc::new(ipc) as Arc<dyn NnrpServerProvider>,
            ],
        )
        .await;
        assert!(matches!(result, Err(RuntimeError::Io(_))));
        assert_eq!(drops.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn configured_uninstalled_route_reports_local_unavailable() {
        let routes = ServerProviderRoutes::from([(
            TransportId::Ipc,
            ServerProviderRoute::at("npipe://nnrp-uninstalled".parse().unwrap()),
        )]);
        let error = bind_server(
            &options(routes),
            [Arc::new(provider(TransportId::Tcp, false)) as Arc<dyn NnrpServerProvider>],
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::ServerRouteRejected {
                transport_id: TransportId::Ipc,
                reason: TransportRejectionReason::LocalUnavailable,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn duplicate_server_provider_ids_are_rejected_before_binding() {
        let tcp = provider(TransportId::Tcp, false);
        let mut ipc = provider(TransportId::Ipc, false);
        ipc.descriptor.metadata.id = tcp.descriptor.metadata.id.clone();
        let error = bind_server(
            &options(ServerProviderRoutes::new()),
            [
                Arc::new(tcp) as Arc<dyn NnrpServerProvider>,
                Arc::new(ipc) as Arc<dyn NnrpServerProvider>,
            ],
        )
        .await
        .unwrap_err();
        assert!(matches!(error, RuntimeError::DuplicateServerProviderId(_)));
    }

    #[tokio::test]
    async fn duplicate_server_transports_are_rejected_before_binding() {
        let first = provider(TransportId::Tcp, false);
        let second = provider(TransportId::Tcp, false);
        let error = bind_server(
            &options(ServerProviderRoutes::new()),
            [
                Arc::new(first) as Arc<dyn NnrpServerProvider>,
                Arc::new(second) as Arc<dyn NnrpServerProvider>,
            ],
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::DuplicateServerTransportProvider(TransportId::Tcp)
        ));
    }

    #[tokio::test]
    async fn auto_skips_security_ineligible_routes_and_binds_eligible_routes() {
        let ipc = provider(TransportId::Ipc, false);
        let ipc_bind_calls = Arc::clone(&ipc.bind_calls);
        let tcp = provider(TransportId::Tcp, false);
        let tcp_bind_calls = Arc::clone(&tcp.bind_calls);
        let mut options = options(ServerProviderRoutes::from([
            (
                TransportId::Ipc,
                ServerProviderRoute::at("npipe://nnrp-secure-auto".parse().unwrap()),
            ),
            (
                TransportId::Tcp,
                ServerProviderRoute {
                    provider_endpoint: None,
                    security: Some(ServerTransportSecurity::new([1], [2])),
                },
            ),
        ]));
        options.endpoint = "nnrps://127.0.0.1:4500/session".parse().unwrap();

        let listeners = bind_server(
            &options,
            [
                Arc::new(ipc) as Arc<dyn NnrpServerProvider>,
                Arc::new(tcp) as Arc<dyn NnrpServerProvider>,
            ],
        )
        .await
        .unwrap();

        assert_eq!(
            listeners
                .iter()
                .map(BoundServerProvider::transport_id)
                .collect::<Vec<_>>(),
            [TransportId::Tcp]
        );
        assert_eq!(ipc_bind_calls.load(AtomicOrdering::SeqCst), 0);
        assert_eq!(tcp_bind_calls.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn force_rejects_a_security_ineligible_route_without_binding() {
        let ipc = provider(TransportId::Ipc, false);
        let bind_calls = Arc::clone(&ipc.bind_calls);
        let mut options = options(ServerProviderRoutes::from([(
            TransportId::Ipc,
            ServerProviderRoute::at("npipe://nnrp-secure-force".parse().unwrap()),
        )]));
        options.endpoint = "nnrps://127.0.0.1:4500/session".parse().unwrap();
        options.transport_policy = TransportPolicy::ForceIpc;

        let error = bind_server(&options, [Arc::new(ipc) as Arc<dyn NnrpServerProvider>])
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::ServerRouteRejected {
                transport_id: TransportId::Ipc,
                reason: TransportRejectionReason::SecurityUnsatisfied,
                ..
            }
        ));
        assert_eq!(bind_calls.load(AtomicOrdering::SeqCst), 0);
    }

    #[tokio::test]
    async fn force_binds_only_the_named_transport() {
        let ipc = provider(TransportId::Ipc, false);
        let ipc_bind_calls = Arc::clone(&ipc.bind_calls);
        let tcp = provider(TransportId::Tcp, false);
        let tcp_bind_calls = Arc::clone(&tcp.bind_calls);
        let mut options = options(ServerProviderRoutes::from([(
            TransportId::Ipc,
            ServerProviderRoute::at("npipe://nnrp-force-tcp".parse().unwrap()),
        )]));
        options.transport_policy = TransportPolicy::ForceTcp;

        let listeners = bind_server(
            &options,
            [
                Arc::new(ipc) as Arc<dyn NnrpServerProvider>,
                Arc::new(tcp) as Arc<dyn NnrpServerProvider>,
            ],
        )
        .await
        .unwrap();

        assert_eq!(listeners.len(), 1);
        assert_eq!(listeners[0].transport_id(), TransportId::Tcp);
        assert_eq!(ipc_bind_calls.load(AtomicOrdering::SeqCst), 0);
        assert_eq!(tcp_bind_calls.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn prefer_policy_places_the_preferred_listener_first() {
        let ipc = provider(TransportId::Ipc, false);
        let tcp = provider(TransportId::Tcp, false);
        let mut options = options(ServerProviderRoutes::from([(
            TransportId::Ipc,
            ServerProviderRoute::at("npipe://nnrp-prefer-tcp".parse().unwrap()),
        )]));
        options.transport_policy = TransportPolicy::PreferTcp;

        let listeners = bind_server(
            &options,
            [
                Arc::new(ipc) as Arc<dyn NnrpServerProvider>,
                Arc::new(tcp) as Arc<dyn NnrpServerProvider>,
            ],
        )
        .await
        .unwrap();

        assert_eq!(
            listeners
                .iter()
                .map(BoundServerProvider::transport_id)
                .collect::<Vec<_>>(),
            [TransportId::Tcp, TransportId::Ipc]
        );
    }

    #[tokio::test]
    async fn terminal_listener_failure_closes_the_logical_server() {
        let mut tcp = provider(TransportId::Tcp, false);
        tcp.fail_accept = true;
        let server = crate::NnrpServer::listen(
            options(ServerProviderRoutes::new()),
            [Arc::new(tcp) as Arc<dyn NnrpServerProvider>],
        )
        .await
        .unwrap();

        assert!(matches!(server.accept().await, Err(RuntimeError::Io(_))));
        assert!(matches!(
            server.accept().await,
            Err(RuntimeError::ServerListenerSetClosed)
        ));
    }

    #[test]
    fn bound_listener_constructor_preserves_the_provider_endpoint() {
        let endpoint: ProviderEndpoint = "tcp://127.0.0.1:4500".parse().unwrap();
        let server = crate::NnrpServer::from_bound_listener(
            endpoint.clone(),
            ScriptedListener {
                transport: RuntimeTransportKind::Tcp,
                fail_accept: false,
                drops: Arc::new(AtomicUsize::new(0)),
            },
            NnrpServerConfig::default(),
        )
        .unwrap();

        assert_eq!(
            server.bound_provider_endpoints().get(&TransportId::Tcp),
            Some(&endpoint)
        );
    }

    #[test]
    fn websocket_listener_requires_an_explicit_path_bearing_endpoint() {
        let inferred = crate::NnrpServer::from_listener(
            ScriptedListener {
                transport: RuntimeTransportKind::WebSocket,
                fail_accept: false,
                drops: Arc::new(AtomicUsize::new(0)),
            },
            NnrpServerConfig::default(),
        )
        .unwrap();
        assert!(inferred.bound_provider_endpoints().is_empty());
        assert_eq!(
            inferred.local_addr().unwrap(),
            "127.0.0.1:4500".parse().unwrap()
        );

        let endpoint: ProviderEndpoint = "wss://127.0.0.1:4500/nnrp".parse().unwrap();
        let explicit = crate::NnrpServer::from_bound_listener(
            endpoint.clone(),
            ScriptedListener {
                transport: RuntimeTransportKind::WebSocket,
                fail_accept: false,
                drops: Arc::new(AtomicUsize::new(0)),
            },
            NnrpServerConfig::default(),
        )
        .unwrap();
        assert_eq!(
            explicit
                .bound_provider_endpoints()
                .get(&TransportId::WebSocket),
            Some(&endpoint)
        );
    }

    #[test]
    fn bound_provider_rejects_a_locator_for_another_transport() {
        let error = BoundServerProvider::new(
            "npipe://nnrp-mismatched-listener".parse().unwrap(),
            Box::new(ScriptedListener {
                transport: RuntimeTransportKind::Tcp,
                fail_accept: false,
                drops: Arc::new(AtomicUsize::new(0)),
            }),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::ServerRouteRejected {
                transport_id: TransportId::Tcp,
                reason: TransportRejectionReason::RouteUnresolved,
                ..
            }
        ));
    }

    #[test]
    fn server_security_matrix_matches_the_frozen_route_contract() {
        let plain: NnrpEndpoint = "nnrp://localhost:443/session".parse().unwrap();
        let secure: NnrpEndpoint = "nnrps://localhost:443/session".parse().unwrap();
        let tcp: ProviderEndpoint = "tcp://127.0.0.1:443".parse().unwrap();
        let quic: ProviderEndpoint = "quic://127.0.0.1:443".parse().unwrap();
        let ipc: ProviderEndpoint = "npipe://nnrp".parse().unwrap();
        let ws: ProviderEndpoint = "ws://127.0.0.1:443/nnrp".parse().unwrap();
        let wss: ProviderEndpoint = "wss://127.0.0.1:443/nnrp".parse().unwrap();
        let security = ServerTransportSecurity::new([1], [2]);

        assert_eq!(
            validate_server_security(&plain, &tcp, None, TransportId::Tcp).unwrap(),
            ServerSecurityEligibility::Eligible
        );
        assert_eq!(
            validate_server_security(&plain, &tcp, Some(&security), TransportId::Tcp).unwrap(),
            ServerSecurityEligibility::Eligible
        );
        assert!(matches!(
            validate_server_security(&secure, &tcp, None, TransportId::Tcp).unwrap(),
            ServerSecurityEligibility::Ineligible(_)
        ));
        assert!(matches!(
            validate_server_security(&plain, &quic, None, TransportId::Quic).unwrap(),
            ServerSecurityEligibility::Ineligible(_)
        ));
        assert_eq!(
            validate_server_security(&plain, &quic, Some(&security), TransportId::Quic).unwrap(),
            ServerSecurityEligibility::Eligible
        );
        assert!(matches!(
            validate_server_security(&secure, &ipc, None, TransportId::Ipc).unwrap(),
            ServerSecurityEligibility::Ineligible(_)
        ));
        assert!(validate_server_security(&plain, &ipc, Some(&security), TransportId::Ipc).is_err());
        assert!(
            validate_server_security(&plain, &ws, Some(&security), TransportId::WebSocket).is_err()
        );
        assert!(matches!(
            validate_server_security(&secure, &ws, None, TransportId::WebSocket).unwrap(),
            ServerSecurityEligibility::Ineligible(_)
        ));
        assert!(matches!(
            validate_server_security(&plain, &wss, None, TransportId::WebSocket).unwrap(),
            ServerSecurityEligibility::Ineligible(_)
        ));
        assert_eq!(
            validate_server_security(&plain, &wss, Some(&security), TransportId::WebSocket)
                .unwrap(),
            ServerSecurityEligibility::Eligible
        );
    }
}
