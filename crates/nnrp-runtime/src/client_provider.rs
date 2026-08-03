use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use nnrp_core::{
    CommonHeader, MessageType, TransportId, TransportPolicy, TransportProbeAckMetadata,
    TransportProbeMetadata, TRANSPORT_PROBE_ACK_METADATA_LEN,
};
use nnrp_transport_provider::{
    select_transport, select_transport_with_probe, summarize_provider_probe, ProbeSample,
    ProbeState, TransportCandidateReadiness, TransportProbeObservation,
    TransportProviderDescriptor, TransportProviderKind, TransportRejectionReason,
    TransportSelection, TransportSelectionError, TransportSelectionOptions,
};

use crate::{
    BoxedFramedTransport, ClientProviderRoute, ClientProviderRoutes, ClientTransportSecurity,
    NnrpClientConfig, NnrpEndpoint, ProviderEndpoint, RouteConfigurationError, RuntimeError,
    RuntimeFrameLimits, RuntimePacket,
};

const PROBE_SAMPLE_COUNT: u32 = 3;
const PROBE_PAYLOAD_BYTES: u32 = 32 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpClientOptions {
    pub endpoint: NnrpEndpoint,
    pub provider_routes: ClientProviderRoutes,
    pub transport_policy: TransportPolicy,
    pub session_defaults: NnrpClientConfig,
}

impl NnrpClientOptions {
    pub fn new(
        endpoint: NnrpEndpoint,
        provider_routes: ClientProviderRoutes,
        transport_policy: TransportPolicy,
        session_defaults: NnrpClientConfig,
    ) -> Self {
        Self {
            endpoint,
            provider_routes,
            transport_policy,
            session_defaults,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg(target_arch = "wasm32")]
pub trait NnrpClientProvider {
    fn descriptor(&self) -> TransportProviderDescriptor;

    async fn connect(
        &self,
        endpoint: &ProviderEndpoint,
        security: Option<&ClientTransportSecurity>,
        limits: RuntimeFrameLimits,
    ) -> Result<BoxedFramedTransport, RuntimeError>;
}

#[async_trait]
#[cfg(not(target_arch = "wasm32"))]
pub trait NnrpClientProvider: Send + Sync {
    fn descriptor(&self) -> TransportProviderDescriptor;

    async fn connect(
        &self,
        endpoint: &ProviderEndpoint,
        security: Option<&ClientTransportSecurity>,
        limits: RuntimeFrameLimits,
    ) -> Result<BoxedFramedTransport, RuntimeError>;
}

#[derive(Debug, Clone)]
struct ResolvedClientRoute {
    endpoint: ProviderEndpoint,
    security: Option<ClientTransportSecurity>,
}

pub(crate) async fn connect_client<I>(
    options: NnrpClientOptions,
    providers: I,
) -> Result<(BoxedFramedTransport, NnrpClientConfig, TransportSelection), RuntimeError>
where
    I: IntoIterator<Item = Arc<dyn NnrpClientProvider>>,
{
    let providers = providers.into_iter().collect::<Vec<_>>();
    reject_duplicate_transports(&providers)?;

    let mut descriptors = providers
        .iter()
        .map(|provider| provider.descriptor())
        .collect::<Vec<_>>();
    add_uninstalled_route_descriptors(&mut descriptors, &options.provider_routes);

    let mut resolved = BTreeMap::new();
    let readiness = descriptors
        .iter()
        .map(|descriptor| {
            resolve_candidate(
                &options.endpoint,
                options.provider_routes.get(&descriptor.transport_id),
                descriptor,
            )
            .map(|route| {
                resolved.insert(descriptor.metadata.id.clone(), route);
                TransportCandidateReadiness::ready(descriptor.transport_id, &descriptor.metadata.id)
            })
            .unwrap_or_else(|failure| failure)
        })
        .collect::<Vec<_>>();
    let mut selection_options = TransportSelectionOptions {
        peer_supported_transports: descriptors
            .iter()
            .map(|descriptor| descriptor.transport_id)
            .collect(),
        policy: options.transport_policy,
        requested_max_frame_bytes: Some(RuntimeFrameLimits::DEFAULT_MAX_PACKET_BYTES as u64),
        candidate_readiness: readiness,
        probe_observations: Vec::new(),
    };

    let initial = select_transport(&descriptors, &selection_options);
    let selection = match initial {
        Ok(selection) => selection,
        Err(error) if requires_probe(&error) => {
            let observations = probe_candidates(&providers, &resolved, &error).await;
            selection_options.probe_observations = observations;
            select_transport_with_probe(&descriptors, &selection_options)?
        }
        Err(error) => return Err(error.into()),
    };

    let provider = providers
        .iter()
        .find(|provider| {
            provider.descriptor().metadata.id == selection.selected_provider.metadata.id
        })
        .ok_or_else(|| {
            RuntimeError::SelectedProviderUnavailable(
                selection.selected_provider.metadata.id.clone(),
            )
        })?;
    let route = resolved
        .get(&selection.selected_provider.metadata.id)
        .ok_or_else(|| {
            RuntimeError::SelectedProviderUnavailable(
                selection.selected_provider.metadata.id.clone(),
            )
        })?;
    let transport = provider
        .connect(
            &route.endpoint,
            route.security.as_ref(),
            RuntimeFrameLimits::default(),
        )
        .await?;
    Ok((transport, options.session_defaults, selection))
}

fn reject_duplicate_transports(
    providers: &[Arc<dyn NnrpClientProvider>],
) -> Result<(), RuntimeError> {
    let mut transports = BTreeMap::new();
    let mut provider_ids = BTreeMap::new();
    for provider in providers {
        let descriptor = provider.descriptor();
        let transport_id = descriptor.transport_id;
        if transports.insert(transport_id, ()).is_some() {
            return Err(RuntimeError::DuplicateTransportProvider(transport_id));
        }
        if provider_ids
            .insert(descriptor.metadata.id.clone(), ())
            .is_some()
        {
            return Err(RuntimeError::DuplicateClientProviderId(
                descriptor.metadata.id,
            ));
        }
    }
    Ok(())
}

fn add_uninstalled_route_descriptors(
    descriptors: &mut Vec<TransportProviderDescriptor>,
    routes: &ClientProviderRoutes,
) {
    for transport_id in routes.keys().copied() {
        if descriptors
            .iter()
            .any(|descriptor| descriptor.transport_id == transport_id)
        {
            continue;
        }
        descriptors.push(TransportProviderDescriptor::missing(
            "uninstalled-client-provider",
            env!("CARGO_PKG_VERSION"),
            transport_id,
            TransportProviderKind::PureRust,
            "configured route has no installed provider",
        ));
    }
}

fn resolve_candidate(
    application: &NnrpEndpoint,
    route: Option<&ClientProviderRoute>,
    descriptor: &TransportProviderDescriptor,
) -> Result<ResolvedClientRoute, TransportCandidateReadiness> {
    if !descriptor.available {
        return Err(TransportCandidateReadiness::ready(
            descriptor.transport_id,
            &descriptor.metadata.id,
        ));
    }
    let route = route.cloned().unwrap_or_default();
    if let Some(endpoint) = &route.provider_endpoint {
        if !endpoint.matches_transport(descriptor.transport_id) {
            return Err(TransportCandidateReadiness::route_unresolved(
                descriptor.transport_id,
                &descriptor.metadata.id,
                "provider endpoint scheme does not match the provider transport",
            ));
        }
    }
    if let Some(security) = &route.security {
        if let Err(error) = security.validate() {
            return Err(TransportCandidateReadiness::security_unsatisfied(
                descriptor.transport_id,
                &descriptor.metadata.id,
                error.to_string(),
            ));
        }
    }
    let endpoint = match route.provider_endpoint.clone() {
        Some(endpoint) => endpoint,
        None => {
            derive_provider_endpoint(application, descriptor.transport_id).map_err(|error| {
                TransportCandidateReadiness::route_unresolved(
                    descriptor.transport_id,
                    &descriptor.metadata.id,
                    error.to_string(),
                )
            })?
        }
    };
    if let Err(diagnostic) =
        validate_security(application, &endpoint, route.security.as_ref(), descriptor)
    {
        return Err(TransportCandidateReadiness::security_unsatisfied(
            descriptor.transport_id,
            &descriptor.metadata.id,
            diagnostic,
        ));
    }
    Ok(ResolvedClientRoute {
        endpoint,
        security: route.security,
    })
}

fn derive_provider_endpoint(
    application: &NnrpEndpoint,
    transport_id: TransportId,
) -> Result<ProviderEndpoint, RouteConfigurationError> {
    if application.port().is_none() {
        return Err(RouteConfigurationError::EmptyProviderLocator);
    }
    match transport_id {
        TransportId::Tcp => format!("tcp://{}", application.authority()).parse(),
        TransportId::Quic => format!("quic://{}", application.authority()).parse(),
        TransportId::Ipc | TransportId::WebSocket => {
            Err(RouteConfigurationError::EmptyProviderLocator)
        }
        _ => Err(RouteConfigurationError::UnsupportedProviderScheme),
    }
}

fn validate_security(
    application: &NnrpEndpoint,
    endpoint: &ProviderEndpoint,
    security: Option<&ClientTransportSecurity>,
    descriptor: &TransportProviderDescriptor,
) -> Result<(), &'static str> {
    match descriptor.transport_id {
        TransportId::Quic if security.is_none() => {
            Err("QUIC requires route-local peer verification credentials")
        }
        TransportId::Tcp if application.is_secure() && security.is_none() => {
            Err("nnrps TCP requires route-local peer verification credentials")
        }
        TransportId::Ipc if security.is_some() => {
            Err("IPC does not accept transport security credentials")
        }
        TransportId::Ipc if application.is_secure() => {
            Err("IPC does not satisfy nnrps in Preview4")
        }
        TransportId::WebSocket if !endpoint.is_secure() && security.is_some() => {
            Err("plain WebSocket does not accept transport security credentials")
        }
        TransportId::WebSocket if application.is_secure() && !endpoint.is_secure() => {
            Err("nnrps WebSocket requires a wss provider endpoint")
        }
        TransportId::WebSocket
            if descriptor.kind == TransportProviderKind::Wasm
                && endpoint.is_secure()
                && security.is_some() =>
        {
            Err("browser WSS uses platform trust and rejects native certificate material")
        }
        TransportId::WebSocket
            if descriptor.kind != TransportProviderKind::Wasm
                && endpoint.is_secure()
                && security.is_none() =>
        {
            Err("native WSS requires route-local peer verification credentials")
        }
        _ => Ok(()),
    }
}

fn requires_probe(error: &TransportSelectionError) -> bool {
    let candidates = match error {
        TransportSelectionError::InvalidEvidence { .. } => return false,
        TransportSelectionError::ForcedTransportUnavailable { candidates, .. }
        | TransportSelectionError::NoViableTransport { candidates, .. } => candidates,
    };
    candidates
        .iter()
        .filter(|candidate| {
            candidate.rejection_reason == Some(TransportRejectionReason::ProbeMissing)
        })
        .count()
        > 1
}

async fn probe_candidates(
    providers: &[Arc<dyn NnrpClientProvider>],
    routes: &BTreeMap<String, ResolvedClientRoute>,
    error: &TransportSelectionError,
) -> Vec<TransportProbeObservation> {
    let candidates = match error {
        TransportSelectionError::InvalidEvidence { .. } => return Vec::new(),
        TransportSelectionError::ForcedTransportUnavailable { candidates, .. }
        | TransportSelectionError::NoViableTransport { candidates, .. } => candidates,
    };
    let mut observations = Vec::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.probe_state == ProbeState::Missing)
    {
        let Some(provider) = providers
            .iter()
            .find(|provider| provider.descriptor().metadata.id == candidate.provider.id)
        else {
            continue;
        };
        let Some(route) = routes.get(&candidate.provider.id) else {
            continue;
        };
        let mut samples = Vec::new();
        probe_provider(provider, route, &mut samples).await;
        let descriptor = provider.descriptor();
        match summarize_provider_probe(&descriptor, &samples) {
            Some(metrics) => observations.push(TransportProbeObservation::succeeded(
                descriptor.transport_id,
                descriptor.metadata.id,
                metrics,
            )),
            None => observations.push(TransportProbeObservation::failed(
                descriptor.transport_id,
                descriptor.metadata.id,
                "transport probe failed",
            )),
        }
    }
    observations
}

async fn probe_provider(
    provider: &Arc<dyn NnrpClientProvider>,
    route: &ResolvedClientRoute,
    samples: &mut Vec<ProbeSample>,
) {
    let descriptor = provider.descriptor();
    let started = Instant::now();
    let Ok(mut transport) = provider
        .connect(
            &route.endpoint,
            route.security.as_ref(),
            RuntimeFrameLimits::default(),
        )
        .await
    else {
        samples.push(ProbeSample::failure(
            descriptor.transport_id,
            descriptor.metadata.id,
            elapsed_us(started),
            false,
        ));
        return;
    };

    for probe_id in 1..=PROBE_SAMPLE_COUNT {
        let sample_started = Instant::now();
        let metadata = TransportProbeMetadata {
            probe_id,
            probe_payload_bytes: PROBE_PAYLOAD_BYTES,
            client_send_ts_us: unix_time_us(),
        };
        let packet = RuntimePacket::new(
            CommonHeader::new(MessageType::TransportProbe, 0, 0),
            match metadata.to_bytes() {
                Ok(bytes) => bytes.to_vec(),
                Err(_) => break,
            },
            vec![0; PROBE_PAYLOAD_BYTES as usize],
        );
        let result = async {
            let packet = packet?;
            transport.write_packet(&packet).await?;
            let ack_packet = transport.read_packet().await?;
            if ack_packet.header.message_type != MessageType::TransportProbeAck
                || ack_packet.metadata.len() != TRANSPORT_PROBE_ACK_METADATA_LEN
            {
                return Err(RuntimeError::UnexpectedMessage(
                    "transport probe expected TRANSPORT_PROBE_ACK",
                ));
            }
            let ack = TransportProbeAckMetadata::parse(&ack_packet.metadata)?;
            if ack.probe_id != probe_id {
                return Err(RuntimeError::UnexpectedMessage(
                    "transport probe acknowledgement id mismatch",
                ));
            }
            Ok::<(), RuntimeError>(())
        }
        .await;
        let elapsed = elapsed_us(sample_started);
        match result {
            Ok(()) => samples.push(ProbeSample::success(
                descriptor.transport_id,
                &descriptor.metadata.id,
                elapsed,
                elapsed,
                PROBE_PAYLOAD_BYTES as u64,
                TRANSPORT_PROBE_ACK_METADATA_LEN as u64,
            )),
            Err(_) => {
                samples.push(ProbeSample::failure(
                    descriptor.transport_id,
                    &descriptor.metadata.id,
                    elapsed,
                    false,
                ));
                break;
            }
        }
    }
    let _ = transport.close().await;
}

fn elapsed_us(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros().max(1)).unwrap_or(u64::MAX)
}

fn unix_time_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_micros()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use nnrp_transport_provider::{TransportProviderKind, TransportRejectionReason};

    use super::*;
    use crate::{FramedTransport, RuntimeTransportKind};

    #[derive(Clone)]
    struct TestProvider {
        descriptor: TransportProviderDescriptor,
        packets: Arc<Mutex<VecDeque<RuntimePacket>>>,
        connects: Arc<Mutex<u32>>,
    }

    impl TestProvider {
        fn new(transport_id: TransportId) -> Self {
            Self {
                descriptor: TransportProviderDescriptor::available(
                    format!("test-{transport_id:?}"),
                    "1",
                    transport_id,
                    TransportProviderKind::PureRust,
                ),
                packets: Arc::new(Mutex::new(VecDeque::new())),
                connects: Arc::new(Mutex::new(0)),
            }
        }
    }

    #[async_trait]
    impl NnrpClientProvider for TestProvider {
        fn descriptor(&self) -> TransportProviderDescriptor {
            self.descriptor.clone()
        }

        async fn connect(
            &self,
            _endpoint: &ProviderEndpoint,
            _security: Option<&ClientTransportSecurity>,
            _limits: RuntimeFrameLimits,
        ) -> Result<BoxedFramedTransport, RuntimeError> {
            *self.connects.lock().unwrap() += 1;
            Ok(Box::new(TestTransport {
                kind: RuntimeTransportKind::from_transport_id(self.descriptor.transport_id)
                    .unwrap(),
                packets: Arc::clone(&self.packets),
            }))
        }
    }

    struct TestTransport {
        kind: RuntimeTransportKind,
        packets: Arc<Mutex<VecDeque<RuntimePacket>>>,
    }

    #[async_trait]
    impl FramedTransport for TestTransport {
        fn transport_kind(&self) -> RuntimeTransportKind {
            self.kind
        }

        async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
            self.packets
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(RuntimeError::Internal("missing test probe acknowledgement"))
        }

        async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
            let probe = TransportProbeMetadata::parse(&packet.metadata)?;
            self.packets.lock().unwrap().push_back(RuntimePacket::new(
                CommonHeader::new(MessageType::TransportProbeAck, 0, 0),
                TransportProbeAckMetadata {
                    probe_id: probe.probe_id,
                    server_recv_ts_us: 0,
                }
                .to_bytes()?
                .to_vec(),
                Vec::new(),
            )?);
            Ok(())
        }

        async fn close(&mut self) -> Result<(), RuntimeError> {
            Ok(())
        }
    }

    fn endpoint() -> NnrpEndpoint {
        "nnrp://runtime.example:4433/session".parse().unwrap()
    }

    #[tokio::test]
    async fn one_provider_connects_once_without_probe() {
        let provider = Arc::new(TestProvider::new(TransportId::Tcp));
        let client = crate::NnrpClient::connect(
            NnrpClientOptions::new(
                endpoint(),
                ClientProviderRoutes::new(),
                TransportPolicy::Auto,
                NnrpClientConfig::default(),
            ),
            vec![provider.clone() as Arc<dyn NnrpClientProvider>],
        )
        .await
        .unwrap();

        assert_eq!(*provider.connects.lock().unwrap(), 1);
        assert_eq!(
            client
                .transport_selection()
                .unwrap()
                .selected_provider()
                .transport_id,
            TransportId::Tcp
        );
    }

    #[tokio::test]
    async fn multiple_providers_probe_then_adopt_one_carrier() {
        let tcp = Arc::new(TestProvider::new(TransportId::Tcp));
        let ipc = Arc::new(TestProvider::new(TransportId::Ipc));
        let routes = ClientProviderRoutes::from([(
            TransportId::Ipc,
            ClientProviderRoute::at("unix:///tmp/nnrp.sock".parse().unwrap()),
        )]);
        let client = crate::NnrpClient::connect(
            NnrpClientOptions {
                endpoint: endpoint(),
                provider_routes: routes,
                transport_policy: TransportPolicy::Auto,
                session_defaults: NnrpClientConfig::default(),
            },
            vec![
                tcp.clone() as Arc<dyn NnrpClientProvider>,
                ipc.clone() as Arc<dyn NnrpClientProvider>,
            ],
        )
        .await
        .unwrap();

        let selection = client.transport_selection().unwrap();
        assert_eq!(selection.candidates.len(), 2);
        assert!(selection
            .candidates
            .iter()
            .all(|candidate| candidate.probe_state == ProbeState::Succeeded));
        assert_eq!(
            *tcp.connects.lock().unwrap() + *ipc.connects.lock().unwrap(),
            3
        );
    }

    #[tokio::test]
    async fn missing_routes_and_secure_ipc_remain_diagnostic() {
        let routes = ClientProviderRoutes::from([(
            TransportId::Ipc,
            ClientProviderRoute::at("unix:///tmp/nnrp.sock".parse().unwrap()),
        )]);
        let error = crate::NnrpClient::connect(
            NnrpClientOptions {
                endpoint: "nnrps://runtime.example:4433/session".parse().unwrap(),
                provider_routes: routes,
                transport_policy: TransportPolicy::Auto,
                session_defaults: NnrpClientConfig::default(),
            },
            Vec::<Arc<dyn NnrpClientProvider>>::new(),
        )
        .await
        .unwrap_err();

        let RuntimeError::TransportSelection(TransportSelectionError::NoViableTransport {
            candidates,
            ..
        }) = error
        else {
            panic!("unexpected route selection error")
        };
        assert_eq!(
            candidates[0].rejection_reason,
            Some(TransportRejectionReason::LocalUnavailable)
        );
    }

    #[tokio::test]
    async fn duplicate_transport_providers_are_rejected_before_connect() {
        let first = Arc::new(TestProvider::new(TransportId::Tcp));
        let second = Arc::new(TestProvider::new(TransportId::Tcp));
        let error = crate::NnrpClient::connect(
            NnrpClientOptions {
                endpoint: endpoint(),
                provider_routes: ClientProviderRoutes::new(),
                transport_policy: TransportPolicy::Auto,
                session_defaults: NnrpClientConfig::default(),
            },
            vec![
                first as Arc<dyn NnrpClientProvider>,
                second as Arc<dyn NnrpClientProvider>,
            ],
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::DuplicateTransportProvider(TransportId::Tcp)
        ));
    }

    #[tokio::test]
    async fn duplicate_provider_ids_are_rejected_before_route_resolution() {
        let first = Arc::new(TestProvider::new(TransportId::Tcp));
        let mut second = TestProvider::new(TransportId::Ipc);
        second.descriptor.metadata.id = first.descriptor.metadata.id.clone();
        let error = crate::NnrpClient::connect(
            NnrpClientOptions {
                endpoint: endpoint(),
                provider_routes: ClientProviderRoutes::new(),
                transport_policy: TransportPolicy::Auto,
                session_defaults: NnrpClientConfig::default(),
            },
            vec![
                first as Arc<dyn NnrpClientProvider>,
                Arc::new(second) as Arc<dyn NnrpClientProvider>,
            ],
        )
        .await
        .unwrap_err();
        assert!(matches!(error, RuntimeError::DuplicateClientProviderId(_)));
    }

    #[test]
    fn derived_network_routes_preserve_ipv6_authority() {
        let application = "nnrp://[::1]:4433/session".parse::<NnrpEndpoint>().unwrap();
        assert_eq!(
            derive_provider_endpoint(&application, TransportId::Tcp)
                .unwrap()
                .as_str(),
            "tcp://[::1]:4433"
        );
    }

    #[test]
    fn route_derivation_rejects_missing_or_non_network_locators() {
        let without_port = "nnrp://runtime.example/session"
            .parse::<NnrpEndpoint>()
            .unwrap();
        assert!(derive_provider_endpoint(&without_port, TransportId::Tcp).is_err());

        let application = endpoint();
        assert_eq!(
            derive_provider_endpoint(&application, TransportId::Quic)
                .unwrap()
                .as_str(),
            "quic://runtime.example:4433"
        );
        assert!(derive_provider_endpoint(&application, TransportId::Ipc).is_err());
        assert!(derive_provider_endpoint(&application, TransportId::Unspecified).is_err());
    }

    #[test]
    fn client_security_matrix_matches_the_frozen_route_contract() {
        let plain = endpoint();
        let secure = "nnrps://runtime.example:4433/session"
            .parse::<NnrpEndpoint>()
            .unwrap();
        let tcp = TestProvider::new(TransportId::Tcp).descriptor;
        let quic = TestProvider::new(TransportId::Quic).descriptor;
        let ipc = TestProvider::new(TransportId::Ipc).descriptor;
        let native_ws = TestProvider::new(TransportId::WebSocket).descriptor;
        let mut browser_ws = native_ws.clone();
        browser_ws.kind = TransportProviderKind::Wasm;
        let tcp_endpoint = "tcp://runtime.example:4433".parse().unwrap();
        let quic_endpoint = "quic://runtime.example:4433".parse().unwrap();
        let ipc_endpoint = "unix:///tmp/nnrp.sock".parse().unwrap();
        let ws_endpoint = "ws://runtime.example/nnrp".parse().unwrap();
        let wss_endpoint = "wss://runtime.example/nnrp".parse().unwrap();
        let security = ClientTransportSecurity::new("runtime.example", [1]);

        assert!(validate_security(&plain, &tcp_endpoint, None, &tcp).is_ok());
        assert!(validate_security(&secure, &tcp_endpoint, None, &tcp).is_err());
        assert!(validate_security(&plain, &quic_endpoint, None, &quic).is_err());
        assert!(validate_security(&plain, &quic_endpoint, Some(&security), &quic).is_ok());
        assert!(validate_security(&secure, &ipc_endpoint, None, &ipc).is_err());
        assert!(validate_security(&plain, &ipc_endpoint, Some(&security), &ipc).is_err());
        assert!(validate_security(&secure, &ws_endpoint, None, &native_ws).is_err());
        assert!(validate_security(&plain, &ws_endpoint, Some(&security), &native_ws).is_err());
        assert!(validate_security(&plain, &wss_endpoint, None, &native_ws).is_err());
        assert!(validate_security(&plain, &wss_endpoint, Some(&security), &native_ws).is_ok());
        assert!(validate_security(&plain, &wss_endpoint, Some(&security), &browser_ws).is_err());
        assert!(validate_security(&plain, &wss_endpoint, None, &browser_ws).is_ok());
    }

    #[test]
    fn candidate_resolution_reports_locator_and_security_failures() {
        let tcp = TestProvider::new(TransportId::Tcp).descriptor;
        let mismatched = ClientProviderRoute::at("ws://runtime.example/nnrp".parse().unwrap());
        let failure = resolve_candidate(&endpoint(), Some(&mismatched), &tcp).unwrap_err();
        assert!(!failure.route_resolved);

        let invalid_security = ClientProviderRoute {
            provider_endpoint: Some("tcp://runtime.example:4433".parse().unwrap()),
            security: Some(ClientTransportSecurity::new("", [1])),
        };
        let failure = resolve_candidate(&endpoint(), Some(&invalid_security), &tcp).unwrap_err();
        assert!(!failure.security_satisfied);

        let unavailable = TransportProviderDescriptor::missing(
            "missing",
            "1",
            TransportId::Tcp,
            TransportProviderKind::PureRust,
            "not installed",
        );
        assert!(resolve_candidate(&endpoint(), None, &unavailable).is_err());
    }
}
