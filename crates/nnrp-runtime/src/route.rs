use std::{collections::BTreeMap, fmt, str::FromStr};

use http::Uri;
use nnrp_core::TransportId;
use thiserror::Error;

pub type ClientProviderRoutes = BTreeMap<TransportId, ClientProviderRoute>;
pub type ServerProviderRoutes = BTreeMap<TransportId, ServerProviderRoute>;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RouteConfigurationError {
    #[error("endpoint is not a valid absolute URI")]
    InvalidEndpoint,

    #[error("application endpoint must use nnrp:// or nnrps://, not {0}://")]
    UnsupportedApplicationScheme(String),

    #[error("application endpoint credentials are not allowed")]
    ApplicationCredentialsNotAllowed,

    #[error("endpoint fragments are not allowed")]
    FragmentNotAllowed,

    #[error("provider endpoint must use tcp://, quic://, unix://, npipe://, ws://, or wss://")]
    UnsupportedProviderScheme,

    #[error("provider endpoint locator is empty")]
    EmptyProviderLocator,

    #[error("client transport security server_name must not be empty")]
    EmptyServerName,

    #[error("client transport security trusted_certificate_der must not be empty")]
    EmptyTrustedCertificate,

    #[error("server transport security certificate_der must not be empty")]
    EmptyCertificate,

    #[error("server transport security private_key_pkcs8_der must not be empty")]
    EmptyPrivateKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NnrpEndpoint {
    value: String,
    uri: Uri,
}

impl NnrpEndpoint {
    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn authority(&self) -> &str {
        self.uri
            .authority()
            .expect("validated NNRP endpoints always have an authority")
            .as_str()
    }

    pub fn path_and_query(&self) -> &str {
        self.uri
            .path_and_query()
            .map(http::uri::PathAndQuery::as_str)
            .unwrap_or("/")
    }

    pub fn host(&self) -> &str {
        self.uri
            .authority()
            .expect("validated NNRP endpoints always have an authority")
            .host()
    }

    pub fn port(&self) -> Option<u16> {
        self.uri
            .authority()
            .and_then(http::uri::Authority::port_u16)
    }

    pub fn is_secure(&self) -> bool {
        self.uri.scheme_str() == Some("nnrps")
    }
}

impl FromStr for NnrpEndpoint {
    type Err = RouteConfigurationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.contains('#') {
            return Err(RouteConfigurationError::FragmentNotAllowed);
        }
        let uri = Uri::from_str(value).map_err(|_| RouteConfigurationError::InvalidEndpoint)?;
        match uri.scheme_str() {
            None => return Err(RouteConfigurationError::InvalidEndpoint),
            Some("nnrp" | "nnrps") => {}
            Some(other) => {
                return Err(RouteConfigurationError::UnsupportedApplicationScheme(
                    other.to_owned(),
                ));
            }
        }
        let Some(authority) = uri.authority() else {
            return Err(RouteConfigurationError::InvalidEndpoint);
        };
        if authority.as_str().contains('@') {
            return Err(RouteConfigurationError::ApplicationCredentialsNotAllowed);
        }
        Ok(Self {
            value: value.to_owned(),
            uri,
        })
    }
}

impl fmt::Display for NnrpEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEndpoint {
    value: String,
    scheme_end: usize,
}

impl ProviderEndpoint {
    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn scheme(&self) -> &str {
        &self.value[..self.scheme_end]
    }

    pub fn is_secure(&self) -> bool {
        self.scheme() == "wss"
    }

    pub fn matches_transport(&self, transport_id: TransportId) -> bool {
        matches!(
            (transport_id, self.scheme()),
            (TransportId::Tcp, "tcp")
                | (TransportId::Quic, "quic")
                | (TransportId::Ipc, "unix" | "npipe")
                | (TransportId::WebSocket, "ws" | "wss")
        )
    }
}

impl FromStr for ProviderEndpoint {
    type Err = RouteConfigurationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some(scheme_end) = value.find("://") else {
            return Err(RouteConfigurationError::InvalidEndpoint);
        };
        let scheme = &value[..scheme_end];
        if !matches!(scheme, "tcp" | "quic" | "unix" | "npipe" | "ws" | "wss") {
            return Err(RouteConfigurationError::UnsupportedProviderScheme);
        }
        let locator = &value[scheme_end + 3..];
        if locator.is_empty() {
            return Err(RouteConfigurationError::EmptyProviderLocator);
        }
        if value.contains('#') {
            return Err(RouteConfigurationError::FragmentNotAllowed);
        }
        match scheme {
            "unix" => {
                if !locator.starts_with('/') || locator == "/" || locator.contains('?') {
                    return Err(RouteConfigurationError::InvalidEndpoint);
                }
            }
            "npipe" => {
                if locator.contains(['?', '@']) {
                    return Err(RouteConfigurationError::InvalidEndpoint);
                }
            }
            _ => {
                let uri =
                    Uri::from_str(value).map_err(|_| RouteConfigurationError::InvalidEndpoint)?;
                let Some(authority) = uri.authority() else {
                    return Err(RouteConfigurationError::EmptyProviderLocator);
                };
                if authority.as_str().contains('@') {
                    return Err(RouteConfigurationError::InvalidEndpoint);
                }
            }
        }
        Ok(Self {
            value: value.to_owned(),
            scheme_end,
        })
    }
}

impl fmt::Display for ProviderEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientTransportSecurity {
    pub server_name: String,
    pub trusted_certificate_der: Vec<u8>,
}

impl ClientTransportSecurity {
    pub fn new(
        server_name: impl Into<String>,
        trusted_certificate_der: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            trusted_certificate_der: trusted_certificate_der.into(),
        }
    }

    pub fn validate(&self) -> Result<(), RouteConfigurationError> {
        if self.server_name.trim().is_empty() {
            return Err(RouteConfigurationError::EmptyServerName);
        }
        if self.trusted_certificate_der.is_empty() {
            return Err(RouteConfigurationError::EmptyTrustedCertificate);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerTransportSecurity {
    pub certificate_der: Vec<u8>,
    pub private_key_pkcs8_der: Vec<u8>,
}

impl ServerTransportSecurity {
    pub fn new(
        certificate_der: impl Into<Vec<u8>>,
        private_key_pkcs8_der: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            certificate_der: certificate_der.into(),
            private_key_pkcs8_der: private_key_pkcs8_der.into(),
        }
    }

    pub fn validate(&self) -> Result<(), RouteConfigurationError> {
        if self.certificate_der.is_empty() {
            return Err(RouteConfigurationError::EmptyCertificate);
        }
        if self.private_key_pkcs8_der.is_empty() {
            return Err(RouteConfigurationError::EmptyPrivateKey);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientProviderRoute {
    pub provider_endpoint: Option<ProviderEndpoint>,
    pub security: Option<ClientTransportSecurity>,
}

impl ClientProviderRoute {
    pub fn at(provider_endpoint: ProviderEndpoint) -> Self {
        Self {
            provider_endpoint: Some(provider_endpoint),
            security: None,
        }
    }

    pub fn native_tls(
        server_name: impl Into<String>,
        trusted_certificate_der: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            provider_endpoint: None,
            security: Some(ClientTransportSecurity::new(
                server_name,
                trusted_certificate_der,
            )),
        }
    }

    pub fn validate_for(&self, transport_id: TransportId) -> Result<(), RouteConfigurationError> {
        if let Some(endpoint) = &self.provider_endpoint {
            if !endpoint.matches_transport(transport_id) {
                return Err(RouteConfigurationError::UnsupportedProviderScheme);
            }
        }
        if let Some(security) = &self.security {
            security.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerProviderRoute {
    pub provider_endpoint: Option<ProviderEndpoint>,
    pub security: Option<ServerTransportSecurity>,
}

impl ServerProviderRoute {
    pub fn at(provider_endpoint: ProviderEndpoint) -> Self {
        Self {
            provider_endpoint: Some(provider_endpoint),
            security: None,
        }
    }

    pub fn native_tls(
        certificate_der: impl Into<Vec<u8>>,
        private_key_pkcs8_der: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            provider_endpoint: None,
            security: Some(ServerTransportSecurity::new(
                certificate_der,
                private_key_pkcs8_der,
            )),
        }
    }

    pub fn validate_for(&self, transport_id: TransportId) -> Result<(), RouteConfigurationError> {
        if let Some(endpoint) = &self.provider_endpoint {
            if !endpoint.matches_transport(transport_id) {
                return Err(RouteConfigurationError::UnsupportedProviderScheme);
            }
        }
        if let Some(security) = &self.security {
            security.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_endpoint_preserves_frozen_components() {
        let endpoint = "nnrps://runtime.example:4433/session/default?model=1"
            .parse::<NnrpEndpoint>()
            .unwrap();
        assert!(endpoint.is_secure());
        assert_eq!(endpoint.authority(), "runtime.example:4433");
        assert_eq!(endpoint.host(), "runtime.example");
        assert_eq!(endpoint.port(), Some(4433));
        assert_eq!(endpoint.path_and_query(), "/session/default?model=1");
    }

    #[test]
    fn application_endpoint_rejects_carrier_schemes_and_unsafe_parts() {
        assert_eq!(
            "tcp://runtime.example:4433"
                .parse::<NnrpEndpoint>()
                .unwrap_err(),
            RouteConfigurationError::UnsupportedApplicationScheme("tcp".to_owned())
        );
        assert_eq!(
            "nnrp://user@runtime.example/session"
                .parse::<NnrpEndpoint>()
                .unwrap_err(),
            RouteConfigurationError::ApplicationCredentialsNotAllowed
        );
        assert_eq!(
            "nnrp://runtime.example/session#fragment"
                .parse::<NnrpEndpoint>()
                .unwrap_err(),
            RouteConfigurationError::FragmentNotAllowed
        );
        assert_eq!(
            "nnrp:/session".parse::<NnrpEndpoint>().unwrap_err(),
            RouteConfigurationError::InvalidEndpoint
        );
        assert_eq!(
            "not an endpoint".parse::<NnrpEndpoint>().unwrap_err(),
            RouteConfigurationError::InvalidEndpoint
        );

        let insecure = "nnrp://runtime.example/session"
            .parse::<NnrpEndpoint>()
            .unwrap();
        assert!(!insecure.is_secure());
        assert_eq!(insecure.port(), None);
        assert_eq!(insecure.to_string(), "nnrp://runtime.example/session");
    }

    #[test]
    fn provider_endpoint_accepts_only_frozen_carrier_schemes() {
        for value in [
            "tcp://127.0.0.1:4433",
            "quic://127.0.0.1:4433",
            "unix:///run/nnrp/runtime.sock",
            "npipe://nnrp-runtime",
            "ws://runtime.example/nnrp",
            "wss://runtime.example/nnrp",
        ] {
            let endpoint = value
                .parse::<ProviderEndpoint>()
                .unwrap_or_else(|error| panic!("{value}: {error}"));
            assert_eq!(endpoint.as_str(), value);
        }
        assert!("nnrp://runtime.example"
            .parse::<ProviderEndpoint>()
            .is_err());
        assert!("unix://".parse::<ProviderEndpoint>().is_err());
        assert!("wss://runtime.example/nnrp"
            .parse::<ProviderEndpoint>()
            .unwrap()
            .is_secure());
        assert_eq!(
            "ws://runtime.example/nnrp"
                .parse::<ProviderEndpoint>()
                .unwrap()
                .scheme(),
            "ws"
        );
        for invalid in [
            "ws://",
            "unix://relative.sock",
            "unix:///",
            "unix:///run/nnrp.sock?query=1",
            "npipe://name?query=1",
            "tcp://user@runtime.example:4433",
            "wss://runtime.example/nnrp#fragment",
            "not-an-endpoint",
        ] {
            assert!(invalid.parse::<ProviderEndpoint>().is_err(), "{invalid}");
        }
    }

    #[test]
    fn provider_routes_keep_locator_and_security_role_local() {
        let client = ClientProviderRoute {
            provider_endpoint: Some("wss://runtime.example/nnrp".parse().unwrap()),
            security: Some(ClientTransportSecurity::new("runtime.example", [1, 2, 3])),
        };
        client.validate_for(TransportId::WebSocket).unwrap();
        assert_eq!(
            client.validate_for(TransportId::Tcp).unwrap_err(),
            RouteConfigurationError::UnsupportedProviderScheme
        );

        let server = ServerProviderRoute {
            provider_endpoint: Some("quic://127.0.0.1:4433".parse().unwrap()),
            security: Some(ServerTransportSecurity::new([1, 2], [3, 4])),
        };
        server.validate_for(TransportId::Quic).unwrap();

        let client_at = ClientProviderRoute::at("tcp://127.0.0.1:4433".parse().unwrap());
        client_at.validate_for(TransportId::Tcp).unwrap();
        let client_tls = ClientProviderRoute::native_tls("runtime.example", [1]);
        client_tls.validate_for(TransportId::Quic).unwrap();
        let server_at = ServerProviderRoute::at("npipe://nnrp-runtime".parse().unwrap());
        server_at.validate_for(TransportId::Ipc).unwrap();
        let server_tls = ServerProviderRoute::native_tls([1], [2]);
        server_tls.validate_for(TransportId::Tcp).unwrap();
        assert_eq!(ClientProviderRoute::default().security, None);
        assert_eq!(ServerProviderRoute::default().security, None);
    }

    #[test]
    fn route_security_rejects_empty_owned_fields() {
        assert_eq!(
            ClientTransportSecurity::new(" ", [1]).validate(),
            Err(RouteConfigurationError::EmptyServerName)
        );
        assert_eq!(
            ClientTransportSecurity::new("runtime.example", []).validate(),
            Err(RouteConfigurationError::EmptyTrustedCertificate)
        );
        assert_eq!(
            ServerTransportSecurity::new([], [1]).validate(),
            Err(RouteConfigurationError::EmptyCertificate)
        );
        assert_eq!(
            ServerTransportSecurity::new([1], []).validate(),
            Err(RouteConfigurationError::EmptyPrivateKey)
        );
    }
}
