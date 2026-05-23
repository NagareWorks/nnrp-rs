use std::path::{Path, PathBuf};

use nnrp_core::TransportId;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportPolicy {
    Auto,
    PreferQuic,
    PreferTcp,
    ForceQuic,
    ForceTcp,
}

impl TransportPolicy {
    pub fn allows(self, transport_id: TransportId) -> bool {
        match self {
            Self::Auto | Self::PreferQuic | Self::PreferTcp => {
                is_selectable_transport(transport_id)
            }
            Self::ForceQuic => transport_id == TransportId::Quic,
            Self::ForceTcp => transport_id == TransportId::Tcp,
        }
    }

    fn preference_rank(self, transport_id: TransportId) -> u8 {
        match (self, transport_id) {
            (Self::PreferQuic | Self::ForceQuic, TransportId::Quic) => 0,
            (Self::PreferTcp | Self::ForceTcp, TransportId::Tcp) => 0,
            (Self::PreferQuic, TransportId::Tcp) => 1,
            (Self::PreferTcp, TransportId::Quic) => 1,
            (Self::Auto, TransportId::Quic) => 0,
            (Self::Auto, TransportId::Tcp) => 1,
            _ => u8::MAX,
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
    pub native_loader: bool,
    pub wasm: bool,
}

pub const fn compile_time_provider_features() -> ProviderFeatureSet {
    ProviderFeatureSet {
        tcp: cfg!(feature = "tcp"),
        quic: cfg!(feature = "quic"),
        native_loader: cfg!(feature = "native-loader"),
        wasm: cfg!(feature = "wasm"),
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
            diagnostic: Some(diagnostic.into()),
        }
    }

    pub fn with_library_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.library_path = Some(path.into());
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
    ) -> Result<TransportSelection, TransportSelectionError> {
        select_transport(self.providers(), remote, policy)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportSelection {
    pub selected: TransportProviderDescriptor,
    pub rejected: Vec<RejectedTransportCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeSample {
    pub transport_id: TransportId,
    pub provider_name: String,
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
        provider_name: impl Into<String>,
        elapsed_us: u64,
        rtt_us: u64,
        bytes_sent: u64,
        bytes_received: u64,
    ) -> Self {
        Self {
            transport_id,
            provider_name: provider_name.into(),
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
        provider_name: impl Into<String>,
        elapsed_us: u64,
        timed_out: bool,
    ) -> Self {
        Self {
            transport_id,
            provider_name: provider_name.into(),
            elapsed_us,
            rtt_us: None,
            bytes_sent: 0,
            bytes_received: 0,
            timed_out,
            failed: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProbeScore {
    pub sample_count: usize,
    pub failure_count: usize,
    pub failure_rate: f64,
    pub median_rtt_us: u64,
    pub throughput_bytes_per_sec: u64,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProbeCandidateScore {
    pub provider: TransportProviderDescriptor,
    pub probe_score: ProbeScore,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProbeSelection {
    pub selected: TransportProviderDescriptor,
    pub selected_score: ProbeScore,
    pub candidates: Vec<ProbeCandidateScore>,
    pub rejected: Vec<RejectedTransportCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedTransportCandidate {
    pub transport_id: TransportId,
    pub provider_name: Option<String>,
    pub reason: TransportRejectionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportRejectionReason {
    PolicyDisallowed,
    LocalProviderUnavailable,
    RemoteUnsupported,
    ProbeMissing,
    ProbeFailed,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransportSelectionError {
    #[error("forced transport is not available: {transport_id:?}")]
    ForcedTransportUnavailable { transport_id: TransportId },
    #[error("no viable transport provider after applying policy and remote support")]
    NoViableTransport {
        rejected: Vec<RejectedTransportCandidate>,
    },
}

pub fn select_transport(
    providers: &[TransportProviderDescriptor],
    remote: &RemoteTransportSupport,
    policy: TransportPolicy,
) -> Result<TransportSelection, TransportSelectionError> {
    let candidates = select_transport_candidates(providers, remote, policy);
    let mut viable = candidates.viable;
    viable.sort_by_key(|provider| {
        (
            policy.preference_rank(provider.transport_id),
            provider.name.clone(),
        )
    });

    match viable.into_iter().next() {
        Some(selected) => Ok(TransportSelection {
            selected,
            rejected: candidates.rejected,
        }),
        None => transport_selection_error(policy, candidates.rejected),
    }
}

pub fn select_transport_with_probe(
    providers: &[TransportProviderDescriptor],
    remote: &RemoteTransportSupport,
    policy: TransportPolicy,
    samples: &[ProbeSample],
) -> Result<ProbeSelection, TransportSelectionError> {
    let mut candidates = select_transport_candidates(providers, remote, policy);
    let mut scored = Vec::new();

    for provider in candidates.viable {
        let provider_samples = matching_probe_samples(&provider, samples).collect::<Vec<_>>();
        if provider_samples.is_empty() {
            candidates
                .rejected
                .push(rejection(&provider, TransportRejectionReason::ProbeMissing));
            continue;
        }

        match score_provider_probe(&provider, samples, policy) {
            Some(probe_score) if probe_score.failure_rate < 1.0 => {
                scored.push(ProbeCandidateScore {
                    provider,
                    probe_score,
                });
            }
            _ => candidates
                .rejected
                .push(rejection(&provider, TransportRejectionReason::ProbeFailed)),
        }
    }

    scored.sort_by(|left, right| {
        left.probe_score
            .score
            .total_cmp(&right.probe_score.score)
            .then_with(|| {
                policy
                    .preference_rank(left.provider.transport_id)
                    .cmp(&policy.preference_rank(right.provider.transport_id))
            })
            .then_with(|| left.provider.name.cmp(&right.provider.name))
    });

    match scored.first().cloned() {
        Some(selected) => Ok(ProbeSelection {
            selected: selected.provider,
            selected_score: selected.probe_score,
            candidates: scored,
            rejected: candidates.rejected,
        }),
        None => transport_selection_error(policy, candidates.rejected),
    }
}

pub fn score_provider_probe(
    provider: &TransportProviderDescriptor,
    samples: &[ProbeSample],
    policy: TransportPolicy,
) -> Option<ProbeScore> {
    let provider_samples = matching_probe_samples(provider, samples).collect::<Vec<_>>();
    if provider_samples.is_empty() {
        return None;
    }

    let failure_count = provider_samples
        .iter()
        .filter(|sample| sample.failed || sample.timed_out || sample.rtt_us.is_none())
        .count();
    let sample_count = provider_samples.len();
    let failure_rate = failure_count as f64 / sample_count as f64;
    let median_rtt_us = median_rtt_or_penalty(
        provider_samples
            .iter()
            .filter_map(|sample| sample.rtt_us)
            .collect(),
    );
    let elapsed_us = provider_samples
        .iter()
        .map(|sample| sample.elapsed_us)
        .sum::<u64>();
    let transferred = provider_samples
        .iter()
        .map(|sample| sample.bytes_sent.saturating_add(sample.bytes_received))
        .sum::<u64>();
    let throughput_bytes_per_sec = transferred
        .saturating_mul(1_000_000)
        .checked_div(elapsed_us)
        .unwrap_or(0);
    let throughput_bonus = (throughput_bytes_per_sec / 1_000).min(500) as f64;
    let policy_penalty = policy.preference_rank(provider.transport_id) as f64 * 1_000.0;
    let score =
        median_rtt_us as f64 + failure_rate * 10_000_000.0 + policy_penalty - throughput_bonus;

    Some(ProbeScore {
        sample_count,
        failure_count,
        failure_rate,
        median_rtt_us,
        throughput_bytes_per_sec,
        score,
    })
}

fn select_transport_candidates(
    providers: &[TransportProviderDescriptor],
    remote: &RemoteTransportSupport,
    policy: TransportPolicy,
) -> TransportCandidates {
    let mut viable = Vec::new();
    let mut rejections = Vec::new();

    for provider in providers {
        if !is_selectable_transport(provider.transport_id) {
            continue;
        }

        if !policy.allows(provider.transport_id) {
            rejections.push(rejection(
                provider,
                TransportRejectionReason::PolicyDisallowed,
            ));
        } else if !provider.available {
            rejections.push(rejection(
                provider,
                TransportRejectionReason::LocalProviderUnavailable,
            ));
        } else if !remote.supports(provider.transport_id) {
            rejections.push(rejection(
                provider,
                TransportRejectionReason::RemoteUnsupported,
            ));
        } else {
            viable.push(provider.clone());
        }
    }

    TransportCandidates {
        viable,
        rejected: rejections,
    }
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
struct TransportCandidates {
    viable: Vec<TransportProviderDescriptor>,
    rejected: Vec<RejectedTransportCandidate>,
}

fn matching_probe_samples<'a>(
    provider: &'a TransportProviderDescriptor,
    samples: &'a [ProbeSample],
) -> impl Iterator<Item = &'a ProbeSample> {
    samples.iter().filter(|sample| {
        sample.transport_id == provider.transport_id && sample.provider_name == provider.name
    })
}

fn median_rtt_or_penalty(mut rtts: Vec<u64>) -> u64 {
    const MISSING_RTT_PENALTY_US: u64 = 10_000_000;

    if rtts.is_empty() {
        return MISSING_RTT_PENALTY_US;
    }

    rtts.sort_unstable();
    rtts[rtts.len() / 2]
}

fn transport_selection_error<T>(
    policy: TransportPolicy,
    rejected: Vec<RejectedTransportCandidate>,
) -> Result<T, TransportSelectionError> {
    if matches!(
        policy,
        TransportPolicy::ForceQuic | TransportPolicy::ForceTcp
    ) {
        Err(TransportSelectionError::ForcedTransportUnavailable {
            transport_id: forced_transport(policy),
        })
    } else {
        Err(TransportSelectionError::NoViableTransport { rejected })
    }
}

fn rejection(
    provider: &TransportProviderDescriptor,
    reason: TransportRejectionReason,
) -> RejectedTransportCandidate {
    RejectedTransportCandidate {
        transport_id: provider.transport_id,
        provider_name: Some(provider.name.clone()),
        reason,
    }
}

fn forced_transport(policy: TransportPolicy) -> TransportId {
    match policy {
        TransportPolicy::ForceQuic => TransportId::Quic,
        TransportPolicy::ForceTcp => TransportId::Tcp,
        _ => TransportId::Unspecified,
    }
}

fn is_selectable_transport(transport_id: TransportId) -> bool {
    matches!(transport_id, TransportId::Quic | TransportId::Tcp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn registry_selects_policy_preferred_transport_from_remote_intersection() {
        let registry = TransportProviderRegistry::new()
            .with_provider(available("tcp", TransportId::Tcp))
            .with_provider(available("quic", TransportId::Quic));
        let remote = RemoteTransportSupport::new([TransportId::Tcp, TransportId::Quic]);

        let auto = registry
            .select(&remote, TransportPolicy::Auto)
            .expect("auto should select a provider");
        assert_eq!(auto.selected.transport_id, TransportId::Quic);

        let prefer_tcp = registry
            .select(&remote, TransportPolicy::PreferTcp)
            .expect("prefer tcp should select tcp");
        assert_eq!(prefer_tcp.selected.transport_id, TransportId::Tcp);
    }

    #[test]
    fn selection_rejects_missing_remote_and_policy_candidates() {
        let registry = TransportProviderRegistry::new()
            .with_provider(available("tcp", TransportId::Tcp))
            .with_provider(available("quic", TransportId::Quic));
        let remote = RemoteTransportSupport::new([TransportId::Tcp]);

        let selection = registry
            .select(&remote, TransportPolicy::PreferQuic)
            .expect("tcp should remain viable as fallback");
        assert_eq!(selection.selected.transport_id, TransportId::Tcp);
        assert_eq!(
            selection.rejected[0].reason,
            TransportRejectionReason::RemoteUnsupported
        );

        let forced = registry
            .select(&remote, TransportPolicy::ForceQuic)
            .expect_err("force quic should fail when remote lacks quic");
        assert!(matches!(
            forced,
            TransportSelectionError::ForcedTransportUnavailable {
                transport_id: TransportId::Quic
            }
        ));
    }

    #[test]
    fn selection_reports_unavailable_local_provider() {
        let registry = TransportProviderRegistry::new()
            .with_provider(missing("tcp", TransportId::Tcp))
            .with_provider(missing("quic", TransportId::Quic));
        let remote = RemoteTransportSupport::new([TransportId::Tcp, TransportId::Quic]);

        let error = registry
            .select(&remote, TransportPolicy::Auto)
            .expect_err("all providers are missing");
        match error {
            TransportSelectionError::NoViableTransport { rejected } => {
                assert_eq!(rejected.len(), 2);
                assert!(rejected.iter().all(
                    |entry| entry.reason == TransportRejectionReason::LocalProviderUnavailable
                ));
            }
            other => panic!("unexpected error: {other:?}"),
        }
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
                native_loader: cfg!(feature = "native-loader"),
                wasm: cfg!(feature = "wasm"),
            }
        );
    }

    #[test]
    fn probe_selection_prefers_lower_latency_viable_candidate() {
        let providers = [
            available("tcp", TransportId::Tcp),
            available("quic", TransportId::Quic),
        ];
        let remote = RemoteTransportSupport::new([TransportId::Tcp, TransportId::Quic]);
        let samples = [
            ProbeSample::success(TransportId::Tcp, "tcp", 10_000, 900, 256, 256),
            ProbeSample::success(TransportId::Tcp, "tcp", 10_000, 1_100, 256, 256),
            ProbeSample::success(TransportId::Quic, "quic", 10_000, 4_000, 256, 256),
            ProbeSample::success(TransportId::Quic, "quic", 10_000, 5_000, 256, 256),
        ];

        let selection =
            select_transport_with_probe(&providers, &remote, TransportPolicy::Auto, &samples)
                .expect("probe selection should pick a viable candidate");

        assert_eq!(selection.selected.transport_id, TransportId::Tcp);
        assert_eq!(selection.candidates.len(), 2);
        assert!(selection.selected_score.score < selection.candidates[1].probe_score.score);
    }

    #[test]
    fn probe_selection_downgrades_flaky_preferred_quic() {
        let providers = [
            available("tcp", TransportId::Tcp),
            available("quic", TransportId::Quic),
        ];
        let remote = RemoteTransportSupport::new([TransportId::Tcp, TransportId::Quic]);
        let samples = [
            ProbeSample::success(TransportId::Tcp, "tcp", 20_000, 5_000, 512, 512),
            ProbeSample::success(TransportId::Tcp, "tcp", 20_000, 5_200, 512, 512),
            ProbeSample::success(TransportId::Quic, "quic", 20_000, 800, 512, 512),
            ProbeSample::failure(TransportId::Quic, "quic", 20_000, true),
        ];

        let selection =
            select_transport_with_probe(&providers, &remote, TransportPolicy::PreferQuic, &samples)
                .expect("tcp should win when preferred quic is flaky");

        assert_eq!(selection.selected.transport_id, TransportId::Tcp);
        let quic = selection
            .candidates
            .iter()
            .find(|candidate| candidate.provider.transport_id == TransportId::Quic)
            .expect("quic should still be scored");
        assert_eq!(quic.probe_score.failure_count, 1);
    }

    #[test]
    fn probe_selection_reports_missing_and_failed_probe_candidates() {
        let providers = [
            available("tcp", TransportId::Tcp),
            available("quic", TransportId::Quic),
        ];
        let remote = RemoteTransportSupport::new([TransportId::Tcp, TransportId::Quic]);
        let samples = [ProbeSample::failure(
            TransportId::Quic,
            "quic",
            30_000,
            true,
        )];

        let error =
            select_transport_with_probe(&providers, &remote, TransportPolicy::Auto, &samples)
                .expect_err("no probe candidate is viable");

        match error {
            TransportSelectionError::NoViableTransport { rejected } => {
                assert_eq!(rejected.len(), 2);
                assert!(rejected
                    .iter()
                    .any(|entry| entry.transport_id == TransportId::Tcp
                        && entry.reason == TransportRejectionReason::ProbeMissing));
                assert!(rejected
                    .iter()
                    .any(|entry| entry.transport_id == TransportId::Quic
                        && entry.reason == TransportRejectionReason::ProbeFailed));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn forced_probe_selection_fails_when_forced_transport_probe_fails() {
        let providers = [available("quic", TransportId::Quic)];
        let remote = RemoteTransportSupport::new([TransportId::Quic]);
        let samples = [ProbeSample::failure(
            TransportId::Quic,
            "quic",
            30_000,
            true,
        )];

        let error =
            select_transport_with_probe(&providers, &remote, TransportPolicy::ForceQuic, &samples)
                .expect_err("forced quic should fail on failed probe");

        assert!(matches!(
            error,
            TransportSelectionError::ForcedTransportUnavailable {
                transport_id: TransportId::Quic
            }
        ));
    }

    #[test]
    fn score_provider_probe_returns_none_without_matching_samples() {
        let provider = available("tcp", TransportId::Tcp);
        let samples = [ProbeSample::success(
            TransportId::Quic,
            "quic",
            10_000,
            1_000,
            128,
            128,
        )];

        assert_eq!(
            score_provider_probe(&provider, &samples, TransportPolicy::Auto),
            None
        );
    }

    fn available(name: &str, transport_id: TransportId) -> TransportProviderDescriptor {
        TransportProviderDescriptor::available(
            name,
            "0.0.0",
            transport_id,
            TransportProviderKind::PureRust,
        )
    }

    fn missing(name: &str, transport_id: TransportId) -> TransportProviderDescriptor {
        TransportProviderDescriptor::missing(
            name,
            "0.0.0",
            transport_id,
            TransportProviderKind::PureRust,
            "not installed",
        )
    }
}
