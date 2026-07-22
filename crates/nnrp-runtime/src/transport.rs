use async_trait::async_trait;
#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
use nnrp_core::{CommonHeader, COMMON_HEADER_LEN};
#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
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

#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
const STREAM_READ_CHUNK_BYTES: usize = 64 * 1024;

#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
#[derive(Debug)]
pub struct StreamPacketReader {
    buffered: Vec<u8>,
    consumed: usize,
    packet_len: Option<usize>,
    scratch: Vec<u8>,
}

#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
impl Default for StreamPacketReader {
    fn default() -> Self {
        Self {
            buffered: Vec::new(),
            consumed: 0,
            packet_len: None,
            scratch: vec![0; STREAM_READ_CHUNK_BYTES],
        }
    }
}

#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
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

            let read = reader.read(&mut self.scratch).await?;
            if read == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "stream closed before a complete NNRP packet",
                )
                .into());
            }
            self.buffered.extend_from_slice(&self.scratch[..read]);
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

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg(target_arch = "wasm32")]
pub trait FramedTransport {
    fn transport_kind(&self) -> RuntimeTransportKind;
    fn try_read_packet(&mut self) -> Result<Option<RuntimePacket>, RuntimeError> {
        Ok(None)
    }
    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError>;
    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError>;
    async fn close(&mut self) -> Result<(), RuntimeError>;
}

#[async_trait]
#[cfg(not(target_arch = "wasm32"))]
pub trait FramedTransport: Send {
    fn transport_kind(&self) -> RuntimeTransportKind;
    fn try_read_packet(&mut self) -> Result<Option<RuntimePacket>, RuntimeError> {
        Ok(None)
    }
    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError>;
    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError>;
    async fn close(&mut self) -> Result<(), RuntimeError>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg(target_arch = "wasm32")]
pub trait FramedListener {
    fn transport_kind(&self) -> RuntimeTransportKind;
    fn local_addr(&self) -> Result<std::net::SocketAddr, RuntimeError>;
    async fn accept(&self) -> Result<BoxedFramedTransport, RuntimeError>;
}

#[async_trait]
#[cfg(not(target_arch = "wasm32"))]
pub trait FramedListener: Send + Sync {
    fn transport_kind(&self) -> RuntimeTransportKind;
    fn local_addr(&self) -> Result<std::net::SocketAddr, RuntimeError>;
    async fn accept(&self) -> Result<BoxedFramedTransport, RuntimeError>;
}

#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
#[derive(Debug)]
pub struct TcpTransport {
    stream: TcpStream,
    limits: RuntimeFrameLimits,
    reader: StreamPacketReader,
}

#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
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
#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
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

#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
#[derive(Debug)]
pub struct TcpFramedListener {
    listener: TcpListener,
    limits: RuntimeFrameLimits,
}

#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
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
#[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
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

    #[cfg(all(feature = "native-tcp", not(target_arch = "wasm32")))]
    #[test]
    fn stream_packet_reader_keeps_read_scratch_out_of_the_async_future() {
        let mut packet_reader = StreamPacketReader::new();
        assert_eq!(packet_reader.scratch.len(), STREAM_READ_CHUNK_BYTES);

        let mut input = tokio::io::empty();
        let future = packet_reader.read_packet(&mut input, RuntimeFrameLimits::default());

        assert!(
            std::mem::size_of_val(&future) < 1024,
            "read_packet future unexpectedly stores a large inline buffer"
        );
    }
}
