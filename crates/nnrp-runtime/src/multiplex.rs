use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

use async_trait::async_trait;
use futures_channel::{mpsc, mpsc::TryRecvError, oneshot};
use futures_util::{
    future::{select, Either},
    lock::Mutex,
    pin_mut,
    stream::StreamExt,
};

use crate::{
    BoxedFramedTransport, FramedTransport, RuntimeError, RuntimePacket, RuntimeTransportKind,
};
use nnrp_core::MessageType;

const MAX_PRE_REGISTRATION_PACKETS: usize = 4_096;
const MAX_PRE_REGISTRATION_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default)]
struct PendingSessionPackets {
    packets: VecDeque<RuntimePacket>,
    bytes: usize,
}

impl PendingSessionPackets {
    fn push(&mut self, packet: RuntimePacket) -> Result<(), &'static str> {
        let packet_bytes = packet
            .metadata
            .len()
            .checked_add(packet.body.len())
            .and_then(|size| size.checked_add(nnrp_core::COMMON_HEADER_LEN))
            .ok_or("pre-registration packet byte count overflowed")?;
        let total_bytes = self
            .bytes
            .checked_add(packet_bytes)
            .ok_or("pre-registration packet byte count overflowed")?;
        if self.packets.len() >= MAX_PRE_REGISTRATION_PACKETS
            || total_bytes > MAX_PRE_REGISTRATION_BYTES
        {
            return Err("pre-registration packet buffer limit exceeded");
        }
        self.bytes = total_bytes;
        self.packets.push_back(packet);
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct DriverFailure {
    transport: RuntimeTransportKind,
    detail: String,
}

impl DriverFailure {
    fn from_error(transport: RuntimeTransportKind, error: RuntimeError) -> Self {
        Self {
            transport,
            detail: error.to_string(),
        }
    }

    fn into_runtime_error(self) -> RuntimeError {
        RuntimeError::TransportClosed {
            transport: self.transport,
            detail: self.detail,
        }
    }
}

enum DriverEvent {
    Packet(RuntimePacket),
    Failed(DriverFailure),
}

enum DriverCommand {
    Write {
        packet: RuntimePacket,
        completion: oneshot::Sender<Result<(), DriverFailure>>,
    },
    Register {
        session_id: u32,
        events: mpsc::UnboundedSender<DriverEvent>,
        completion: oneshot::Sender<Result<(), DriverFailure>>,
    },
    Unregister {
        session_id: u32,
    },
    Close {
        completion: oneshot::Sender<Result<(), DriverFailure>>,
    },
}

#[derive(Clone)]
pub(crate) struct MultiplexedConnection {
    transport_kind: RuntimeTransportKind,
    commands: mpsc::UnboundedSender<DriverCommand>,
    control_events: Arc<Mutex<mpsc::UnboundedReceiver<DriverEvent>>>,
    driver_start: Arc<Mutex<Option<DriverStart>>>,
}

struct DriverStart {
    transport: BoxedFramedTransport,
    commands: mpsc::UnboundedReceiver<DriverCommand>,
    control_events: mpsc::UnboundedSender<DriverEvent>,
}

pub(crate) struct MultiplexedSessionTransport {
    session_id: u32,
    transport_kind: RuntimeTransportKind,
    commands: mpsc::UnboundedSender<DriverCommand>,
    events: mpsc::UnboundedReceiver<DriverEvent>,
    closed: bool,
}

impl MultiplexedConnection {
    pub(crate) fn start(transport: BoxedFramedTransport) -> Self {
        let transport_kind = transport.transport_kind();
        let (commands, command_events) = mpsc::unbounded();
        let (control_events, control_receiver) = mpsc::unbounded();
        Self {
            transport_kind,
            commands,
            control_events: Arc::new(Mutex::new(control_receiver)),
            driver_start: Arc::new(Mutex::new(Some(DriverStart {
                transport,
                commands: command_events,
                control_events,
            }))),
        }
    }

    pub(crate) fn transport_kind(&self) -> RuntimeTransportKind {
        self.transport_kind
    }

    pub(crate) async fn write_packet(&self, packet: RuntimePacket) -> Result<(), RuntimeError> {
        self.ensure_started().await;
        let (completion, result) = oneshot::channel();
        self.commands
            .unbounded_send(DriverCommand::Write { packet, completion })
            .map_err(|_| self.closed_error("connection driver command channel closed"))?;
        result
            .await
            .map_err(|_| self.closed_error("connection driver dropped write completion"))?
            .map_err(DriverFailure::into_runtime_error)
    }

    pub(crate) async fn read_control_packet(&self) -> Result<RuntimePacket, RuntimeError> {
        self.ensure_started().await;
        let mut receiver = self.control_events.lock().await;
        match receiver.next().await {
            Some(DriverEvent::Packet(packet)) => Ok(packet),
            Some(DriverEvent::Failed(error)) => Err(error.into_runtime_error()),
            None => Err(self.closed_error("connection driver control channel closed")),
        }
    }

    pub(crate) async fn register_session(
        &self,
        session_id: u32,
    ) -> Result<MultiplexedSessionTransport, RuntimeError> {
        if session_id == 0 {
            return Err(RuntimeError::UnexpectedMessage(
                "multiplexed session id must be non-zero",
            ));
        }
        self.ensure_started().await;
        let (events, receiver) = mpsc::unbounded();
        let (completion, result) = oneshot::channel();
        self.commands
            .unbounded_send(DriverCommand::Register {
                session_id,
                events,
                completion,
            })
            .map_err(|_| self.closed_error("connection driver command channel closed"))?;
        result
            .await
            .map_err(|_| self.closed_error("connection driver dropped register completion"))?
            .map_err(DriverFailure::into_runtime_error)?;
        Ok(MultiplexedSessionTransport {
            session_id,
            transport_kind: self.transport_kind,
            commands: self.commands.clone(),
            events: receiver,
            closed: false,
        })
    }

    pub(crate) async fn close(&self) -> Result<(), RuntimeError> {
        self.ensure_started().await;
        let (completion, result) = oneshot::channel();
        self.commands
            .unbounded_send(DriverCommand::Close { completion })
            .map_err(|_| self.closed_error("connection driver command channel closed"))?;
        result
            .await
            .map_err(|_| self.closed_error("connection driver dropped close completion"))?
            .map_err(DriverFailure::into_runtime_error)
    }

    fn closed_error(&self, detail: impl Into<String>) -> RuntimeError {
        RuntimeError::TransportClosed {
            transport: self.transport_kind,
            detail: detail.into(),
        }
    }

    async fn ensure_started(&self) {
        let start = self.driver_start.lock().await.take();
        if let Some(start) = start {
            spawn_driver(run_driver(
                start.transport,
                self.transport_kind,
                start.commands,
                start.control_events,
            ));
        }
    }
}

impl MultiplexedSessionTransport {
    fn unregister(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let _ = self.commands.unbounded_send(DriverCommand::Unregister {
            session_id: self.session_id,
        });
    }

    fn closed_error(&self, detail: impl Into<String>) -> RuntimeError {
        RuntimeError::TransportClosed {
            transport: self.transport_kind,
            detail: detail.into(),
        }
    }
}

impl Drop for MultiplexedSessionTransport {
    fn drop(&mut self) {
        self.unregister();
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl FramedTransport for MultiplexedSessionTransport {
    fn transport_kind(&self) -> RuntimeTransportKind {
        self.transport_kind
    }

    fn try_read_packet(&mut self) -> Result<Option<RuntimePacket>, RuntimeError> {
        match self.events.try_recv() {
            Ok(DriverEvent::Packet(packet)) => Ok(Some(packet)),
            Ok(DriverEvent::Failed(error)) => Err(error.into_runtime_error()),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Closed) => Err(self.closed_error("session event channel closed")),
        }
    }

    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
        match self.events.next().await {
            Some(DriverEvent::Packet(packet)) => Ok(packet),
            Some(DriverEvent::Failed(error)) => Err(error.into_runtime_error()),
            None => Err(self.closed_error("session event channel closed")),
        }
    }

    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
        if self.closed {
            return Err(self.closed_error("session transport is closed"));
        }
        let (completion, result) = oneshot::channel();
        self.commands
            .unbounded_send(DriverCommand::Write {
                packet: packet.clone(),
                completion,
            })
            .map_err(|_| self.closed_error("connection driver command channel closed"))?;
        result
            .await
            .map_err(|_| self.closed_error("connection driver dropped write completion"))?
            .map_err(DriverFailure::into_runtime_error)
    }

    async fn close(&mut self) -> Result<(), RuntimeError> {
        self.unregister();
        Ok(())
    }
}

async fn run_driver(
    mut transport: BoxedFramedTransport,
    transport_kind: RuntimeTransportKind,
    mut commands: mpsc::UnboundedReceiver<DriverCommand>,
    control_events: mpsc::UnboundedSender<DriverEvent>,
) {
    enum NextDriverEvent {
        Command(Option<DriverCommand>),
        Incoming(Result<RuntimePacket, RuntimeError>),
    }

    let mut sessions = BTreeMap::<u32, mpsc::UnboundedSender<DriverEvent>>::new();
    let mut awaiting_registration = BTreeSet::<u32>::new();
    let mut pending_sessions = BTreeMap::<u32, PendingSessionPackets>::new();
    let mut terminal_failure = None::<DriverFailure>;
    loop {
        let next = if terminal_failure.is_some() {
            NextDriverEvent::Command(commands.next().await)
        } else {
            let next = {
                let command = commands.next();
                let incoming = transport.read_packet();
                pin_mut!(command, incoming);
                match select(command, incoming).await {
                    Either::Left((command, _pending_incoming)) => NextDriverEvent::Command(command),
                    Either::Right((incoming, _pending_command)) => {
                        NextDriverEvent::Incoming(incoming)
                    }
                }
            };
            next
        };

        match next {
            NextDriverEvent::Command(Some(command)) => match command {
                DriverCommand::Write { packet, completion } => {
                    if let Some(error) = terminal_failure.clone() {
                        let _ = completion.send(Err(error));
                        continue;
                    }
                    if packet.header.message_type == MessageType::SessionOpenAck
                        && packet.header.session_id != 0
                    {
                        awaiting_registration.insert(packet.header.session_id);
                    }
                    let result = transport
                        .write_packet(&packet)
                        .await
                        .map_err(|error| DriverFailure::from_error(transport_kind, error));
                    let failed = result.as_ref().err().cloned();
                    let _ = completion.send(result);
                    if let Some(error) = failed {
                        broadcast_failure(&control_events, &sessions, error);
                        break;
                    }
                }
                DriverCommand::Register {
                    session_id,
                    events,
                    completion,
                } => {
                    let result = match sessions.entry(session_id) {
                        std::collections::btree_map::Entry::Occupied(_) => Err(DriverFailure {
                            transport: transport_kind,
                            detail: format!("session {session_id} is already registered"),
                        }),
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            awaiting_registration.remove(&session_id);
                            if let Some(mut pending) = pending_sessions.remove(&session_id) {
                                while let Some(packet) = pending.packets.pop_front() {
                                    if events.unbounded_send(DriverEvent::Packet(packet)).is_err() {
                                        break;
                                    }
                                }
                            }
                            if let Some(error) = terminal_failure.clone() {
                                let _ = events.unbounded_send(DriverEvent::Failed(error));
                            }
                            entry.insert(events);
                            Ok(())
                        }
                    };
                    let _ = completion.send(result);
                }
                DriverCommand::Unregister { session_id } => {
                    sessions.remove(&session_id);
                    awaiting_registration.remove(&session_id);
                    pending_sessions.remove(&session_id);
                }
                DriverCommand::Close { completion } => {
                    if let Some(error) = terminal_failure.clone() {
                        let _ = completion.send(Err(error));
                        break;
                    }
                    let result = transport
                        .close()
                        .await
                        .map_err(|error| DriverFailure::from_error(transport_kind, error));
                    let _ = completion.send(result);
                    break;
                }
            },
            NextDriverEvent::Command(None) => {
                let _ = transport.close().await;
                break;
            }
            NextDriverEvent::Incoming(Ok(packet)) => {
                if packet.header.message_type == MessageType::SessionOpenAck
                    && packet.header.session_id != 0
                {
                    awaiting_registration.insert(packet.header.session_id);
                }
                if packet.header.session_id != 0 {
                    let session_id = packet.header.session_id;
                    if let Some(events) = sessions.get(&session_id).cloned() {
                        if events.unbounded_send(DriverEvent::Packet(packet)).is_ok() {
                            continue;
                        }
                        sessions.remove(&session_id);
                        awaiting_registration.remove(&session_id);
                        pending_sessions.remove(&session_id);
                        continue;
                    }
                    if awaiting_registration.contains(&session_id)
                        && packet.header.message_type != MessageType::SessionOpenAck
                    {
                        let pending = pending_sessions.entry(session_id).or_default();
                        if let Err(detail) = pending.push(packet) {
                            broadcast_failure(
                                &control_events,
                                &sessions,
                                DriverFailure {
                                    transport: transport_kind,
                                    detail: detail.to_owned(),
                                },
                            );
                            break;
                        }
                        continue;
                    }
                    if packet.header.message_type != MessageType::SessionOpenAck {
                        let failure = DriverFailure {
                            transport: transport_kind,
                            detail: format!(
                                "received packet for unknown session {}",
                                packet.header.session_id
                            ),
                        };
                        broadcast_failure(&control_events, &sessions, failure.clone());
                        terminal_failure = Some(failure);
                        continue;
                    }
                }
                if control_events
                    .unbounded_send(DriverEvent::Packet(packet))
                    .is_err()
                    && sessions.is_empty()
                {
                    let _ = transport.close().await;
                    break;
                }
            }
            NextDriverEvent::Incoming(Err(error)) => {
                let failure = DriverFailure::from_error(transport_kind, error);
                broadcast_failure(&control_events, &sessions, failure.clone());
                terminal_failure = Some(failure);
            }
        }
    }
}

fn broadcast_failure(
    control: &mpsc::UnboundedSender<DriverEvent>,
    sessions: &BTreeMap<u32, mpsc::UnboundedSender<DriverEvent>>,
    error: DriverFailure,
) {
    let _ = control.unbounded_send(DriverEvent::Failed(error.clone()));
    for events in sessions.values() {
        let _ = events.unbounded_send(DriverEvent::Failed(error.clone()));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_driver(future: impl std::future::Future<Output = ()> + Send + 'static) {
    tokio::spawn(future);
}

#[cfg(target_arch = "wasm32")]
fn spawn_driver(future: impl std::future::Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn spawn_runtime_task(future: impl std::future::Future<Output = ()> + Send + 'static) {
    tokio::spawn(future);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn spawn_runtime_task(future: impl std::future::Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use futures_channel::{mpsc, oneshot};
    use futures_util::StreamExt;
    use nnrp_core::{CommonHeader, MessageType};

    use super::{DriverCommand, MultiplexedConnection, PendingSessionPackets};
    use crate::{FramedTransport, RuntimeError, RuntimePacket, RuntimeTransportKind};

    struct ChannelTransport {
        incoming: mpsc::UnboundedReceiver<Result<RuntimePacket, RuntimeError>>,
        writes: mpsc::UnboundedSender<RuntimePacket>,
        closed: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl FramedTransport for ChannelTransport {
        fn transport_kind(&self) -> RuntimeTransportKind {
            RuntimeTransportKind::Tcp
        }

        async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
            self.incoming.next().await.unwrap_or_else(|| {
                Err(RuntimeError::TransportClosed {
                    transport: RuntimeTransportKind::Tcp,
                    detail: "test incoming channel closed".to_owned(),
                })
            })
        }

        async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
            self.writes
                .unbounded_send(packet.clone())
                .map_err(|_| RuntimeError::TransportClosed {
                    transport: RuntimeTransportKind::Tcp,
                    detail: "test write channel closed".to_owned(),
                })
        }

        async fn close(&mut self) -> Result<(), RuntimeError> {
            self.closed.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    fn packet(message_type: MessageType, session_id: u32) -> RuntimePacket {
        let mut header = CommonHeader::new(message_type, 0, 0);
        header.session_id = session_id;
        RuntimePacket::new(header, Vec::new(), Vec::new()).expect("test packet should encode")
    }

    struct ConnectionHarness {
        connection: MultiplexedConnection,
        incoming: mpsc::UnboundedSender<Result<RuntimePacket, RuntimeError>>,
        writes: mpsc::UnboundedReceiver<RuntimePacket>,
        closed: Arc<AtomicBool>,
    }

    fn create_connection() -> ConnectionHarness {
        let (incoming, incoming_events) = mpsc::unbounded();
        let (writes, write_events) = mpsc::unbounded();
        let closed = Arc::new(AtomicBool::new(false));
        let transport = ChannelTransport {
            incoming: incoming_events,
            writes,
            closed: Arc::clone(&closed),
        };
        ConnectionHarness {
            connection: MultiplexedConnection::start(Box::new(transport)),
            incoming,
            writes: write_events,
            closed,
        }
    }

    #[tokio::test]
    async fn buffers_pre_registration_packets_and_rejects_duplicate_sessions() {
        let ConnectionHarness {
            connection,
            incoming,
            mut writes,
            closed,
        } = create_connection();
        assert_eq!(connection.transport_kind(), RuntimeTransportKind::Tcp);
        assert!(matches!(
            connection.register_session(0).await,
            Err(RuntimeError::UnexpectedMessage(
                "multiplexed session id must be non-zero"
            ))
        ));

        connection
            .write_packet(packet(MessageType::SessionOpenAck, 7))
            .await
            .expect("open acknowledgement should be written");
        assert_eq!(
            writes
                .next()
                .await
                .expect("driver should forward the write")
                .header
                .session_id,
            7
        );

        incoming
            .unbounded_send(Ok(packet(MessageType::ResultPush, 7)))
            .expect("pre-registration packet should be accepted");
        tokio::task::yield_now().await;

        let mut session = connection
            .register_session(7)
            .await
            .expect("session should register");
        assert_eq!(
            session
                .read_packet()
                .await
                .expect("buffered packet should be delivered")
                .header
                .message_type,
            MessageType::ResultPush
        );
        assert!(session
            .try_read_packet()
            .expect("empty poll should work")
            .is_none());
        assert!(connection.register_session(7).await.is_err());

        session.close().await.expect("session close should succeed");
        assert!(session
            .write_packet(&packet(MessageType::FrameSubmit, 7))
            .await
            .is_err());
        connection.close().await.expect("connection should close");
        assert!(closed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn unknown_session_and_transport_failure_reach_control_reader() {
        let ConnectionHarness {
            connection,
            incoming,
            writes: _writes,
            closed: _closed,
        } = create_connection();
        incoming
            .unbounded_send(Ok(packet(MessageType::ResultPush, 99)))
            .expect("unknown-session packet should reach the driver");
        let error = connection
            .read_control_packet()
            .await
            .expect_err("unknown session should fail the connection");
        assert!(error.to_string().contains("unknown session 99"));
        assert!(connection.close().await.is_err());

        let ConnectionHarness {
            connection,
            incoming,
            writes: _writes,
            closed: _closed,
        } = create_connection();
        incoming
            .unbounded_send(Err(RuntimeError::TransportClosed {
                transport: RuntimeTransportKind::Tcp,
                detail: "injected read failure".to_owned(),
            }))
            .expect("transport failure should reach the driver");
        let error = connection
            .read_control_packet()
            .await
            .expect_err("transport failure should reach the control reader");
        assert!(error.to_string().contains("injected read failure"));
    }

    #[tokio::test]
    async fn late_packet_for_dropped_session_receiver_does_not_close_connection() {
        let ConnectionHarness {
            connection,
            incoming,
            writes: _writes,
            closed: _closed,
        } = create_connection();
        connection.ensure_started().await;
        let (events, receiver) = mpsc::unbounded();
        drop(receiver);
        let (completion, result) = oneshot::channel();
        connection
            .commands
            .unbounded_send(DriverCommand::Register {
                session_id: 7,
                events,
                completion,
            })
            .expect("closed receiver should still register before delivery");
        result
            .await
            .expect("driver should return registration result")
            .expect("registration should succeed");

        incoming
            .unbounded_send(Ok(packet(MessageType::ResultPush, 7)))
            .expect("late session packet should reach the driver");
        incoming
            .unbounded_send(Ok(packet(MessageType::ServerHelloAck, 0)))
            .expect("control packet should reach the driver");

        assert_eq!(
            connection
                .read_control_packet()
                .await
                .expect("connection should stay alive after a late session packet")
                .header
                .message_type,
            MessageType::ServerHelloAck
        );
    }

    #[tokio::test]
    async fn write_failure_terminates_the_connection_driver() {
        let ConnectionHarness {
            connection,
            incoming: _incoming,
            writes,
            closed: _closed,
        } = create_connection();
        drop(writes);
        let error = connection
            .write_packet(packet(MessageType::ClientHello, 0))
            .await
            .expect_err("closed write channel should fail the driver");
        assert!(error.to_string().contains("test write channel closed"));
        assert!(connection.read_control_packet().await.is_err());
    }

    #[test]
    fn pre_registration_buffer_enforces_packet_and_byte_limits() {
        let mut packet_limited = PendingSessionPackets::default();
        for _ in 0..super::MAX_PRE_REGISTRATION_PACKETS {
            packet_limited
                .push(packet(MessageType::ResultPush, 1))
                .expect("packet should fit below the count limit");
        }
        assert!(packet_limited
            .push(packet(MessageType::ResultPush, 1))
            .is_err());

        let mut byte_limited = PendingSessionPackets::default();
        let oversized = RuntimePacket::new(
            CommonHeader::new(MessageType::ResultPush, 0, 0),
            Vec::new(),
            vec![0; super::MAX_PRE_REGISTRATION_BYTES],
        )
        .expect("oversized packet should still be structurally valid");
        assert!(byte_limited.push(oversized).is_err());
    }
}
