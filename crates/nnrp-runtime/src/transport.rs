use async_trait::async_trait;
use nnrp_core::{CommonHeader, COMMON_HEADER_LEN};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, ToSocketAddrs},
};

use crate::{RuntimeError, RuntimePacket};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTransportKind {
    Tcp,
    Quic,
}

impl RuntimeTransportKind {
    pub fn transport_id(self) -> nnrp_core::TransportId {
        match self {
            Self::Tcp => nnrp_core::TransportId::Tcp,
            Self::Quic => nnrp_core::TransportId::Quic,
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
}

impl TcpTransport {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    pub async fn connect(addr: impl ToSocketAddrs) -> Result<Self, RuntimeError> {
        Ok(Self {
            stream: TcpStream::connect(addr).await?,
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
        self.stream.write_all(&packet.to_bytes()?).await?;
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
}

impl TcpFramedListener {
    pub fn new(listener: TcpListener) -> Self {
        Self { listener }
    }

    pub async fn bind(addr: impl ToSocketAddrs) -> Result<Self, RuntimeError> {
        Ok(Self {
            listener: TcpListener::bind(addr).await?,
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
        Ok(Box::new(TcpTransport::new(stream)))
    }
}
