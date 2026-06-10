use nnrp_core::{
    FrameSubmitMetadata, InputProfile, PayloadKindBitmap, ResultClass, ResultPushMetadata,
    SubmitMode, TileIndexMode, STANDARD_PROFILE_TOKEN,
};
use nnrp_runtime::{NnrpClient, NnrpClientConfig, NnrpServer, NnrpServerConfig, RuntimeError};
use nnrp_transport_ipc::{IpcEndpoint, IpcProvider};
use nnrp_transport_quic::{
    quic_client_config, quic_server_config, QuicClientEndpointConfig, QuicProvider,
    QuicServerEndpointConfig,
};
use nnrp_transport_websocket::{WebSocketEndpoint, WebSocketProvider};
use serde_json::{json, Map, Value};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const REQUEST_BODY: &[u8] = b"wire-reference-request";
const RESPONSE_BODY: &[u8] = b"wire-reference-result";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceTransport {
    Tcp,
    Ipc,
    Quic,
    WebSocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireTraceExpectation {
    pub trace_id: u64,
    pub span_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireReportExpectation<'a> {
    pub terminal_state: &'a str,
    pub required_frames: &'a [&'a str],
    pub result_drop_reason_code: Option<u64>,
    pub trace_context: Option<WireTraceExpectation>,
}

impl<'a> WireReportExpectation<'a> {
    pub fn success(required_frames: &'a [&'a str]) -> Self {
        Self {
            terminal_state: "success",
            required_frames,
            result_drop_reason_code: None,
            trace_context: None,
        }
    }

    pub fn with_result_drop_reason_code(mut self, drop_reason_code: u64) -> Self {
        self.result_drop_reason_code = Some(drop_reason_code);
        self
    }

    pub fn with_trace_context(mut self, trace_context: WireTraceExpectation) -> Self {
        self.trace_context = Some(trace_context);
        self
    }
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

pub async fn run_suite_as_client_reference(
    transport: ReferenceTransport,
) -> Result<Value, RuntimeError> {
    match transport {
        ReferenceTransport::Tcp => run_tcp_suite_as_client_reference().await,
        ReferenceTransport::Ipc => run_ipc_suite_as_client_reference().await,
        ReferenceTransport::Quic => run_quic_suite_as_client_reference().await,
        ReferenceTransport::WebSocket => run_websocket_suite_as_client_reference().await,
    }
}

pub async fn run_suite_as_server_reference(
    transport: ReferenceTransport,
) -> Result<Value, RuntimeError> {
    match transport {
        ReferenceTransport::Tcp => run_tcp_suite_as_server_reference().await,
        ReferenceTransport::Ipc => run_ipc_suite_as_server_reference().await,
        ReferenceTransport::Quic => run_quic_suite_as_server_reference().await,
        ReferenceTransport::WebSocket => run_websocket_suite_as_server_reference().await,
    }
}

pub fn validate_wire_reference_report(
    report: &Value,
    expectation: &WireReportExpectation<'_>,
) -> Result<(), String> {
    let terminal_state = report
        .get("terminal_state")
        .and_then(Value::as_str)
        .ok_or_else(|| "wire reference report must contain terminal_state".to_string())?;
    if terminal_state != expectation.terminal_state {
        return Err(format!(
            "wire reference terminal_state '{terminal_state}' did not match expected '{}'",
            expectation.terminal_state
        ));
    }

    let frames = report
        .get("frames")
        .and_then(Value::as_array)
        .ok_or_else(|| "wire reference report must contain frames".to_string())?;
    let mut last_timestamp = None;
    for (index, frame) in frames.iter().enumerate() {
        frame
            .get("direction")
            .and_then(Value::as_str)
            .filter(|direction| !direction.is_empty())
            .ok_or_else(|| format!("wire frame {index} must contain direction"))?;
        let timestamp_us = frame
            .get("timestamp_us")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("wire frame {index} must contain timestamp_us"))?;
        if let Some(last_timestamp) = last_timestamp {
            if timestamp_us < last_timestamp {
                return Err(format!(
                    "wire frame {index} timestamp_us regressed from {last_timestamp} to {timestamp_us}"
                ));
            }
        }
        last_timestamp = Some(timestamp_us);
    }

    for required in expectation.required_frames {
        if !frames.iter().any(|frame| {
            frame
                .get("message_type")
                .and_then(Value::as_str)
                .is_some_and(|message_type| message_type == *required)
        }) {
            return Err(format!(
                "wire reference report did not contain required frame '{required}'"
            ));
        }
    }

    if let Some(expected_drop_reason) = expectation.result_drop_reason_code {
        let drop_reason = frames
            .iter()
            .find(|frame| {
                frame
                    .get("message_type")
                    .and_then(Value::as_str)
                    .is_some_and(|message_type| message_type == "RESULT_DROP_REASON")
            })
            .and_then(|frame| frame.get("drop_reason_code"))
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                "wire reference report did not contain expected RESULT_DROP_REASON frame"
                    .to_string()
            })?;
        if drop_reason != expected_drop_reason {
            return Err(format!(
                "wire reference RESULT_DROP_REASON code {drop_reason} did not match expected {expected_drop_reason}"
            ));
        }
    }

    if let Some(expected_trace) = expectation.trace_context {
        let trace = frames
            .iter()
            .find(|frame| {
                frame
                    .get("message_type")
                    .and_then(Value::as_str)
                    .is_some_and(|message_type| message_type == "TRACE_CONTEXT")
            })
            .ok_or_else(|| {
                "wire reference report did not contain expected TRACE_CONTEXT frame".to_string()
            })?;
        let trace_id = trace
            .get("trace_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| "TRACE_CONTEXT frame must contain trace_id".to_string())?;
        let span_id = trace
            .get("span_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| "TRACE_CONTEXT frame must contain span_id".to_string())?;
        if trace_id != expected_trace.trace_id || span_id != expected_trace.span_id {
            return Err(format!(
                "TRACE_CONTEXT ({trace_id}, {span_id}) did not match expected ({}, {})",
                expected_trace.trace_id, expected_trace.span_id
            ));
        }
    }

    Ok(())
}

async fn run_tcp_suite_as_client_reference() -> Result<Value, RuntimeError> {
    let started = Instant::now();
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;
    let server_task = tokio::spawn(reference_server_task(server));

    let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
    let report = run_reference_client(ReferenceTransport::Tcp, started, client).await?;
    server_task
        .await
        .map_err(|_| RuntimeError::Internal("wire reference TCP server task panicked"))??;
    Ok(report)
}

async fn run_tcp_suite_as_server_reference() -> Result<Value, RuntimeError> {
    let started = Instant::now();
    let server = NnrpServer::bind_tcp("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let addr = server.local_addr()?;
    let target_task = tokio::spawn(async move {
        let client = NnrpClient::connect_tcp(addr, NnrpClientConfig::default()).await?;
        target_client_task(client).await
    });

    let report = run_reference_server(ReferenceTransport::Tcp, started, server).await?;
    target_task
        .await
        .map_err(|_| RuntimeError::Internal("wire reference TCP target task panicked"))??;
    Ok(report)
}

async fn run_quic_suite_as_client_reference() -> Result<Value, RuntimeError> {
    let started = Instant::now();
    let (server_endpoint, certificate) = QuicServerEndpointConfig::self_signed_localhost(
        "127.0.0.1:0"
            .parse()
            .expect("loopback QUIC bind address should be a valid socket address"),
    )?;
    let server = QuicProvider::bind(
        server_endpoint,
        quic_server_config(NnrpServerConfig::default()),
    )
    .await?;
    let addr = server.local_addr()?;
    let server_task = tokio::spawn(reference_server_task(server));

    let client_endpoint =
        QuicClientEndpointConfig::localhost_with_root_certificate(certificate.certificate_der);
    let client = QuicProvider::connect_addr(
        addr,
        client_endpoint,
        quic_client_config(NnrpClientConfig::default()),
    )
    .await?;
    let report = run_reference_client(ReferenceTransport::Quic, started, client).await?;
    server_task
        .await
        .map_err(|_| RuntimeError::Internal("wire reference QUIC server task panicked"))??;
    Ok(report)
}

async fn run_quic_suite_as_server_reference() -> Result<Value, RuntimeError> {
    let started = Instant::now();
    let (server_endpoint, certificate) = QuicServerEndpointConfig::self_signed_localhost(
        "127.0.0.1:0"
            .parse()
            .expect("loopback QUIC bind address should be a valid socket address"),
    )?;
    let server = QuicProvider::bind(
        server_endpoint,
        quic_server_config(NnrpServerConfig::default()),
    )
    .await?;
    let addr = server.local_addr()?;
    let target_task = tokio::spawn(async move {
        let client_endpoint =
            QuicClientEndpointConfig::localhost_with_root_certificate(certificate.certificate_der);
        let client = QuicProvider::connect_addr(
            addr,
            client_endpoint,
            quic_client_config(NnrpClientConfig::default()),
        )
        .await?;
        target_client_task(client).await
    });

    let report = run_reference_server(ReferenceTransport::Quic, started, server).await?;
    target_task
        .await
        .map_err(|_| RuntimeError::Internal("wire reference QUIC target task panicked"))??;
    Ok(report)
}

async fn run_ipc_suite_as_client_reference() -> Result<Value, RuntimeError> {
    let started = Instant::now();
    let endpoint = unique_ipc_endpoint();
    let server = IpcProvider::bind(&endpoint, NnrpServerConfig::default()).await?;
    let server_task = tokio::spawn(reference_server_task(server));

    let client = connect_ipc_client_with_retry(&endpoint).await?;
    let report = run_reference_client(ReferenceTransport::Ipc, started, client).await?;
    server_task
        .await
        .map_err(|_| RuntimeError::Internal("wire reference IPC server task panicked"))??;
    cleanup_ipc_endpoint(&endpoint);
    Ok(report)
}

async fn run_ipc_suite_as_server_reference() -> Result<Value, RuntimeError> {
    let started = Instant::now();
    let endpoint = unique_ipc_endpoint();
    let server = IpcProvider::bind(&endpoint, NnrpServerConfig::default()).await?;
    let target_endpoint = endpoint.clone();
    let target_task = tokio::spawn(async move {
        let client = connect_ipc_client_with_retry(&target_endpoint).await?;
        target_client_task(client).await
    });

    let report = run_reference_server(ReferenceTransport::Ipc, started, server).await?;
    target_task
        .await
        .map_err(|_| RuntimeError::Internal("wire reference IPC target task panicked"))??;
    cleanup_ipc_endpoint(&endpoint);
    Ok(report)
}

async fn run_websocket_suite_as_client_reference() -> Result<Value, RuntimeError> {
    let started = Instant::now();
    let server = WebSocketProvider::bind("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let endpoint = WebSocketEndpoint::ws(format!("ws://{}", server.local_addr()?))?;
    let server_task = tokio::spawn(reference_server_task(server));

    let client = WebSocketProvider::connect(&endpoint, NnrpClientConfig::default()).await?;
    let report = run_reference_client(ReferenceTransport::WebSocket, started, client).await?;
    server_task
        .await
        .map_err(|_| RuntimeError::Internal("wire reference WebSocket server task panicked"))??;
    Ok(report)
}

async fn run_websocket_suite_as_server_reference() -> Result<Value, RuntimeError> {
    let started = Instant::now();
    let server = WebSocketProvider::bind("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let endpoint = WebSocketEndpoint::ws(format!("ws://{}", server.local_addr()?))?;
    let target_task = tokio::spawn(async move {
        let client = WebSocketProvider::connect(&endpoint, NnrpClientConfig::default()).await?;
        target_client_task(client).await
    });

    let report = run_reference_server(ReferenceTransport::WebSocket, started, server).await?;
    target_task
        .await
        .map_err(|_| RuntimeError::Internal("wire reference WebSocket target task panicked"))??;
    Ok(report)
}

async fn connect_ipc_client_with_retry(endpoint: &IpcEndpoint) -> Result<NnrpClient, RuntimeError> {
    let mut last_error = None;
    for _ in 0..25 {
        match IpcProvider::connect(endpoint, NnrpClientConfig::default()).await {
            Ok(client) => return Ok(client),
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }
    Err(last_error.unwrap_or(RuntimeError::UnsupportedTransport(
        "wire reference IPC endpoint did not accept connections",
    )))
}

async fn reference_server_task(server: NnrpServer) -> Result<(), RuntimeError> {
    let mut session = server.accept().await?;
    let submit = session.receive_submit().await?;
    if submit.body != REQUEST_BODY {
        return Err(RuntimeError::UnexpectedMessage(
            "wire reference server received unexpected request body",
        ));
    }
    session
        .send_result(submit.frame_id, token_result(), RESPONSE_BODY.to_vec())
        .await?;
    let close = session.receive_close().await?;
    session.ack_close(&close).await?;
    session.close().await
}

async fn target_client_task(client: NnrpClient) -> Result<(), RuntimeError> {
    let mut session = client.open_session().await?;
    let frame_id = session
        .submit(token_submit(), REQUEST_BODY.to_vec())
        .await?;
    let result = session.await_result().await?;
    if result.frame_id != frame_id || result.body != RESPONSE_BODY {
        return Err(RuntimeError::UnexpectedMessage(
            "wire reference target received unexpected response",
        ));
    }
    session.close().await
}

async fn run_reference_server(
    transport: ReferenceTransport,
    started: Instant,
    server: NnrpServer,
) -> Result<Value, RuntimeError> {
    let mut frames = ObservedFrameLog::new(started);
    let mut session = server.accept().await?;
    let session_id = session.session_id();
    frames.push(
        "target->suite",
        "SESSION_OPEN",
        json!({
            "session_id": session_id,
        }),
    );
    let submit = session.receive_submit().await?;
    frames.push(
        "target->suite",
        "FRAME_SUBMIT",
        json!({
            "session_id": session_id,
            "frame_id": submit.frame_id,
            "body_bytes": submit.body.len(),
        }),
    );
    if submit.body != REQUEST_BODY {
        return Err(RuntimeError::UnexpectedMessage(
            "wire reference suite received unexpected request body",
        ));
    }
    session
        .send_result(submit.frame_id, token_result(), RESPONSE_BODY.to_vec())
        .await?;
    frames.push(
        "suite->target",
        "RESULT_PUSH",
        json!({
            "session_id": session_id,
            "frame_id": submit.frame_id,
            "body_bytes": RESPONSE_BODY.len(),
            "status_code": 200,
        }),
    );
    let close = session.receive_close().await?;
    frames.push(
        "target->suite",
        "SESSION_CLOSE",
        json!({
            "session_id": session_id,
        }),
    );
    session.ack_close(&close).await?;
    session.close().await?;

    Ok(json!({
        "mode": "suite-as-server",
        "transport": transport.as_str(),
        "target": "nnrp-rs-reference",
        "status": "passed",
        "terminal_state": "success",
        "timing": {
            "elapsed_us": started.elapsed().as_micros(),
        },
        "frames": frames.into_frames(),
    }))
}

fn unique_ipc_endpoint() -> IpcEndpoint {
    #[cfg(unix)]
    {
        IpcEndpoint::unix(std::env::temp_dir().join(format!(
            "nnrp-wire-reference-{}-{}.sock",
            std::process::id(),
            unique_suffix()
        )))
    }
    #[cfg(windows)]
    {
        IpcEndpoint::named_pipe(format!(
            "nnrp-wire-reference-{}-{}",
            std::process::id(),
            unique_suffix()
        ))
    }
}

fn cleanup_ipc_endpoint(endpoint: &IpcEndpoint) {
    #[cfg(unix)]
    if let Some(path) = endpoint.as_unix_path() {
        let _ = std::fs::remove_file(path);
    }
    #[cfg(not(unix))]
    let _ = endpoint;
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after UNIX epoch")
        .as_nanos()
}

async fn run_reference_client(
    transport: ReferenceTransport,
    started: Instant,
    client: NnrpClient,
) -> Result<Value, RuntimeError> {
    let mut frames = ObservedFrameLog::new(started);
    let mut session = client.open_session().await?;
    let session_id = session.session_id();
    frames.push(
        "suite->target",
        "SESSION_OPEN",
        json!({
            "session_id": session_id,
        }),
    );
    let frame_id = session
        .submit(token_submit(), REQUEST_BODY.to_vec())
        .await?;
    frames.push(
        "suite->target",
        "FRAME_SUBMIT",
        json!({
            "session_id": session_id,
            "frame_id": frame_id,
            "body_bytes": REQUEST_BODY.len(),
        }),
    );
    let result = session.await_result().await?;
    frames.push(
        "target->suite",
        "RESULT_PUSH",
        json!({
            "session_id": session_id,
            "frame_id": result.frame_id,
            "body_bytes": result.body.len(),
            "status_code": result.metadata.status_code,
        }),
    );
    if result.body != RESPONSE_BODY {
        return Err(RuntimeError::UnexpectedMessage(
            "wire reference client received unexpected response body",
        ));
    }
    session.close().await?;
    frames.push(
        "suite->target",
        "SESSION_CLOSE",
        json!({
            "session_id": session_id,
        }),
    );

    Ok(json!({
        "mode": "suite-as-client",
        "transport": transport.as_str(),
        "target": "nnrp-rs-reference",
        "status": "passed",
        "terminal_state": "success",
        "timing": {
            "elapsed_us": started.elapsed().as_micros(),
        },
        "frames": frames.into_frames(),
    }))
}

struct ObservedFrameLog {
    started: Instant,
    frames: Vec<Value>,
}

impl ObservedFrameLog {
    fn new(started: Instant) -> Self {
        Self {
            started,
            frames: Vec::new(),
        }
    }

    fn push(&mut self, direction: &str, message_type: &str, extra: Value) {
        let mut frame = match extra {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        frame.insert(
            "direction".to_string(),
            Value::String(direction.to_string()),
        );
        frame.insert(
            "message_type".to_string(),
            Value::String(message_type.to_string()),
        );
        frame.insert(
            "timestamp_us".to_string(),
            Value::from(self.started.elapsed().as_micros() as u64),
        );
        self.frames.push(Value::Object(frame));
    }

    fn into_frames(self) -> Vec<Value> {
        self.frames
    }
}

fn token_submit() -> FrameSubmitMetadata {
    FrameSubmitMetadata {
        src_width: 0,
        src_height: 0,
        tile_width: 0,
        tile_height: 0,
        tile_count: 0,
        section_count: 0,
        frame_class: 0,
        input_profile: InputProfile::Unspecified,
        tile_index_mode: TileIndexMode::DenseRange,
        latency_budget_ms: 25,
        target_fps_x100: 0,
        retry_of_frame: 0,
        tile_base_id: 0,
        camera_bytes: 0,
        tile_index_bytes: 0,
        submit_mode: SubmitMode::Inline,
        budget_policy: 0,
        loss_tolerance_policy: 0,
        object_ref_mask: 0,
        dependency_frame_id: 0,
        payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
        payload_frame_count: 1,
    }
}

fn token_result() -> ResultPushMetadata {
    ResultPushMetadata {
        status_code: 200,
        result_flags: 0,
        section_count: 0,
        tile_count: 0,
        active_profile_id: STANDARD_PROFILE_TOKEN,
        inference_ms: 1,
        queue_ms: 0,
        server_total_ms: 1,
        tile_base_id: 0,
        tile_index_bytes: 0,
        result_class: ResultClass::Complete,
        applied_budget_policy: 0,
        reused_frame_id: 0,
        covered_tile_count: 0,
        dropped_tile_count: 0,
        payload_kind_bitmap: PayloadKindBitmap(PayloadKindBitmap::TOKEN_CHUNK),
        payload_frame_count: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        run_suite_as_client_reference, run_suite_as_server_reference,
        validate_wire_reference_report, ReferenceTransport, WireReportExpectation,
        WireTraceExpectation,
    };
    use serde_json::json;

    const REQUIRED_LOOPBACK_FRAMES: &[&str] = &[
        "SESSION_OPEN",
        "FRAME_SUBMIT",
        "RESULT_PUSH",
        "SESSION_CLOSE",
    ];

    #[tokio::test]
    async fn suite_as_client_reference_runs_tcp_endpoint() {
        let report = run_suite_as_client_reference(ReferenceTransport::Tcp)
            .await
            .expect("TCP reference endpoint should run");
        assert_suite_as_client_report(&report, "tcp");
    }

    #[tokio::test]
    async fn suite_as_client_reference_runs_ipc_endpoint() {
        let report = run_suite_as_client_reference(ReferenceTransport::Ipc)
            .await
            .expect("IPC reference endpoint should run");
        assert_suite_as_client_report(&report, "ipc");
    }

    #[tokio::test]
    async fn ipc_reference_reports_unavailable_endpoint() {
        let endpoint = super::unique_ipc_endpoint();
        let error = super::connect_ipc_client_with_retry(&endpoint)
            .await
            .expect_err("unbound IPC endpoint should fail");
        assert!(!error.to_string().is_empty());
        super::cleanup_ipc_endpoint(&endpoint);
    }

    #[tokio::test]
    async fn suite_as_client_reference_runs_quic_endpoint() {
        let report = run_suite_as_client_reference(ReferenceTransport::Quic)
            .await
            .expect("QUIC reference endpoint should run");
        assert_suite_as_client_report(&report, "quic");
    }

    #[tokio::test]
    async fn suite_as_client_reference_runs_websocket_endpoint() {
        let report = run_suite_as_client_reference(ReferenceTransport::WebSocket)
            .await
            .expect("WebSocket reference endpoint should run");
        assert_suite_as_client_report(&report, "websocket");
    }

    #[tokio::test]
    async fn suite_as_server_reference_runs_tcp_listener() {
        let report = run_suite_as_server_reference(ReferenceTransport::Tcp)
            .await
            .expect("TCP reference listener should run");
        assert_suite_as_server_report(&report, "tcp");
    }

    #[tokio::test]
    async fn suite_as_server_reference_runs_ipc_listener() {
        let report = run_suite_as_server_reference(ReferenceTransport::Ipc)
            .await
            .expect("IPC reference listener should run");
        assert_suite_as_server_report(&report, "ipc");
    }

    #[tokio::test]
    async fn suite_as_server_reference_runs_quic_listener() {
        let report = run_suite_as_server_reference(ReferenceTransport::Quic)
            .await
            .expect("QUIC reference listener should run");
        assert_suite_as_server_report(&report, "quic");
    }

    #[tokio::test]
    async fn suite_as_server_reference_runs_websocket_listener() {
        let report = run_suite_as_server_reference(ReferenceTransport::WebSocket)
            .await
            .expect("WebSocket reference listener should run");
        assert_suite_as_server_report(&report, "websocket");
    }

    fn assert_suite_as_client_report(report: &serde_json::Value, transport: &str) {
        validate_wire_reference_report(
            report,
            &WireReportExpectation::success(REQUIRED_LOOPBACK_FRAMES),
        )
        .expect("suite-as-client report should validate");
        assert_eq!(report["mode"], "suite-as-client");
        assert_eq!(report["transport"], transport);
        assert_eq!(report["status"], "passed");
        assert_eq!(report["terminal_state"], "success");
        assert!(report["timing"]["elapsed_us"].as_u64().is_some());

        let frames = report["frames"]
            .as_array()
            .expect("reference report should contain frames");
        let message_types = frames
            .iter()
            .map(|frame| frame["message_type"].as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        assert_eq!(
            message_types,
            vec![
                "SESSION_OPEN",
                "FRAME_SUBMIT",
                "RESULT_PUSH",
                "SESSION_CLOSE"
            ]
        );
        assert_eq!(frames[1]["body_bytes"], super::REQUEST_BODY.len());
        assert_eq!(frames[2]["body_bytes"], super::RESPONSE_BODY.len());
        assert_eq!(frames[2]["status_code"], 200);
    }

    fn assert_suite_as_server_report(report: &serde_json::Value, transport: &str) {
        validate_wire_reference_report(
            report,
            &WireReportExpectation::success(REQUIRED_LOOPBACK_FRAMES),
        )
        .expect("suite-as-server report should validate");
        assert_eq!(report["mode"], "suite-as-server");
        assert_eq!(report["transport"], transport);
        assert_eq!(report["status"], "passed");
        assert_eq!(report["terminal_state"], "success");
        assert!(report["timing"]["elapsed_us"].as_u64().is_some());

        let frames = report["frames"]
            .as_array()
            .expect("reference report should contain frames");
        let message_types = frames
            .iter()
            .map(|frame| frame["message_type"].as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        assert_eq!(
            message_types,
            vec![
                "SESSION_OPEN",
                "FRAME_SUBMIT",
                "RESULT_PUSH",
                "SESSION_CLOSE"
            ]
        );
        assert_eq!(frames[1]["direction"], "target->suite");
        assert_eq!(frames[1]["body_bytes"], super::REQUEST_BODY.len());
        assert_eq!(frames[2]["direction"], "suite->target");
        assert_eq!(frames[2]["body_bytes"], super::RESPONSE_BODY.len());
        assert_eq!(frames[2]["status_code"], 200);
    }

    #[test]
    fn validator_rejects_wrong_terminal_state() {
        let report = json!({
            "terminal_state": "failed",
            "frames": []
        });
        let error = validate_wire_reference_report(
            &report,
            &WireReportExpectation::success(&["SESSION_OPEN"]),
        )
        .expect_err("terminal state should fail");
        assert!(error.contains("terminal_state 'failed'"));
    }

    #[test]
    fn validator_rejects_missing_required_frame() {
        let report = json!({
            "terminal_state": "success",
            "frames": [{
                "direction": "suite->target",
                "message_type": "SESSION_OPEN",
                "timestamp_us": 1
            }]
        });
        let error = validate_wire_reference_report(
            &report,
            &WireReportExpectation::success(&["SESSION_OPEN", "RESULT_PUSH"]),
        )
        .expect_err("missing frame should fail");
        assert!(error.contains("required frame 'RESULT_PUSH'"));
    }

    #[test]
    fn validator_rejects_unordered_frame_timestamps() {
        let report = json!({
            "terminal_state": "success",
            "frames": [
                {
                    "direction": "suite->target",
                    "message_type": "SESSION_OPEN",
                    "timestamp_us": 5
                },
                {
                    "direction": "suite->target",
                    "message_type": "SESSION_CLOSE",
                    "timestamp_us": 4
                }
            ]
        });
        let error = validate_wire_reference_report(&report, &WireReportExpectation::success(&[]))
            .expect_err("timestamp regression should fail");
        assert!(error.contains("timestamp_us regressed"));
    }

    #[test]
    fn validator_checks_expected_drop_reason_and_trace_context() {
        let report = json!({
            "terminal_state": "success",
            "frames": [
                {
                    "direction": "target->suite",
                    "message_type": "TRACE_CONTEXT",
                    "timestamp_us": 1,
                    "trace_id": 42,
                    "span_id": 7
                },
                {
                    "direction": "suite->target",
                    "message_type": "RESULT_DROP_REASON",
                    "timestamp_us": 2,
                    "drop_reason_code": 3
                }
            ]
        });
        let expectation = WireReportExpectation::success(&["TRACE_CONTEXT", "RESULT_DROP_REASON"])
            .with_trace_context(WireTraceExpectation {
                trace_id: 42,
                span_id: 7,
            })
            .with_result_drop_reason_code(3);
        validate_wire_reference_report(&report, &expectation)
            .expect("drop reason and trace context should validate");

        let expectation = WireReportExpectation::success(&["TRACE_CONTEXT"]).with_trace_context(
            WireTraceExpectation {
                trace_id: 42,
                span_id: 8,
            },
        );
        let error = validate_wire_reference_report(&report, &expectation)
            .expect_err("wrong trace context should fail");
        assert!(error.contains("TRACE_CONTEXT"));
    }
}
