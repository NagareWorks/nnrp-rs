use async_trait::async_trait;
use js_sys::{Function, Promise, Uint8Array};
use nnrp_core::{
    CommonHeader, FrameSubmitMetadata, MessageType, SessionPriorityClass, FRAME_SUBMIT_METADATA_LEN,
};
use nnrp_runtime::{
    FramedTransport, NnrpClient, NnrpClientConfig, NnrpClientSession, RuntimeError,
    RuntimeFrameLimits, RuntimePacket, RuntimeTransportKind,
};
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

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

struct HostWebSocketTransport {
    send: Function,
    receive: Function,
    close: Function,
    limits: RuntimeFrameLimits,
    closed: bool,
}

impl HostWebSocketTransport {
    fn new(send: Function, receive: Function, close: Function, limits: RuntimeFrameLimits) -> Self {
        Self {
            send,
            receive,
            close,
            limits,
            closed: false,
        }
    }

    fn ensure_open(&self) -> Result<(), RuntimeError> {
        if self.closed {
            Err(RuntimeError::TransportClosed {
                transport: RuntimeTransportKind::WebSocket,
                detail: "browser WebSocket carrier is already closed".to_owned(),
            })
        } else {
            Ok(())
        }
    }
}

#[async_trait(?Send)]
impl FramedTransport for HostWebSocketTransport {
    fn transport_kind(&self) -> RuntimeTransportKind {
        RuntimeTransportKind::WebSocket
    }

    async fn read_packet(&mut self) -> Result<RuntimePacket, RuntimeError> {
        self.ensure_open()?;
        let value = await_callback(self.receive.call0(&JsValue::NULL)).await?;
        let bytes = Uint8Array::new(&value).to_vec();
        self.limits.validate_packet_len(bytes.len())?;
        let (header, metadata, body) = CommonHeader::parse_packet(&bytes)?;
        RuntimePacket::from_parts(header, metadata.to_vec(), body.to_vec()).map_err(Into::into)
    }

    async fn write_packet(&mut self, packet: &RuntimePacket) -> Result<(), RuntimeError> {
        self.ensure_open()?;
        let bytes = packet.to_bytes()?;
        self.limits.validate_packet_len(bytes.len())?;
        let bytes = Uint8Array::from(bytes.as_slice());
        await_callback(self.send.call1(&JsValue::NULL, bytes.as_ref())).await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RuntimeError> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        await_callback(self.close.call0(&JsValue::NULL)).await?;
        Ok(())
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
    session: Option<NnrpClientSession>,
}

#[wasm_bindgen(js_class = BrowserClientRole)]
impl BrowserClientRole {
    #[wasm_bindgen(getter, js_name = sessionId)]
    pub fn session_id(&self) -> Result<u32, JsValue> {
        self.session
            .as_ref()
            .map(NnrpClientSession::session_id)
            .ok_or_else(|| js_error("browser client role is closed"))
    }

    #[wasm_bindgen(js_name = submitNoWait)]
    pub async fn submit_no_wait(&mut self, frame_id: u32, payload: &[u8]) -> Result<u32, JsValue> {
        if payload.len() < FRAME_SUBMIT_METADATA_LEN {
            return Err(js_error("FRAME_SUBMIT payload is truncated"));
        }
        let metadata = FrameSubmitMetadata::parse(&payload[..FRAME_SUBMIT_METADATA_LEN])
            .map_err(js_nnrp_error)?;
        self.session_mut()?
            .submit_with_frame_id(
                frame_id,
                metadata,
                payload[FRAME_SUBMIT_METADATA_LEN..].to_vec(),
            )
            .await
            .map_err(js_runtime_error)
    }

    #[wasm_bindgen(js_name = sendRuntimeFrame)]
    pub async fn send_runtime_frame(
        &mut self,
        message_type: u8,
        frame_id: u32,
        payload: &[u8],
    ) -> Result<(), JsValue> {
        let message_type = MessageType::try_from_u8(message_type).map_err(js_nnrp_error)?;
        self.session_mut()?
            .send_runtime_frame(message_type, frame_id, payload)
            .await
            .map_err(js_runtime_error)
    }

    #[wasm_bindgen(js_name = awaitEvent)]
    pub async fn await_event(&mut self) -> Result<BrowserClientEventPacket, JsValue> {
        let (_, packet) = self
            .session_mut()?
            .await_event_packet()
            .await
            .map_err(js_runtime_error)?;
        Ok(packet.into())
    }

    pub async fn close(&mut self) -> Result<(), JsValue> {
        if let Some(mut session) = self.session.take() {
            session.close_in_place().await.map_err(js_runtime_error)?;
        }
        Ok(())
    }

    fn session_mut(&mut self) -> Result<&mut NnrpClientSession, JsValue> {
        self.session
            .as_mut()
            .ok_or_else(|| js_error("browser client role is closed"))
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
    let transport = HostWebSocketTransport::new(send, receive, close, limits);
    let client = NnrpClient::from_transport(transport, config).map_err(js_runtime_error)?;
    let session = client.open_session().await.map_err(js_runtime_error)?;
    Ok(BrowserClientRole {
        session: Some(session),
    })
}

async fn await_callback(result: Result<JsValue, JsValue>) -> Result<JsValue, RuntimeError> {
    let value = result.map_err(js_transport_error)?;
    JsFuture::from(Promise::resolve(&value))
        .await
        .map_err(js_transport_error)
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
