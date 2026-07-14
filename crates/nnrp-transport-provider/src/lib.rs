use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use nnrp_core::TransportId;
use thiserror::Error;

pub const DEFAULT_PROVIDER_MAX_FRAME_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportPolicy {
    Auto,
    PreferQuic,
    PreferTcp,
    PreferIpc,
    PreferWebSocket,
    ForceQuic,
    ForceTcp,
    ForceIpc,
    ForceWebSocket,
}

impl TransportPolicy {
    pub fn allows(self, transport_id: TransportId) -> bool {
        match self {
            Self::Auto
            | Self::PreferQuic
            | Self::PreferTcp
            | Self::PreferIpc
            | Self::PreferWebSocket => is_selectable_transport(transport_id),
            Self::ForceQuic => transport_id == TransportId::Quic,
            Self::ForceTcp => transport_id == TransportId::Tcp,
            Self::ForceIpc => transport_id == TransportId::Ipc,
            Self::ForceWebSocket => transport_id == TransportId::WebSocket,
        }
    }

    fn preferred_transport(self) -> Option<TransportId> {
        match self {
            Self::PreferQuic | Self::ForceQuic => Some(TransportId::Quic),
            Self::PreferTcp | Self::ForceTcp => Some(TransportId::Tcp),
            Self::PreferIpc | Self::ForceIpc => Some(TransportId::Ipc),
            Self::PreferWebSocket | Self::ForceWebSocket => Some(TransportId::WebSocket),
            Self::Auto => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportProviderKind {
    PureRust,
    NativeDynamic,
    Wasm,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderFeatureSet {
    pub tcp: bool,
    pub quic: bool,
    pub ipc: bool,
    pub websocket: bool,
    pub native_loader: bool,
    pub wasm: bool,
}

pub const fn compile_time_provider_features() -> ProviderFeatureSet {
    ProviderFeatureSet {
        tcp: cfg!(feature = "tcp"),
        quic: cfg!(feature = "quic"),
        ipc: cfg!(feature = "ipc"),
        websocket: cfg!(feature = "websocket"),
        native_loader: cfg!(feature = "native-loader"),
        wasm: cfg!(feature = "wasm"),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderCost {
    pub model_id: u16,
    pub units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderLimits {
    pub max_frame_bytes: u64,
}

impl Default for ProviderLimits {
    fn default() -> Self {
        Self {
            max_frame_bytes: DEFAULT_PROVIDER_MAX_FRAME_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProviderLimitation {
    RequiresUdp,
    RequiresTcp,
    LocalHostOnly,
    NativeHostOnly,
    BrowserHostOnly,
    UnixDomainSocket,
    WindowsNamedPipe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportProviderMetadata {
    pub id: String,
    pub cost: ProviderCost,
    pub preference_rank: u16,
    pub limits: ProviderLimits,
    pub limitations: Vec<ProviderLimitation>,
}

impl TransportProviderMetadata {
    pub fn official(transport_id: TransportId, kind: TransportProviderKind) -> Self {
        let (id, preference_rank, limitations) = match (transport_id, kind) {
            (TransportId::WebSocket, TransportProviderKind::Wasm) => (
                "nnrp.transport.websocket.browser-wasm",
                3,
                vec![
                    ProviderLimitation::RequiresTcp,
                    ProviderLimitation::BrowserHostOnly,
                ],
            ),
            (_, TransportProviderKind::Wasm) => (
                "nnrp.transport.unspecified.browser-wasm",
                u16::MAX,
                vec![ProviderLimitation::BrowserHostOnly],
            ),
            (TransportId::Quic, _) => (
                "nnrp.transport.quic.native",
                1,
                vec![
                    ProviderLimitation::RequiresUdp,
                    ProviderLimitation::NativeHostOnly,
                ],
            ),
            (TransportId::Tcp, _) => (
                "nnrp.transport.tcp.native",
                2,
                vec![
                    ProviderLimitation::RequiresTcp,
                    ProviderLimitation::NativeHostOnly,
                ],
            ),
            (TransportId::Ipc, _) => {
                let mut limitations = vec![
                    ProviderLimitation::LocalHostOnly,
                    ProviderLimitation::NativeHostOnly,
                ];
                if cfg!(target_os = "windows") {
                    limitations.push(ProviderLimitation::WindowsNamedPipe);
                } else {
                    limitations.push(ProviderLimitation::UnixDomainSocket);
                }
                ("nnrp.transport.ipc.native", 0, limitations)
            }
            (TransportId::WebSocket, _) => (
                "nnrp.transport.websocket.native",
                3,
                vec![
                    ProviderLimitation::RequiresTcp,
                    ProviderLimitation::NativeHostOnly,
                ],
            ),
            _ => ("nnrp.transport.unspecified.native", u16::MAX, Vec::new()),
        };

        Self {
            id: id.to_owned(),
            cost: ProviderCost::default(),
            preference_rank,
            limits: ProviderLimits::default(),
            limitations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportProviderDescriptor {
    pub name: String,
    pub version: String,
    pub transport_id: TransportId,
    pub kind: TransportProviderKind,
    pub available: bool,
    pub library_path: Option<PathBuf>,
    pub metadata: TransportProviderMetadata,
    pub diagnostic: Option<String>,
}

impl TransportProviderDescriptor {
    pub fn available(
        name: impl Into<String>,
        version: impl Into<String>,
        transport_id: TransportId,
        kind: TransportProviderKind,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            transport_id,
            kind,
            available: true,
            library_path: None,
            metadata: TransportProviderMetadata::official(transport_id, kind),
            diagnostic: None,
        }
    }

    pub fn missing(
        name: impl Into<String>,
        version: impl Into<String>,
        transport_id: TransportId,
        kind: TransportProviderKind,
        diagnostic: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            transport_id,
            kind,
            available: false,
            library_path: None,
            metadata: TransportProviderMetadata::official(transport_id, kind),
            diagnostic: Some(diagnostic.into()),
        }
    }

    pub fn with_library_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.library_path = Some(path.into());
        self
    }

    pub fn with_metadata(mut self, metadata: TransportProviderMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransportProviderRegistry {
    providers: Vec<TransportProviderDescriptor>,
}

impl TransportProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn with_provider(mut self, provider: TransportProviderDescriptor) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn register(&mut self, provider: TransportProviderDescriptor) {
        self.providers.push(provider);
    }

    pub fn providers(&self) -> &[TransportProviderDescriptor] {
        &self.providers
    }

    pub fn available_for(&self, transport_id: TransportId) -> Vec<&TransportProviderDescriptor> {
        self.providers
            .iter()
            .filter(|provider| provider.available && provider.transport_id == transport_id)
            .collect()
    }

    pub fn select(
        &self,
        remote: &RemoteTransportSupport,
        policy: TransportPolicy,
        requested_max_frame_bytes: Option<u64>,
    ) -> Result<TransportSelection, TransportSelectionError> {
        select_transport(self.providers(), remote, policy, requested_max_frame_bytes)
    }

    pub fn select_with_probe(
        &self,
        remote: &RemoteTransportSupport,
        policy: TransportPolicy,
        requested_max_frame_bytes: Option<u64>,
        samples: &[ProbeSample],
    ) -> Result<TransportSelection, TransportSelectionError> {
        select_transport_with_probe(
            self.providers(),
            remote,
            policy,
            requested_max_frame_bytes,
            samples,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTransportSupport {
    supported: Vec<TransportId>,
}

impl RemoteTransportSupport {
    pub fn new(transports: impl IntoIterator<Item = TransportId>) -> Self {
        let mut supported = transports
            .into_iter()
            .filter(|transport_id| is_selectable_transport(*transport_id))
            .collect::<Vec<_>>();
        supported.sort_by_key(|transport_id| *transport_id as u32);
        supported.dedup();
        Self { supported }
    }

    pub fn supports(&self, transport_id: TransportId) -> bool {
        self.supported.contains(&transport_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeState {
    NotRun,
    Succeeded,
    Failed,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeMetrics {
    pub sample_count: u32,
    pub success_count: u32,
    pub median_throughput_bytes_per_sec: u64,
    pub median_rtt_us: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeSample {
    pub transport_id: TransportId,
    pub provider_id: String,
    pub elapsed_us: u64,
    pub rtt_us: Option<u64>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub timed_out: bool,
    pub failed: bool,
}

impl ProbeSample {
    pub fn success(
        transport_id: TransportId,
        provider_id: impl Into<String>,
        elapsed_us: u64,
        rtt_us: u64,
        bytes_sent: u64,
        bytes_received: u64,
    ) -> Self {
        Self {
            transport_id,
            provider_id: provider_id.into(),
            elapsed_us,
            rtt_us: Some(rtt_us),
            bytes_sent,
            bytes_received,
            timed_out: false,
            failed: false,
        }
    }

    pub fn failure(
        transport_id: TransportId,
        provider_id: impl Into<String>,
        elapsed_us: u64,
        timed_out: bool,
    ) -> Self {
        Self {
            transport_id,
            provider_id: provider_id.into(),
            elapsed_us,
            rtt_us: None,
            bytes_sent: 0,
            bytes_received: 0,
            timed_out,
            failed: true,
        }
    }

    fn is_successful(&self) -> bool {
        !self.failed && !self.timed_out && self.rtt_us.is_some() && self.elapsed_us > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportRejectionReason {
    PolicyDisallowed,
    LocalUnavailable,
    PeerUnsupported,
    LimitExceeded,
    ProbeMissing,
    ProbeFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportCandidateDiagnostic {
    pub transport_id: TransportId,
    pub provider: TransportProviderMetadata,
    pub local_available: bool,
    pub peer_supported: bool,
    pub within_limits: bool,
    pub probe_state: ProbeState,
    pub probe: Option<ProbeMetrics>,
    pub selection_rank: Option<u32>,
    pub rejection_reason: Option<TransportRejectionReason>,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportSelection {
    pub selected: TransportProviderDescriptor,
    pub candidates: Vec<TransportCandidateDiagnostic>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransportSelectionError {
    #[error("forced transport is not available: {transport_id:?}")]
    ForcedTransportUnavailable {
        transport_id: TransportId,
        candidates: Vec<TransportCandidateDiagnostic>,
    },
    #[error("no viable transport provider after applying policy and remote support")]
    NoViableTransport {
        candidates: Vec<TransportCandidateDiagnostic>,
    },
}

pub fn select_transport(
    providers: &[TransportProviderDescriptor],
    remote: &RemoteTransportSupport,
    policy: TransportPolicy,
    requested_max_frame_bytes: Option<u64>,
) -> Result<TransportSelection, TransportSelectionError> {
    let mut candidates = evaluate_candidates(providers, remote, policy, requested_max_frame_bytes);
    let eligible = eligible_indices(&candidates);

    if eligible.len() == 1 {
        return direct_selection(candidates, eligible[0]);
    }

    if eligible.len() > 1 {
        for index in eligible {
            let candidate = &mut candidates[index].diagnostic;
            candidate.probe_state = ProbeState::Missing;
            candidate.rejection_reason = Some(TransportRejectionReason::ProbeMissing);
        }
    }

    selection_error(policy, ordered_diagnostics(candidates))
}

pub fn select_transport_with_probe(
    providers: &[TransportProviderDescriptor],
    remote: &RemoteTransportSupport,
    policy: TransportPolicy,
    requested_max_frame_bytes: Option<u64>,
    samples: &[ProbeSample],
) -> Result<TransportSelection, TransportSelectionError> {
    let mut candidates = evaluate_candidates(providers, remote, policy, requested_max_frame_bytes);
    let eligible = eligible_indices(&candidates);

    if eligible.len() == 1 {
        return direct_selection(candidates, eligible[0]);
    }

    for index in eligible {
        let provider = &candidates[index].descriptor;
        let has_samples = matching_probe_samples(provider, samples).next().is_some();
        let metrics = summarize_provider_probe(provider, samples);
        let candidate = &mut candidates[index].diagnostic;
        match (has_samples, metrics) {
            (_, Some(metrics)) => {
                candidate.probe_state = ProbeState::Succeeded;
                candidate.probe = Some(metrics);
            }
            (true, None) => {
                candidate.probe_state = ProbeState::Failed;
                candidate.rejection_reason = Some(TransportRejectionReason::ProbeFailed);
            }
            (false, None) => {
                candidate.probe_state = ProbeState::Missing;
                candidate.rejection_reason = Some(TransportRejectionReason::ProbeMissing);
            }
        }
    }

    let mut successful = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            (candidate.diagnostic.probe_state == ProbeState::Succeeded).then_some(index)
        })
        .collect::<Vec<_>>();
    successful
        .sort_by(|left, right| compare_candidates(&candidates[*left], &candidates[*right], policy));

    for (rank, index) in successful.iter().copied().enumerate() {
        candidates[index].diagnostic.selection_rank = Some(rank as u32);
    }

    match successful.first().copied() {
        Some(selected_index) => {
            let selected = candidates[selected_index].descriptor.clone();
            Ok(TransportSelection {
                selected,
                candidates: ordered_diagnostics(candidates),
            })
        }
        None => selection_error(policy, ordered_diagnostics(candidates)),
    }
}

pub fn summarize_provider_probe(
    provider: &TransportProviderDescriptor,
    samples: &[ProbeSample],
) -> Option<ProbeMetrics> {
    let provider_samples = matching_probe_samples(provider, samples).collect::<Vec<_>>();
    let successful = provider_samples
        .iter()
        .copied()
        .filter(|sample| sample.is_successful())
        .collect::<Vec<_>>();
    if successful.is_empty() {
        return None;
    }

    let throughputs = successful
        .iter()
        .map(|sample| {
            sample
                .bytes_sent
                .saturating_add(sample.bytes_received)
                .saturating_mul(1_000_000)
                / sample.elapsed_us
        })
        .collect::<Vec<_>>();
    let rtts = successful
        .iter()
        .filter_map(|sample| sample.rtt_us)
        .collect::<Vec<_>>();

    Some(ProbeMetrics {
        sample_count: saturating_u32(provider_samples.len()),
        success_count: saturating_u32(successful.len()),
        median_throughput_bytes_per_sec: median(throughputs),
        median_rtt_us: median(rtts),
    })
}

pub fn detect_native_library(
    name: impl Into<String>,
    version: impl Into<String>,
    transport_id: TransportId,
    candidates: impl IntoIterator<Item = PathBuf>,
) -> TransportProviderDescriptor {
    let name = name.into();
    let version = version.into();
    for candidate in candidates {
        if candidate.is_file() {
            return TransportProviderDescriptor::available(
                name,
                version,
                transport_id,
                TransportProviderKind::NativeDynamic,
            )
            .with_library_path(candidate);
        }
    }

    TransportProviderDescriptor::missing(
        name,
        version,
        transport_id,
        TransportProviderKind::NativeDynamic,
        "native transport library was not found",
    )
}

pub fn conventional_native_library_name(base_name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{base_name}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{base_name}.dylib")
    } else {
        format!("lib{base_name}.so")
    }
}

pub fn candidate_library_paths(
    explicit_path: Option<&Path>,
    package_dir: Option<&Path>,
    library_name: &str,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = explicit_path {
        paths.push(path.to_path_buf());
    }
    if let Some(package_dir) = package_dir {
        paths.push(package_dir.join(library_name));
        paths.push(package_dir.join("native").join(library_name));
    }
    paths
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvaluatedCandidate {
    descriptor: TransportProviderDescriptor,
    diagnostic: TransportCandidateDiagnostic,
}

fn evaluate_candidates(
    providers: &[TransportProviderDescriptor],
    remote: &RemoteTransportSupport,
    policy: TransportPolicy,
    requested_max_frame_bytes: Option<u64>,
) -> Vec<EvaluatedCandidate> {
    providers
        .iter()
        .filter(|provider| is_selectable_transport(provider.transport_id))
        .map(|provider| {
            let peer_supported = remote.supports(provider.transport_id);
            let within_limits = requested_max_frame_bytes
                .map(|requested| requested <= provider.metadata.limits.max_frame_bytes)
                .unwrap_or(true);
            let rejection_reason = if !policy.allows(provider.transport_id) {
                Some(TransportRejectionReason::PolicyDisallowed)
            } else if !provider.available {
                Some(TransportRejectionReason::LocalUnavailable)
            } else if !peer_supported {
                Some(TransportRejectionReason::PeerUnsupported)
            } else if !within_limits {
                Some(TransportRejectionReason::LimitExceeded)
            } else {
                None
            };

            EvaluatedCandidate {
                descriptor: provider.clone(),
                diagnostic: TransportCandidateDiagnostic {
                    transport_id: provider.transport_id,
                    provider: provider.metadata.clone(),
                    local_available: provider.available,
                    peer_supported,
                    within_limits,
                    probe_state: ProbeState::NotRun,
                    probe: None,
                    selection_rank: None,
                    rejection_reason,
                    diagnostic: provider.diagnostic.clone(),
                },
            }
        })
        .collect()
}

fn eligible_indices(candidates: &[EvaluatedCandidate]) -> Vec<usize> {
    candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            candidate
                .diagnostic
                .rejection_reason
                .is_none()
                .then_some(index)
        })
        .collect()
}

fn direct_selection(
    mut candidates: Vec<EvaluatedCandidate>,
    selected_index: usize,
) -> Result<TransportSelection, TransportSelectionError> {
    candidates[selected_index].diagnostic.selection_rank = Some(0);
    let selected = candidates[selected_index].descriptor.clone();
    Ok(TransportSelection {
        selected,
        candidates: ordered_diagnostics(candidates),
    })
}

fn compare_candidates(
    left: &EvaluatedCandidate,
    right: &EvaluatedCandidate,
    policy: TransportPolicy,
) -> Ordering {
    let left_probe = left
        .diagnostic
        .probe
        .as_ref()
        .expect("successful candidate must carry probe metrics");
    let right_probe = right
        .diagnostic
        .probe
        .as_ref()
        .expect("successful candidate must carry probe metrics");

    right_probe
        .success_count
        .cmp(&left_probe.success_count)
        .then_with(|| {
            right_probe
                .median_throughput_bytes_per_sec
                .cmp(&left_probe.median_throughput_bytes_per_sec)
        })
        .then_with(|| left_probe.median_rtt_us.cmp(&right_probe.median_rtt_us))
        .then_with(|| {
            compare_cost(
                &left.descriptor.metadata.cost,
                &right.descriptor.metadata.cost,
            )
        })
        .then_with(|| {
            let preferred = policy.preferred_transport();
            let left_preferred = Some(left.descriptor.transport_id) == preferred;
            let right_preferred = Some(right.descriptor.transport_id) == preferred;
            right_preferred.cmp(&left_preferred)
        })
        .then_with(|| {
            left.descriptor
                .metadata
                .preference_rank
                .cmp(&right.descriptor.metadata.preference_rank)
        })
        .then_with(|| {
            (left.descriptor.transport_id as u32).cmp(&(right.descriptor.transport_id as u32))
        })
        .then_with(|| {
            left.descriptor
                .metadata
                .id
                .cmp(&right.descriptor.metadata.id)
        })
}

fn compare_cost(left: &ProviderCost, right: &ProviderCost) -> Ordering {
    if left.model_id != 0 && left.model_id == right.model_id {
        left.units.cmp(&right.units)
    } else {
        Ordering::Equal
    }
}

fn ordered_diagnostics(
    mut candidates: Vec<EvaluatedCandidate>,
) -> Vec<TransportCandidateDiagnostic> {
    candidates.sort_by(|left, right| {
        match (
            left.diagnostic.selection_rank,
            right.diagnostic.selection_rank,
        ) {
            (Some(left_rank), Some(right_rank)) => left_rank.cmp(&right_rank),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => (left.descriptor.transport_id as u32)
                .cmp(&(right.descriptor.transport_id as u32))
                .then_with(|| {
                    left.descriptor
                        .metadata
                        .id
                        .cmp(&right.descriptor.metadata.id)
                }),
        }
    });
    candidates
        .into_iter()
        .map(|candidate| candidate.diagnostic)
        .collect()
}

fn matching_probe_samples<'a>(
    provider: &'a TransportProviderDescriptor,
    samples: &'a [ProbeSample],
) -> impl Iterator<Item = &'a ProbeSample> {
    samples.iter().filter(|sample| {
        sample.transport_id == provider.transport_id && sample.provider_id == provider.metadata.id
    })
}

fn median(mut values: Vec<u64>) -> u64 {
    values.sort_unstable();
    let upper = values.len() / 2;
    if values.len() % 2 == 1 {
        values[upper]
    } else {
        let lower_value = values[upper - 1];
        lower_value + (values[upper] - lower_value) / 2
    }
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn selection_error<T>(
    policy: TransportPolicy,
    candidates: Vec<TransportCandidateDiagnostic>,
) -> Result<T, TransportSelectionError> {
    match forced_transport(policy) {
        Some(transport_id) => Err(TransportSelectionError::ForcedTransportUnavailable {
            transport_id,
            candidates,
        }),
        None => Err(TransportSelectionError::NoViableTransport { candidates }),
    }
}

fn forced_transport(policy: TransportPolicy) -> Option<TransportId> {
    match policy {
        TransportPolicy::ForceQuic => Some(TransportId::Quic),
        TransportPolicy::ForceTcp => Some(TransportId::Tcp),
        TransportPolicy::ForceIpc => Some(TransportId::Ipc),
        TransportPolicy::ForceWebSocket => Some(TransportId::WebSocket),
        _ => None,
    }
}

fn is_selectable_transport(transport_id: TransportId) -> bool {
    matches!(
        transport_id,
        TransportId::Quic | TransportId::Tcp | TransportId::Ipc | TransportId::WebSocket
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn official_metadata_matches_transport_contract() {
        let quic =
            TransportProviderMetadata::official(TransportId::Quic, TransportProviderKind::PureRust);
        assert_eq!(quic.id, "nnrp.transport.quic.native");
        assert_eq!(quic.preference_rank, 1);

        let tcp = TransportProviderMetadata::official(
            TransportId::Tcp,
            TransportProviderKind::NativeDynamic,
        );
        assert_eq!(tcp.id, "nnrp.transport.tcp.native");
        assert_eq!(tcp.preference_rank, 2);

        let ipc =
            TransportProviderMetadata::official(TransportId::Ipc, TransportProviderKind::PureRust);
        assert_eq!(ipc.id, "nnrp.transport.ipc.native");
        assert_eq!(ipc.preference_rank, 0);
        assert_eq!(ipc.limits.max_frame_bytes, DEFAULT_PROVIDER_MAX_FRAME_BYTES);
        assert!(ipc.limitations.contains(&ProviderLimitation::LocalHostOnly));

        let browser = TransportProviderMetadata::official(
            TransportId::WebSocket,
            TransportProviderKind::Wasm,
        );
        assert_eq!(browser.id, "nnrp.transport.websocket.browser-wasm");
        assert_eq!(browser.preference_rank, 3);
        assert_eq!(
            browser.limitations,
            vec![
                ProviderLimitation::RequiresTcp,
                ProviderLimitation::BrowserHostOnly,
            ]
        );

        let native_websocket = TransportProviderMetadata::official(
            TransportId::WebSocket,
            TransportProviderKind::PureRust,
        );
        assert_eq!(native_websocket.id, "nnrp.transport.websocket.native");

        let unsupported_wasm =
            TransportProviderMetadata::official(TransportId::Tcp, TransportProviderKind::Wasm);
        assert_eq!(
            unsupported_wasm.id,
            "nnrp.transport.unspecified.browser-wasm"
        );
    }

    #[test]
    fn transport_policy_helpers_cover_all_frozen_variants() {
        assert_eq!(
            TransportPolicy::PreferQuic.preferred_transport(),
            Some(TransportId::Quic)
        );
        assert_eq!(
            TransportPolicy::PreferTcp.preferred_transport(),
            Some(TransportId::Tcp)
        );
        assert_eq!(
            TransportPolicy::PreferIpc.preferred_transport(),
            Some(TransportId::Ipc)
        );
        assert_eq!(
            TransportPolicy::PreferWebSocket.preferred_transport(),
            Some(TransportId::WebSocket)
        );
        assert_eq!(TransportPolicy::Auto.preferred_transport(), None);

        assert_eq!(
            forced_transport(TransportPolicy::ForceQuic),
            Some(TransportId::Quic)
        );
        assert_eq!(
            forced_transport(TransportPolicy::ForceTcp),
            Some(TransportId::Tcp)
        );
        assert_eq!(
            forced_transport(TransportPolicy::ForceIpc),
            Some(TransportId::Ipc)
        );
        assert_eq!(
            forced_transport(TransportPolicy::ForceWebSocket),
            Some(TransportId::WebSocket)
        );
        assert_eq!(forced_transport(TransportPolicy::Auto), None);
    }

    #[test]
    fn registry_selects_single_eligible_provider_without_probe() {
        let registry = TransportProviderRegistry::new()
            .with_provider(available(TransportId::Tcp))
            .with_provider(available(TransportId::Quic));
        let remote = RemoteTransportSupport::new([TransportId::Tcp]);

        let selection = registry
            .select(&remote, TransportPolicy::Auto, None)
            .expect("one eligible provider should be selected directly");

        assert_eq!(selection.selected.transport_id, TransportId::Tcp);
        assert_eq!(selection.candidates[0].selection_rank, Some(0));
        assert_eq!(selection.candidates[0].probe_state, ProbeState::NotRun);
        assert_eq!(
            selection.candidates[1].rejection_reason,
            Some(TransportRejectionReason::PeerUnsupported)
        );
    }

    #[test]
    fn registry_requires_probe_for_multiple_eligible_providers() {
        let registry = TransportProviderRegistry::new()
            .with_provider(available(TransportId::Tcp))
            .with_provider(available(TransportId::Quic));
        let remote = RemoteTransportSupport::new([TransportId::Tcp, TransportId::Quic]);

        let error = registry
            .select(&remote, TransportPolicy::Auto, None)
            .expect_err("multiple providers must not use a private static score");
        let TransportSelectionError::NoViableTransport { candidates } = error else {
            panic!("unexpected forced error")
        };
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(|candidate| {
            candidate.probe_state == ProbeState::Missing
                && candidate.rejection_reason == Some(TransportRejectionReason::ProbeMissing)
        }));
    }

    #[test]
    fn probe_aggregation_uses_provider_id_and_frozen_median_math() {
        let provider = available(TransportId::Quic);
        let provider_id = provider.metadata.id.clone();
        let samples = [
            ProbeSample::success(TransportId::Quic, &provider_id, 10, 10, 10, 10),
            ProbeSample::success(TransportId::Quic, &provider_id, 10, 20, 20, 20),
            ProbeSample::failure(TransportId::Quic, &provider_id, 10, true),
            ProbeSample::success(TransportId::Quic, "display-name", 10, 1, 100, 100),
        ];

        let metrics = summarize_provider_probe(&provider, &samples).expect("two samples succeed");
        assert_eq!(metrics.sample_count, 3);
        assert_eq!(metrics.success_count, 2);
        assert_eq!(metrics.median_throughput_bytes_per_sec, 3_000_000);
        assert_eq!(metrics.median_rtt_us, 15);
    }

    #[test]
    fn probe_selection_uses_quality_before_preference() {
        let tcp = available(TransportId::Tcp);
        let quic = available(TransportId::Quic);
        let samples = [
            ProbeSample::success(TransportId::Tcp, &tcp.metadata.id, 100, 100, 50, 50),
            ProbeSample::success(TransportId::Quic, &quic.metadata.id, 100, 10, 5, 5),
        ];
        let remote = RemoteTransportSupport::new([TransportId::Tcp, TransportId::Quic]);

        let selection = select_transport_with_probe(
            &[tcp, quic],
            &remote,
            TransportPolicy::PreferQuic,
            None,
            &samples,
        )
        .expect("both probes succeeded");

        assert_eq!(selection.selected.transport_id, TransportId::Tcp);
        assert_eq!(selection.candidates[0].selection_rank, Some(0));
        assert_eq!(selection.candidates[1].selection_rank, Some(1));
    }

    #[test]
    fn probe_selection_uses_comparable_cost_then_explicit_preference() {
        let mut tcp = available(TransportId::Tcp);
        tcp.metadata.cost = ProviderCost {
            model_id: 7,
            units: 20,
        };
        let mut quic = available(TransportId::Quic);
        quic.metadata.cost = ProviderCost {
            model_id: 7,
            units: 10,
        };
        let samples = [
            ProbeSample::success(TransportId::Tcp, &tcp.metadata.id, 100, 10, 10, 10),
            ProbeSample::success(TransportId::Quic, &quic.metadata.id, 100, 10, 10, 10),
        ];
        let remote = RemoteTransportSupport::new([TransportId::Tcp, TransportId::Quic]);

        let by_cost = select_transport_with_probe(
            &[tcp.clone(), quic.clone()],
            &remote,
            TransportPolicy::PreferTcp,
            None,
            &samples,
        )
        .expect("cost should be comparable");
        assert_eq!(by_cost.selected.transport_id, TransportId::Quic);

        tcp.metadata.cost.model_id = 8;
        let by_preference = select_transport_with_probe(
            &[tcp, quic],
            &remote,
            TransportPolicy::PreferTcp,
            None,
            &samples,
        )
        .expect("different cost models defer to explicit preference");
        assert_eq!(by_preference.selected.transport_id, TransportId::Tcp);
    }

    #[test]
    fn registry_probe_selection_applies_preference_rank_and_provider_identity() {
        let mut first = available(TransportId::Tcp);
        first.metadata.id = "nnrp.transport.tcp.z".to_owned();
        first.metadata.preference_rank = 9;
        let mut second = available(TransportId::Tcp);
        second.metadata.id = "nnrp.transport.tcp.a".to_owned();
        second.metadata.preference_rank = 8;
        let remote = RemoteTransportSupport::new([TransportId::Tcp]);
        let samples = [
            ProbeSample::success(TransportId::Tcp, &first.metadata.id, 10, 10, 10, 10),
            ProbeSample::success(TransportId::Tcp, &second.metadata.id, 10, 10, 10, 10),
        ];
        let registry = TransportProviderRegistry::new()
            .with_provider(first)
            .with_provider(second.clone());

        let selection = registry
            .select_with_probe(&remote, TransportPolicy::Auto, None, &samples)
            .expect("preference rank should break the quality tie");
        assert_eq!(selection.selected.metadata.id, second.metadata.id);
        assert_eq!(selection.candidates[0].selection_rank, Some(0));
        assert_eq!(selection.candidates[1].selection_rank, Some(1));

        let mut first = selection.selected.clone();
        first.metadata.id = "nnrp.transport.tcp.z".to_owned();
        first.metadata.preference_rank = 8;
        let mut second = selection.selected;
        second.metadata.id = "nnrp.transport.tcp.a".to_owned();
        let samples = [
            ProbeSample::success(TransportId::Tcp, &first.metadata.id, 10, 10, 10, 10),
            ProbeSample::success(TransportId::Tcp, &second.metadata.id, 10, 10, 10, 10),
        ];

        let selection = select_transport_with_probe(
            &[first, second.clone()],
            &remote,
            TransportPolicy::Auto,
            None,
            &samples,
        )
        .expect("provider id should break the final tie");
        assert_eq!(selection.selected.metadata.id, second.metadata.id);
    }

    #[test]
    fn selection_reports_limits_and_failed_probe_diagnostics() {
        let tcp = available(TransportId::Tcp);
        let quic = available(TransportId::Quic);
        let samples = [ProbeSample::failure(
            TransportId::Tcp,
            &tcp.metadata.id,
            50,
            true,
        )];
        let remote = RemoteTransportSupport::new([TransportId::Tcp, TransportId::Quic]);

        let error = select_transport_with_probe(
            &[tcp, quic],
            &remote,
            TransportPolicy::Auto,
            Some(DEFAULT_PROVIDER_MAX_FRAME_BYTES + 1),
            &samples,
        )
        .expect_err("both providers exceed the requested frame size");
        let TransportSelectionError::NoViableTransport { candidates } = error else {
            panic!("unexpected forced error")
        };
        assert!(candidates.iter().all(|candidate| {
            candidate.rejection_reason == Some(TransportRejectionReason::LimitExceeded)
        }));

        let tcp = available(TransportId::Tcp);
        let quic = available(TransportId::Quic);
        let samples = [ProbeSample::failure(
            TransportId::Tcp,
            &tcp.metadata.id,
            50,
            true,
        )];
        let error = select_transport_with_probe(
            &[tcp, quic],
            &remote,
            TransportPolicy::Auto,
            None,
            &samples,
        )
        .expect_err("failed and missing probes leave no candidate");
        let TransportSelectionError::NoViableTransport { candidates } = error else {
            panic!("unexpected forced error")
        };
        assert!(candidates.iter().any(|candidate| {
            candidate.rejection_reason == Some(TransportRejectionReason::ProbeFailed)
        }));
        assert!(candidates.iter().any(|candidate| {
            candidate.rejection_reason == Some(TransportRejectionReason::ProbeMissing)
        }));
    }

    #[test]
    fn force_policy_preserves_complete_candidate_diagnostics() {
        let registry = TransportProviderRegistry::new()
            .with_provider(available(TransportId::Tcp))
            .with_provider(available(TransportId::Quic));
        let remote = RemoteTransportSupport::new([TransportId::Tcp]);

        let error = registry
            .select(&remote, TransportPolicy::ForceQuic, None)
            .expect_err("forced quic is unavailable remotely");
        let TransportSelectionError::ForcedTransportUnavailable {
            transport_id,
            candidates,
        } = error
        else {
            panic!("unexpected non-forced error")
        };
        assert_eq!(transport_id, TransportId::Quic);
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn native_library_detection_reports_present_and_missing_paths() {
        let temp_dir =
            std::env::temp_dir().join(format!("nnrp-provider-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let library_name = conventional_native_library_name("nnrp_quic");
        let library_path = temp_dir.join(&library_name);
        fs::write(&library_path, b"stub").expect("stub library should be written");

        let paths = candidate_library_paths(None, Some(&temp_dir), &library_name);
        let present = detect_native_library("quic-native", "0.0.0", TransportId::Quic, paths);
        assert!(present.available);
        assert_eq!(present.library_path, Some(library_path));
        assert_eq!(present.metadata.id, "nnrp.transport.quic.native");

        let missing = detect_native_library(
            "quic-native",
            "0.0.0",
            TransportId::Quic,
            [temp_dir.join("missing.dll")],
        );
        assert!(!missing.available);
        assert_eq!(
            missing.diagnostic.as_deref(),
            Some("native transport library was not found")
        );

        fs::remove_dir_all(temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn compile_time_provider_features_report_enabled_cargo_features() {
        assert_eq!(
            compile_time_provider_features(),
            ProviderFeatureSet {
                tcp: cfg!(feature = "tcp"),
                quic: cfg!(feature = "quic"),
                ipc: cfg!(feature = "ipc"),
                websocket: cfg!(feature = "websocket"),
                native_loader: cfg!(feature = "native-loader"),
                wasm: cfg!(feature = "wasm"),
            }
        );
    }

    fn available(transport_id: TransportId) -> TransportProviderDescriptor {
        TransportProviderDescriptor::available(
            format!("{transport_id:?}"),
            "0.0.0",
            transport_id,
            TransportProviderKind::PureRust,
        )
    }
}
