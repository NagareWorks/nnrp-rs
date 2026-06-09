use async_trait::async_trait;
use nnrp_core::{CommonHeader, COMMON_HEADER_LEN};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, ToSocketAddrs},
};

use crate::{RuntimeError, RuntimePacket};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeFrameLimits {
    pub max_packet_bytes: usize,
}

impl RuntimeFrameLimits {
    pub const DEFAULT_MAX_PACKET_BYTES: usize = 64 * 1024 * 1024;

    pub const fn new(max_packet_bytes: usize) -> Self {
        Self { max_packet_bytes }
    }

    pub fn validate_packet_len(&self, declared: usize) -> Result<(), RuntimeError> {
        if declared > self.max_packet_bytes {
            Err(RuntimeError::FrameTooLarge {
                declared,
                max: self.max_packet_bytes,
            })
        } else {
            Ok(())
        }
    }
}

impl Default for RuntimeFrameLimits {
    fn default() -> Self {
        Self::new(Self::DEFAULT_MAX_PACKET_BYTES)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTransportKind {
    Tcp,
    Quic,
    Ipc,
    WebSocket,
}

impl RuntimeTransportKind {
    pub fn transport_id(self) -> nnrp_core::TransportId {
        match self {
            Self::Tcp => nnrp_core::TransportId::Tcp,
            Self::Quic => nnrp_core::TransportId::Quic,
            Self::Ipc => nnrp_core::TransportId::Ipc,
            Self::WebSocket => nnrp_core::TransportId::WebSocket,
        }
    }
}

pub type BoxedFramedTransport = Box<dyn FramedTransport>;
pub type BoxedFramedListener = Box<dyn FramedListener>;

#[async_trait]
pub trait FramedTransport: Send {
    fn transport_kind(&self) -> RuntimeTransportKind;
    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError>;
    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError>;
    async fn close(&mut self) -> Result<(), RuntimeError>;
}

#[async_trait]
pub trait FramedListener: Send + Sync {
    fn transport_kind(&self) -> RuntimeTransportKind;
    fn local_addr(&self) -> Result<std::net::SocketAddr, RuntimeError>;
    async fn accept(&self) -> Result<BoxedFramedTransport, RuntimeError>;
}

#[derive(Debug)]
pub struct TcpTransport {
    stream: TcpStream,
    limits: RuntimeFrameLimits,
}

impl TcpTransport {
    pub fn new(stream: TcpStream) -> Self {
        Self::new_with_limits(stream, RuntimeFrameLimits::default())
    }

    pub fn new_with_limits(stream: TcpStream, limits: RuntimeFrameLimits) -> Self {
        Self { stream, limits }
    }

    pub async fn connect(addr: impl ToSocketAddrs) -> Result<Self, RuntimeError> {
        Self::connect_with_limits(addr, RuntimeFrameLimits::default()).await
    }

    pub async fn connect_with_limits(
        addr: impl ToSocketAddrs,
        limits: RuntimeFrameLimits,
    ) -> Result<Self, RuntimeError> {
        Ok(Self {
            stream: TcpStream::connect(addr).await?,
            limits,
        })
    }
}

#[async_trait]
impl FramedTransport for TcpTransport {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::Tcp
    }

    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
        let mut header_bytes = [0u8; COMMON_HEADER_LEN];
        self.stream.read_exact(&mut header_bytes).await?;
        let header = CommonHeader::parse(&header_bytes)?;
        self.limits.validate_packet_len(header.packet_len()?)?;

        let mut metadata = vec![0u8; header.meta_len as usize];
        if !metadata.is_empty() {
            self.stream.read_exact(&mut metadata).await?;
        }

        let mut body = vec![0u8; header.body_len as usize];
        if !body.is_empty() {
            self.stream.read_exact(&mut body).await?;
        }

        Ok(RuntimePacket::from_parts(header, metadata, body)?)
    }

    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
        let bytes = packet.to_bytes()?;
        self.limits.validate_packet_len(bytes.len())?;
        self.stream.write_all(&bytes).await?;
        self.stream.flush().await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RuntimeError> {
        self.stream.shutdown().await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct TcpFramedListener {
    listener: TcpListener,
    limits: RuntimeFrameLimits,
}

impl TcpFramedListener {
    pub fn new(listener: TcpListener) -> Self {
        Self::new_with_limits(listener, RuntimeFrameLimits::default())
    }

    pub fn new_with_limits(listener: TcpListener, limits: RuntimeFrameLimits) -> Self {
        Self { listener, limits }
    }

    pub async fn bind(addr: impl ToSocketAddrs) -> Result<Self, RuntimeError> {
        Self::bind_with_limits(addr, RuntimeFrameLimits::default()).await
    }

    pub async fn bind_with_limits(
        addr: impl ToSocketAddrs,
        limits: RuntimeFrameLimits,
    ) -> Result<Self, RuntimeError> {
        Ok(Self {
            listener: TcpListener::bind(addr).await?,
            limits,
        })
    }
}

#[async_trait]
impl FramedListener for TcpFramedListener {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::Tcp
    }

    fn local_addr(&self) -> Result<std::net::SocketAddr, RuntimeError> {
        Ok(self.listener.local_addr()?)
    }

    async fn accept(&self) -> Result<BoxedFramedTransport, RuntimeError> {
        let (stream, _) = self.listener.accept().await?;
        Ok(Box::new(TcpTransport::new_with_limits(stream, self.limits)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnrp_core::TransportId;

    #[test]
    fn runtime_transport_kinds_map_to_frozen_transport_ids() {
        assert_eq!(RuntimeTransportKind::Tcp.transport_id(), TransportId::Tcp);
        assert_eq!(RuntimeTransportKind::Quic.transport_id(), TransportId::Quic);
        assert_eq!(RuntimeTransportKind::Ipc.transport_id(), TransportId::Ipc);
        assert_eq!(
            RuntimeTransportKind::WebSocket.transport_id(),
            TransportId::WebSocket
        );
    }
}
