use async_trait::async_trait;
use nnrp_core::{CommonHeader, COMMON_HEADER_LEN};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
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

const STREAM_READ_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Default)]
pub struct StreamPacketReader {
    buffered: Vec<u8>,
    consumed: usize,
    packet_len: Option<usize>,
}

impl StreamPacketReader {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn read_packet<R>(
        &mut self,
        reader: &mut R,
        limits: RuntimeFrameLimits,
    ) -> Result<RuntimePacket, RuntimeError>
    where
        R: AsyncRead + Unpin,
    {
        loop {
            if self.packet_len.is_none() && self.available() >= COMMON_HEADER_LEN {
                let header = CommonHeader::parse(
                    &self.buffered[self.consumed..self.consumed + COMMON_HEADER_LEN],
                )?;
                let packet_len = header.packet_len()?;
                limits.validate_packet_len(packet_len)?;
                self.packet_len = Some(packet_len);
            }

            if let Some(packet_len) = self.packet_len {
                if self.available() >= packet_len {
                    return self.take_packet(packet_len);
                }
            }

            let mut chunk = [0u8; STREAM_READ_CHUNK_BYTES];
            let read = reader.read(&mut chunk).await?;
            if read == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "stream closed before a complete NNRP packet",
                )
                .into());
            }
            self.buffered.extend_from_slice(&chunk[..read]);
        }
    }

    fn available(&self) -> usize {
        self.buffered.len() - self.consumed
    }

    fn take_packet(&mut self, packet_len: usize) -> Result<RuntimePacket, RuntimeError> {
        let packet_start = self.consumed;
        let packet_end = packet_start + packet_len;
        let packet = &self.buffered[packet_start..packet_end];
        let (header, metadata, body) = CommonHeader::parse_packet(packet)?;
        let packet = RuntimePacket::from_parts(header, metadata.to_vec(), body.to_vec())?;

        self.consumed = packet_end;
        self.packet_len = None;
        if self.consumed == self.buffered.len() {
            self.buffered.clear();
            self.consumed = 0;
        } else if self.consumed >= STREAM_READ_CHUNK_BYTES {
            self.buffered.drain(..self.consumed);
            self.consumed = 0;
        }
        Ok(packet)
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
    reader: StreamPacketReader,
}

impl TcpTransport {
    pub fn new(stream: TcpStream) -> Self {
        Self::new_with_limits(stream, RuntimeFrameLimits::default())
    }

    pub fn new_with_limits(stream: TcpStream, limits: RuntimeFrameLimits) -> Self {
        Self {
            stream,
            limits,
            reader: StreamPacketReader::new(),
        }
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
            reader: StreamPacketReader::new(),
        })
    }
}

#[async_trait]
impl FramedTransport for TcpTransport {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::Tcp
    }

    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
        self.reader.read_packet(&mut self.stream, self.limits).await
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
