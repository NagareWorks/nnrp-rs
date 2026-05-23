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

    viable.sort_by_key(|provider| {
        (
            policy.preference_rank(provider.transport_id),
            provider.name.clone(),
        )
    });

    match viable.into_iter().next() {
        Some(selected) => Ok(TransportSelection {
            selected,
            rejected: rejections,
        }),
        None if matches!(
            policy,
            TransportPolicy::ForceQuic | TransportPolicy::ForceTcp
        ) =>
        {
            Err(TransportSelectionError::ForcedTransportUnavailable {
                transport_id: forced_transport(policy),
            })
        }
        None => Err(TransportSelectionError::NoViableTransport {
            rejected: rejections,
        }),
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
