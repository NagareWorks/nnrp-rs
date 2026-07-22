use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, VecDeque},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

use async_trait::async_trait;
use futures_util::{
    future::{AbortHandle, Abortable},
    lock::Mutex,
};
use js_sys::{Array, Function, Promise, Uint32Array, Uint8Array};
use nnrp_core::{
    CommonHeader, FrameSubmitMetadata, MessageType, SessionPatchMetadata, SessionPriorityClass,
    FRAME_SUBMIT_METADATA_LEN, SESSION_PATCH_METADATA_LEN,
};
use nnrp_runtime::{
    FramedTransport, NnrpClient, NnrpClientConfig, NnrpClientSession, RuntimeError,
    RuntimeFrameLimits, RuntimePacket, RuntimeTransportKind,
};
use serde::Deserialize;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{future_to_promise, JsFuture};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BrowserClientRoleConfig {
    requested_session_id: u32,
    profile_id: u16,
    schema_id: u32,
    schema_version: u32,
    priority_class: u8,
    default_deadline_ms: u32,
    max_in_flight_operations: u16,
    lease_ttl_hint_ms: u32,
    max_packet_bytes: usize,
}

impl BrowserClientRoleConfig {
    fn into_runtime(self) -> Result<(NnrpClientConfig, RuntimeFrameLimits), JsValue> {
        if self.max_packet_bytes == 0 {
            return Err(js_error("maxPacketBytes must be greater than zero"));
        }
        let priority_class =
            SessionPriorityClass::try_from_u8(self.priority_class).map_err(js_nnrp_error)?;
        Ok((
            NnrpClientConfig {
                transport: RuntimeTransportKind::WebSocket,
                requested_session_id: self.requested_session_id,
                profile_id: self.profile_id,
                schema_id: self.schema_id,
                schema_version: self.schema_version,
                priority_class,
                default_deadline_ms: self.default_deadline_ms,
                max_in_flight_operations: self.max_in_flight_operations,
                lease_ttl_hint_ms: self.lease_ttl_hint_ms,
                allow_resume: false,
                resume_token_bytes: 0,
                cache_hints: Vec::new(),
            },
            RuntimeFrameLimits::new(self.max_packet_bytes),
        ))
    }
}

struct HostWebSocketCarrier {
    send: Function,
    receive: Function,
    close: Function,
    limits: RuntimeFrameLimits,
    pending_packets: RefCell<VecDeque<Vec<u8>>>,
    receive_error: RefCell<Option<String>>,
    packet_waiters: WaiterRegistry,
    event_waiters: WaiterRegistry,
    event_generation: Cell<u64>,
    external_ingress: Cell<bool>,
    closed: Cell<bool>,
}

impl HostWebSocketCarrier {
    fn new(send: Function, receive: Function, close: Function, limits: RuntimeFrameLimits) -> Self {
        Self {
            send,
            receive,
            close,
            limits,
            pending_packets: RefCell::new(VecDeque::new()),
            receive_error: RefCell::new(None),
            packet_waiters: WaiterRegistry::new(),
            event_waiters: WaiterRegistry::new(),
            event_generation: Cell::new(0),
            external_ingress: Cell::new(false),
            closed: Cell::new(false),
        }
    }

    fn enable_external_ingress(&self) {
        self.external_ingress.set(true);
    }

    fn ingest_receive_value(&self, value: JsValue) -> Result<(), RuntimeError> {
        self.ensure_open()?;
        self.enqueue_receive_value(value)?;
        self.packet_waiters.wake_all();
        self.notify_event_waiters();
        Ok(())
    }

    fn fail_receive(&self, detail: String) {
        self.receive_error.borrow_mut().replace(detail);
        self.packet_waiters.wake_all();
        self.notify_event_waiters();
    }

    fn ensure_open(&self) -> Result<(), RuntimeError> {
        if let Some(detail) = self.receive_error.borrow().as_ref() {
            return Err(RuntimeError::TransportClosed {
                transport: RuntimeTransportKind::WebSocket,
                detail: detail.clone(),
            });
        }
        if self.closed.get() {
            return Err(RuntimeError::TransportClosed {
                transport: RuntimeTransportKind::WebSocket,
                detail: "browser WebSocket carrier is already closed".to_owned(),
            });
        }
        Ok(())
    }

    fn enqueue_receive_value(&self, value: JsValue) -> Result<(), RuntimeError> {
        let mut pending_packets = self.pending_packets.borrow_mut();
        if Array::is_array(&value) {
            let packets = Array::from(&value);
            if packets.length() == 0 {
                return Err(RuntimeError::UnexpectedMessage(
                    "browser WebSocket receive callback returned an empty packet batch",
                ));
            }
            for packet in packets.iter() {
                pending_packets.push_back(receive_packet_bytes(packet)?);
            }
        } else {
            pending_packets.push_back(receive_packet_bytes(value)?);
        }
        Ok(())
    }

    fn decode_packet(&self, bytes: Vec<u8>) -> Result<RuntimePacket, RuntimeError> {
        self.limits.validate_packet_len(bytes.len())?;
        let (header, metadata, body) = CommonHeader::parse_packet(&bytes)?;
        RuntimePacket::from_parts(header, metadata.to_vec(), body.to_vec()).map_err(Into::into)
    }

    fn try_read_packet(&self) -> Result<Option<RuntimePacket>, RuntimeError> {
        self.pending_packets
            .borrow_mut()
            .pop_front()
            .map(|bytes| self.decode_packet(bytes))
            .transpose()
    }

    fn poll_ingress(&self) -> Option<Result<(), RuntimeError>> {
        if !self.pending_packets.borrow().is_empty() {
            return Some(Ok(()));
        }
        if let Some(detail) = self.receive_error.borrow().as_ref() {
            return Some(Err(RuntimeError::TransportClosed {
                transport: RuntimeTransportKind::WebSocket,
                detail: detail.clone(),
            }));
        }
        self.closed.get().then(|| self.ensure_open())
    }

    fn event_generation(&self) -> u64 {
        self.event_generation.get()
    }

    fn poll_event_notification(
        &self,
        observed_generation: u64,
    ) -> Option<Result<(), RuntimeError>> {
        if let Some(result) = self.poll_ingress() {
            return Some(result);
        }
        (self.event_generation.get() != observed_generation).then_some(Ok(()))
    }

    fn notify_event_waiters(&self) {
        self.event_generation
            .set(self.event_generation.get().wrapping_add(1));
        self.event_waiters.wake_all();
    }

    async fn wait_for_packet(&self) -> Result<(), RuntimeError> {
        PacketWaiter::new(self).await
    }

    async fn wait_for_event(&self, observed_generation: u64) -> Result<(), RuntimeError> {
        EventWaiter::new(self, observed_generation).await
    }

    async fn receive_for_handshake(&self) -> Result<(), RuntimeError> {
        let value = await_callback(self.receive.call0(&JsValue::NULL)).await?;
        self.ingest_receive_value(value)
    }

    async fn write_packet(&self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
        self.ensure_open()?;
        let bytes = packet.to_bytes()?;
        self.limits.validate_packet_len(bytes.len())?;
        let bytes = Uint8Array::from(bytes.as_slice());
        await_callback(self.send.call1(&JsValue::NULL, bytes.as_ref())).await?;
        Ok(())
    }

    async fn close(&self) -> Result<(), RuntimeError> {
        if self.closed.replace(true) {
            return Ok(());
        }
        self.packet_waiters.wake_all();
        self.notify_event_waiters();
        await_callback(self.close.call0(&JsValue::NULL)).await?;
        Ok(())
    }
}

struct WaiterRegistry {
    waiters: RefCell<BTreeMap<u64, Waker>>,
    next_waiter_id: Cell<u64>,
}

impl WaiterRegistry {
    fn new() -> Self {
        Self {
            waiters: RefCell::new(BTreeMap::new()),
            next_waiter_id: Cell::new(0),
        }
    }

    fn register(&self, waiter_id: &mut Option<u64>, waker: &Waker) {
        let mut waiters = self.waiters.borrow_mut();
        if let Some(waiter_id) = waiter_id {
            waiters.insert(*waiter_id, waker.clone());
            return;
        }
        let mut candidate = self.next_waiter_id.get();
        while waiters.contains_key(&candidate) {
            candidate = candidate.wrapping_add(1);
        }
        self.next_waiter_id.set(candidate.wrapping_add(1));
        waiters.insert(candidate, waker.clone());
        waiter_id.replace(candidate);
    }

    fn remove(&self, waiter_id: Option<u64>) {
        if let Some(waiter_id) = waiter_id {
            self.waiters.borrow_mut().remove(&waiter_id);
        }
    }

    fn wake_all(&self) {
        let waiters = std::mem::take(&mut *self.waiters.borrow_mut());
        for (_, waker) in waiters {
            waker.wake();
        }
    }
}

struct PacketWaiter<'a> {
    carrier: &'a HostWebSocketCarrier,
    waiter_id: Option<u64>,
}

impl<'a> PacketWaiter<'a> {
    fn new(carrier: &'a HostWebSocketCarrier) -> Self {
        Self {
            carrier,
            waiter_id: None,
        }
    }
}

impl Future for PacketWaiter<'_> {
    type Output = Result<(), RuntimeError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self.carrier.poll_ingress() {
            self.carrier.packet_waiters.remove(self.waiter_id.take());
            return Poll::Ready(result);
        }
        let this = self.as_mut().get_mut();
        this.carrier
            .packet_waiters
            .register(&mut this.waiter_id, context.waker());
        if let Some(result) = this.carrier.poll_ingress() {
            this.carrier.packet_waiters.remove(this.waiter_id.take());
            Poll::Ready(result)
        } else {
            Poll::Pending
        }
    }
}

impl Drop for PacketWaiter<'_> {
    fn drop(&mut self) {
        self.carrier.packet_waiters.remove(self.waiter_id.take());
    }
}

struct EventWaiter<'a> {
    carrier: &'a HostWebSocketCarrier,
    observed_generation: u64,
    waiter_id: Option<u64>,
}

impl<'a> EventWaiter<'a> {
    fn new(carrier: &'a HostWebSocketCarrier, observed_generation: u64) -> Self {
        Self {
            carrier,
            observed_generation,
            waiter_id: None,
        }
    }
}

impl Future for EventWaiter<'_> {
    type Output = Result<(), RuntimeError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self
            .carrier
            .poll_event_notification(self.observed_generation)
        {
            self.carrier.event_waiters.remove(self.waiter_id.take());
            return Poll::Ready(result);
        }
        let this = self.as_mut().get_mut();
        this.carrier
            .event_waiters
            .register(&mut this.waiter_id, context.waker());
        if let Some(result) = this
            .carrier
            .poll_event_notification(this.observed_generation)
        {
            this.carrier.event_waiters.remove(this.waiter_id.take());
            Poll::Ready(result)
        } else {
            Poll::Pending
        }
    }
}

impl Drop for EventWaiter<'_> {
    fn drop(&mut self) {
        self.carrier.event_waiters.remove(self.waiter_id.take());
    }
}

struct HostWebSocketTransport {
    carrier: Rc<HostWebSocketCarrier>,
}

impl HostWebSocketTransport {
    fn new(carrier: Rc<HostWebSocketCarrier>) -> Self {
        Self { carrier }
    }
}

#[async_trait(?Send)]
impl FramedTransport for HostWebSocketTransport {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::WebSocket
    }

    fn try_read_packet(&mut self) -> Result<Option<RuntimePacket>, RuntimeError> {
        self.carrier.try_read_packet()
    }

    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
        if let Some(packet) = self.try_read_packet()? {
            return Ok(packet);
        }
        if self.carrier.external_ingress.get() {
            self.carrier.wait_for_packet().await?;
        } else {
            self.carrier.receive_for_handshake().await?;
        }
        self.try_read_packet()?.ok_or(RuntimeError::Internal(
            "browser WebSocket receive queue was empty after a successful callback",
        ))
    }

    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
        self.carrier.write_packet(packet).await
    }

    async fn close(&mut self) -> Result<(), RuntimeError> {
        self.carrier.close().await
    }
}

#[wasm_bindgen(js_name = BrowserClientEventPacket)]
pub struct BrowserClientEventPacket {
    message_type: u8,
    session_id: u32,
    frame_id: u32,
    metadata: Vec<u8>,
    body: Vec<u8>,
}

impl From<RuntimePacket> for BrowserClientEventPacket {
    fn from(packet: RuntimePacket) -> Self {
        Self {
            message_type: packet.header.message_type as u8,
            session_id: packet.header.session_id,
            frame_id: packet.header.frame_id,
            metadata: packet.metadata,
            body: packet.body,
        }
    }
}

#[wasm_bindgen(js_name = BrowserClientEventBatch)]
pub struct BrowserClientEventBatch {
    packet_bytes: Vec<u8>,
    packet_offsets: Vec<u32>,
}

impl BrowserClientEventBatch {
    fn from_events(
        events: Vec<(nnrp_runtime::NnrpClientEvent, RuntimePacket)>,
    ) -> Result<Self, RuntimeError> {
        let mut packet_bytes = Vec::new();
        let mut packet_offsets = Vec::with_capacity(events.len() + 1);
        packet_offsets.push(0);
        for (_, packet) in events {
            packet_bytes.extend_from_slice(&packet.to_bytes()?);
            packet_offsets.push(
                u32::try_from(packet_bytes.len())
                    .map_err(|_| RuntimeError::Internal("browser event batch exceeds u32"))?,
            );
        }
        Ok(Self {
            packet_bytes,
            packet_offsets,
        })
    }
}

#[wasm_bindgen(js_class = BrowserClientEventBatch)]
impl BrowserClientEventBatch {
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> u32 {
        self.packet_offsets.len().saturating_sub(1) as u32
    }

    #[wasm_bindgen(getter, js_name = packetBytes)]
    pub fn packet_bytes(&self) -> Uint8Array {
        Uint8Array::from(self.packet_bytes.as_slice())
    }

    #[wasm_bindgen(getter, js_name = packetOffsets)]
    pub fn packet_offsets(&self) -> Uint32Array {
        Uint32Array::from(self.packet_offsets.as_slice())
    }
}

#[wasm_bindgen(js_class = BrowserClientEventPacket)]
impl BrowserClientEventPacket {
    #[wasm_bindgen(getter, js_name = messageType)]
    pub fn message_type(&self) -> u8 {
        self.message_type
    }

    #[wasm_bindgen(getter, js_name = sessionId)]
    pub fn session_id(&self) -> u32 {
        self.session_id
    }

    #[wasm_bindgen(getter, js_name = frameId)]
    pub fn frame_id(&self) -> u32 {
        self.frame_id
    }

    #[wasm_bindgen(getter)]
    pub fn metadata(&self) -> Uint8Array {
        Uint8Array::from(self.metadata.as_slice())
    }

    #[wasm_bindgen(getter)]
    pub fn body(&self) -> Uint8Array {
        Uint8Array::from(self.body.as_slice())
    }
}

#[wasm_bindgen(js_name = BrowserClientRole)]
pub struct BrowserClientRole {
    state: Rc<BrowserClientRoleState>,
}

struct BrowserClientRoleState {
    session_id: u32,
    session: Mutex<Option<NnrpClientSession>>,
    carrier: Rc<HostWebSocketCarrier>,
    receive_gate: Mutex<()>,
    receive_abort: RefCell<Option<AbortHandle>>,
}

#[wasm_bindgen(js_class = BrowserClientRole)]
impl BrowserClientRole {
    #[wasm_bindgen(getter, js_name = sessionId)]
    pub fn session_id(&self) -> Result<u32, JsValue> {
        self.state
            .carrier
            .ensure_open()
            .map_err(js_runtime_error)
            .map(|()| self.state.session_id)
    }

    #[wasm_bindgen(js_name = submitNoWait)]
    pub fn submit_no_wait_promise(&self, frame_id: u32, payload: &[u8]) -> Promise {
        let state = Rc::clone(&self.state);
        let payload = payload.to_vec();
        future_to_promise(async move {
            state
                .submit_no_wait(frame_id, &payload)
                .await
                .map(|frame_id| JsValue::from_f64(frame_id.into()))
        })
    }

    #[wasm_bindgen(js_name = sendRuntimeFrame)]
    pub fn send_runtime_frame_promise(
        &self,
        message_type: u8,
        frame_id: u32,
        payload: &[u8],
    ) -> Promise {
        let state = Rc::clone(&self.state);
        let payload = payload.to_vec();
        future_to_promise(async move {
            state
                .send_runtime_frame(message_type, frame_id, &payload)
                .await
                .map(|()| JsValue::UNDEFINED)
        })
    }

    #[wasm_bindgen(js_name = patchSession)]
    pub fn patch_session_promise(&self, metadata: &[u8]) -> Promise {
        let state = Rc::clone(&self.state);
        let metadata = metadata.to_vec();
        future_to_promise(async move {
            state
                .patch_session(&metadata)
                .await
                .map(|bytes| JsValue::from(bytes))
        })
    }

    #[wasm_bindgen(js_name = awaitEvent)]
    pub fn await_event_promise(&self) -> Promise {
        let state = Rc::clone(&self.state);
        future_to_promise(async move { state.await_event().await.map(JsValue::from) })
    }

    #[wasm_bindgen(js_name = awaitEventBatch)]
    pub fn await_event_batch_promise(&self, max_events: u32) -> Promise {
        if max_events == 0 {
            return Promise::reject(&js_error("maxEvents must be greater than zero"));
        }
        let state = Rc::clone(&self.state);
        future_to_promise(
            async move { state.await_event_batch(max_events).await.map(JsValue::from) },
        )
    }

    #[wasm_bindgen(js_name = ingestPackets)]
    pub fn ingest_packets(&self, packets: JsValue) -> Result<(), JsValue> {
        self.state
            .carrier
            .ingest_receive_value(packets)
            .map_err(js_runtime_error)
    }

    #[wasm_bindgen(js_name = failReceive)]
    pub fn fail_receive(&self, detail: String) {
        self.state.carrier.fail_receive(detail);
    }

    #[wasm_bindgen(js_name = close)]
    pub fn close_promise(&self) -> Promise {
        let state = Rc::clone(&self.state);
        future_to_promise(async move { state.close().await.map(|()| JsValue::UNDEFINED) })
    }
}

impl BrowserClientRole {
    pub async fn submit_no_wait(&self, frame_id: u32, payload: &[u8]) -> Result<u32, JsValue> {
        self.state.submit_no_wait(frame_id, payload).await
    }

    pub async fn send_runtime_frame(
        &self,
        message_type: u8,
        frame_id: u32,
        payload: &[u8],
    ) -> Result<(), JsValue> {
        self.state
            .send_runtime_frame(message_type, frame_id, payload)
            .await
    }

    pub async fn patch_session(&self, metadata: &[u8]) -> Result<Uint8Array, JsValue> {
        self.state.patch_session(metadata).await
    }

    pub async fn await_event(&self) -> Result<BrowserClientEventPacket, JsValue> {
        self.state.await_event().await
    }

    pub async fn await_event_batch(
        &self,
        max_events: u32,
    ) -> Result<BrowserClientEventBatch, JsValue> {
        self.state.await_event_batch(max_events).await
    }

    pub async fn close(&self) -> Result<(), JsValue> {
        self.state.close().await
    }
}

impl BrowserClientRoleState {
    async fn submit_no_wait(&self, frame_id: u32, payload: &[u8]) -> Result<u32, JsValue> {
        if payload.len() < FRAME_SUBMIT_METADATA_LEN {
            return Err(js_error("FRAME_SUBMIT payload is truncated"));
        }
        let metadata = FrameSubmitMetadata::parse(&payload[..FRAME_SUBMIT_METADATA_LEN])
            .map_err(js_nnrp_error)?;
        self.session
            .lock()
            .await
            .as_mut()
            .ok_or_else(closed_role_error)?
            .submit_with_frame_id(
                frame_id,
                metadata,
                payload[FRAME_SUBMIT_METADATA_LEN..].to_vec(),
            )
            .await
            .map_err(js_runtime_error)
    }

    async fn send_runtime_frame(
        &self,
        message_type: u8,
        frame_id: u32,
        payload: &[u8],
    ) -> Result<(), JsValue> {
        let message_type = MessageType::try_from_u8(message_type).map_err(js_nnrp_error)?;
        self.session
            .lock()
            .await
            .as_mut()
            .ok_or_else(closed_role_error)?
            .send_runtime_frame(message_type, frame_id, payload)
            .await
            .map_err(js_runtime_error)
    }

    async fn patch_session(&self, metadata: &[u8]) -> Result<Uint8Array, JsValue> {
        if metadata.len() != SESSION_PATCH_METADATA_LEN {
            return Err(js_error(&format!(
                "SESSION_PATCH metadata must be exactly {SESSION_PATCH_METADATA_LEN} bytes"
            )));
        }
        let patch = SessionPatchMetadata::parse(metadata).map_err(js_nnrp_error)?;
        if patch.profile_patch_bytes != 0 {
            return Err(js_error(
                "browser session patch does not accept a profile-specific body",
            ));
        }
        let ack = self
            .session
            .lock()
            .await
            .as_mut()
            .ok_or_else(closed_role_error)?
            .patch_session(patch)
            .await
            .map_err(js_runtime_error)?;
        self.carrier.notify_event_waiters();
        let bytes = ack.to_bytes().map_err(js_nnrp_error)?;
        Ok(Uint8Array::from(bytes.as_slice()))
    }

    async fn await_event(&self) -> Result<BrowserClientEventPacket, JsValue> {
        let mut events = self.receive_event_packets(1).await?;
        let (_, packet) = events
            .pop()
            .ok_or_else(|| js_error("browser event receive produced no packet"))?;
        Ok(packet.into())
    }

    async fn await_event_batch(&self, max_events: u32) -> Result<BrowserClientEventBatch, JsValue> {
        if max_events == 0 {
            return Err(js_error("maxEvents must be greater than zero"));
        }
        let events = self.receive_event_packets(max_events).await?;
        BrowserClientEventBatch::from_events(events).map_err(js_runtime_error)
    }

    async fn receive_event_packets(
        &self,
        max_events: u32,
    ) -> Result<Vec<(nnrp_runtime::NnrpClientEvent, RuntimePacket)>, JsValue> {
        let max_events = usize::try_from(max_events)
            .map_err(|_| js_error("maxEvents is not representable on this host"))?;
        let _receive_guard = self.receive_gate.lock().await;
        let (abort, registration) = AbortHandle::new_pair();
        self.receive_abort.borrow_mut().replace(abort);
        let result =
            Abortable::new(self.receive_event_packets_locked(max_events), registration).await;
        self.receive_abort.borrow_mut().take();
        result.map_err(|_| closed_role_error())?
    }

    async fn receive_event_packets_locked(
        &self,
        max_events: usize,
    ) -> Result<Vec<(nnrp_runtime::NnrpClientEvent, RuntimePacket)>, JsValue> {
        loop {
            let observed_generation = self.carrier.event_generation();
            let buffered = self.poll_session_event_packets(max_events).await?;
            if !buffered.is_empty() {
                return Ok(buffered);
            }
            self.carrier
                .wait_for_event(observed_generation)
                .await
                .map_err(js_runtime_error)?;
        }
    }

    async fn poll_session_event_packets(
        &self,
        max_events: usize,
    ) -> Result<Vec<(nnrp_runtime::NnrpClientEvent, RuntimePacket)>, JsValue> {
        let mut session_slot = self.session.lock().await;
        session_slot
            .as_mut()
            .ok_or_else(closed_role_error)?
            .poll_event_packet_batch(max_events)
            .map_err(js_runtime_error)
    }

    async fn close(&self) -> Result<(), JsValue> {
        if let Some(abort) = self.receive_abort.borrow_mut().take() {
            abort.abort();
        }
        let _receive_guard = self.receive_gate.lock().await;
        let mut session_slot = self.session.lock().await;
        if let Some(mut session) = session_slot.take() {
            session.close_in_place().await.map_err(js_runtime_error)?;
        }
        Ok(())
    }
}

#[wasm_bindgen(js_name = openBrowserClientRole)]
pub async fn open_browser_client_role(
    send: Function,
    receive: Function,
    close: Function,
    config_json: &str,
) -> Result<BrowserClientRole, JsValue> {
    let config: BrowserClientRoleConfig =
        serde_json::from_str(config_json).map_err(js_serde_error)?;
    let (config, limits) = config.into_runtime()?;
    let carrier = Rc::new(HostWebSocketCarrier::new(send, receive, close, limits));
    let transport = HostWebSocketTransport::new(Rc::clone(&carrier));
    let client = NnrpClient::from_transport(transport, config).map_err(js_runtime_error)?;
    let session = client.open_session().await.map_err(js_runtime_error)?;
    carrier.enable_external_ingress();
    let session_id = session.session_id();
    Ok(BrowserClientRole {
        state: Rc::new(BrowserClientRoleState {
            session_id,
            session: Mutex::new(Some(session)),
            carrier,
            receive_gate: Mutex::new(()),
            receive_abort: RefCell::new(None),
        }),
    })
}

fn closed_role_error() -> JsValue {
    js_error("browser client role is closed")
}

async fn await_callback(result: Result<JsValue, JsValue>) -> Result<JsValue, RuntimeError> {
    let value = result.map_err(js_transport_error)?;
    JsFuture::from(Promise::resolve(&value))
        .await
        .map_err(js_transport_error)
}

fn receive_packet_bytes(value: JsValue) -> Result<Vec<u8>, RuntimeError> {
    value
        .dyn_into::<Uint8Array>()
        .map(|packet| packet.to_vec())
        .map_err(|_| {
            RuntimeError::UnexpectedMessage(
                "browser WebSocket receive callback must return Uint8Array packets",
            )
        })
}

fn js_transport_error(value: JsValue) -> RuntimeError {
    RuntimeError::TransportClosed {
        transport: RuntimeTransportKind::WebSocket,
        detail: value
            .as_string()
            .unwrap_or_else(|| "browser WebSocket callback rejected".to_owned()),
    }
}

fn js_runtime_error(error: RuntimeError) -> JsValue {
    js_error(&error.to_string())
}

fn js_nnrp_error(error: nnrp_core::NnrpError) -> JsValue {
    js_error(&error.to_string())
}

fn js_serde_error(error: serde_json::Error) -> JsValue {
    js_error(&format!("invalid browser client role config: {error}"))
}

fn js_error(message: &str) -> JsValue {
    js_sys::Error::new(message).into()
}
