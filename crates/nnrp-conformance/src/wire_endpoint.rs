use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
};

use http::Uri;
use nnrp_runtime::{
    NnrpClient, NnrpClientConfig, NnrpServer, NnrpServerConfig, RuntimeError, RuntimeFrameLimits,
    RuntimeTransportKind,
};
use nnrp_transport_ipc::{IpcEndpoint, IpcProvider};
use nnrp_transport_quic::{
    quic_client_config, quic_server_config, QuicClientEndpointConfig, QuicProvider,
    QuicServerEndpointConfig,
};
use nnrp_transport_websocket::{
    WebSocketEndpoint, WebSocketFramedListener, WebSocketProvider, WebSocketTransport,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceTransport {
    Tcp,
    Ipc,
    Quic,
    WebSocket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireEndpointSecurity {
    pub server_name: String,
    pub trusted_certificate_der: Vec<u8>,
    pub certificate_der: Vec<u8>,
    pub private_key_pkcs8_der: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireReferenceEndpoint {
    pub transport: ReferenceTransport,
    pub endpoint: String,
    pub security: Option<WireEndpointSecurity>,
}

impl ReferenceTransport {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Ipc => "ipc",
            Self::Quic => "quic",
            Self::WebSocket => "websocket",
        }
    }
}

impl WireReferenceEndpoint {
    pub fn plain(transport: ReferenceTransport, endpoint: impl Into<String>) -> Self {
        Self {
            transport,
            endpoint: endpoint.into(),
            security: None,
        }
    }

    pub fn secure(
        transport: ReferenceTransport,
        endpoint: impl Into<String>,
        security: WireEndpointSecurity,
    ) -> Self {
        Self {
            transport,
            endpoint: endpoint.into(),
            security: Some(security),
        }
    }

    pub fn validate(&self) -> Result<(), RuntimeError> {
        if self.endpoint.is_empty() {
            return Err(RuntimeError::UnsupportedTransport(
                "wire reference endpoint cannot be empty",
            ));
        }
        match self.transport {
            ReferenceTransport::Tcp => {
                self.require_plain()?;
                parse_socket_addr(&self.endpoint)?;
            }
            ReferenceTransport::Ipc => {
                self.require_plain()?;
                IpcEndpoint::from_str(&self.endpoint)?;
            }
            ReferenceTransport::Quic => {
                self.require_security()?;
                parse_socket_addr(&self.endpoint)?;
            }
            ReferenceTransport::WebSocket => {
                let endpoint = WebSocketEndpoint::from_str(&self.endpoint)?;
                if endpoint.is_secure() {
                    self.require_security()?;
                } else {
                    self.require_plain()?;
                }
                websocket_bind_address(&endpoint)?;
            }
        }
        Ok(())
    }

    pub async fn connect(&self) -> Result<NnrpClient, RuntimeError> {
        self.validate()?;
        match self.transport {
            ReferenceTransport::Tcp => {
                NnrpClient::connect_tcp(&self.endpoint, NnrpClientConfig::default()).await
            }
            ReferenceTransport::Ipc => {
                let endpoint = IpcEndpoint::from_str(&self.endpoint)?;
                IpcProvider::connect(&endpoint, NnrpClientConfig::default()).await
            }
            ReferenceTransport::Quic => {
                let security = self.require_security()?;
                let endpoint_config = QuicClientEndpointConfig::with_root_certificate(
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
                    security.server_name.clone(),
                    security.trusted_certificate_der.clone(),
                );
                QuicProvider::connect(
                    &self.endpoint,
                    endpoint_config,
                    quic_client_config(NnrpClientConfig::default()),
                )
                .await
            }
            ReferenceTransport::WebSocket => {
                let endpoint = WebSocketEndpoint::from_str(&self.endpoint)?;
                if endpoint.is_secure() {
                    let security = self.require_security()?;
                    let transport = WebSocketTransport::connect_secure_with_limits(
                        &endpoint,
                        &security.server_name,
                        security.trusted_certificate_der.clone(),
                        RuntimeFrameLimits::default(),
                    )
                    .await?;
                    NnrpClient::from_transport(
                        transport,
                        NnrpClientConfig::default().with_transport(RuntimeTransportKind::WebSocket),
                    )
                } else {
                    WebSocketProvider::connect(&endpoint, NnrpClientConfig::default()).await
                }
            }
        }
    }

    pub async fn bind(&self) -> Result<NnrpServer, RuntimeError> {
        self.validate()?;
        match self.transport {
            ReferenceTransport::Tcp => {
                NnrpServer::bind_tcp(&self.endpoint, NnrpServerConfig::default()).await
            }
            ReferenceTransport::Ipc => {
                let endpoint = IpcEndpoint::from_str(&self.endpoint)?;
                IpcProvider::bind(&endpoint, NnrpServerConfig::default()).await
            }
            ReferenceTransport::Quic => {
                let security = self.require_security()?;
                let endpoint_config = QuicServerEndpointConfig::with_single_certificate(
                    parse_socket_addr(&self.endpoint)?,
                    security.certificate_der.clone(),
                    security.private_key_pkcs8_der.clone(),
                );
                QuicProvider::bind(
                    endpoint_config,
                    quic_server_config(NnrpServerConfig::default()),
                )
                .await
            }
            ReferenceTransport::WebSocket => {
                let endpoint = WebSocketEndpoint::from_str(&self.endpoint)?;
                let bind_address = websocket_bind_address(&endpoint)?;
                if endpoint.is_secure() {
                    let security = self.require_security()?;
                    let listener = WebSocketFramedListener::bind_secure_with_limits(
                        bind_address,
                        security.certificate_der.clone(),
                        security.private_key_pkcs8_der.clone(),
                        RuntimeFrameLimits::default(),
                    )
                    .await?;
                    NnrpServer::from_listener(
                        listener,
                        NnrpServerConfig::default().with_transport(RuntimeTransportKind::WebSocket),
                    )
                } else {
                    WebSocketProvider::bind(bind_address, NnrpServerConfig::default()).await
                }
            }
        }
    }

    fn require_plain(&self) -> Result<(), RuntimeError> {
        if self.security.is_some() {
            return Err(RuntimeError::UnsupportedTransport(
                "plain wire endpoint cannot declare TLS security material",
            ));
        }
        Ok(())
    }

    fn require_security(&self) -> Result<&WireEndpointSecurity, RuntimeError> {
        let security = self
            .security
            .as_ref()
            .ok_or(RuntimeError::UnsupportedTransport(
                "secure wire endpoint requires TLS security material",
            ))?;
        if security.server_name.is_empty()
            || security.trusted_certificate_der.is_empty()
            || security.certificate_der.is_empty()
            || security.private_key_pkcs8_der.is_empty()
        {
            return Err(RuntimeError::UnsupportedTransport(
                "wire endpoint TLS security material cannot be empty",
            ));
        }
        Ok(security)
    }
}

fn parse_socket_addr(endpoint: &str) -> Result<SocketAddr, RuntimeError> {
    endpoint.parse().map_err(|_| {
        RuntimeError::UnsupportedTransport("wire endpoint must be a numeric socket address")
    })
}

fn websocket_bind_address(endpoint: &WebSocketEndpoint) -> Result<String, RuntimeError> {
    let uri = endpoint
        .as_str()
        .parse::<Uri>()
        .map_err(|_| RuntimeError::UnsupportedTransport("WebSocket endpoint URI is invalid"))?;
    let host =
        uri.host()
            .filter(|host| !host.is_empty())
            .ok_or(RuntimeError::UnsupportedTransport(
                "WebSocket endpoint URI must contain a host",
            ))?;
    let port = uri
        .port_u16()
        .unwrap_or(if endpoint.is_secure() { 443 } else { 80 });
    Ok(format!("{host}:{port}"))
}

#[cfg(test)]
mod tests {
    use super::{ReferenceTransport, WireEndpointSecurity, WireReferenceEndpoint};

    fn security() -> WireEndpointSecurity {
        WireEndpointSecurity {
            server_name: "localhost".to_string(),
            trusted_certificate_der: vec![1],
            certificate_der: vec![2],
            private_key_pkcs8_der: vec![3],
        }
    }

    #[test]
    fn endpoint_validation_enforces_transport_security_boundary() {
        WireReferenceEndpoint::plain(ReferenceTransport::Tcp, "127.0.0.1:19091")
            .validate()
            .expect("plain TCP endpoint should validate");
        WireReferenceEndpoint::plain(ReferenceTransport::Ipc, "unix:///tmp/nnrp.sock")
            .validate()
            .expect("plain IPC endpoint should validate");
        WireReferenceEndpoint::plain(ReferenceTransport::WebSocket, "ws://127.0.0.1:19093/nnrp")
            .validate()
            .expect("plain WebSocket endpoint should validate");
        WireReferenceEndpoint::secure(ReferenceTransport::Quic, "127.0.0.1:19092", security())
            .validate()
            .expect("secure QUIC endpoint should validate");
        WireReferenceEndpoint::secure(
            ReferenceTransport::WebSocket,
            "wss://localhost:19094/nnrp",
            security(),
        )
        .validate()
        .expect("secure WebSocket endpoint should validate");

        assert!(
            WireReferenceEndpoint::plain(ReferenceTransport::Quic, "127.0.0.1:19092")
                .validate()
                .is_err()
        );
        assert!(WireReferenceEndpoint::secure(
            ReferenceTransport::Tcp,
            "127.0.0.1:19091",
            security(),
        )
        .validate()
        .is_err());
    }
}
