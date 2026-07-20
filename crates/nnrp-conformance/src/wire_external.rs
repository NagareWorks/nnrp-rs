use std::time::Instant;

use nnrp_core::{
    CacheMissMetadata, CacheMissReason, CacheReferenceMetadata, CacheReuseScope,
    CapabilityMetadata, FrameSubmitMetadata, InputProfile, MessageType, PartialResultMetadata,
    PayloadKindBitmap, PressureMetadata, ProgressMetadata, ResultClass, ResultDropReasonMetadata,
    ResultPushMetadata, RouteHintMetadata, SubmitMode, TileIndexMode, TraceContextMetadata,
    RESULT_DROP_REASON_DEADLINE_EXPIRED, STANDARD_PROFILE_TOKEN,
};
use nnrp_runtime::{FramedListener, NnrpClientEvent, NnrpServer, RuntimeError};
use nnrp_transport_quic::{
    quic_client_config, quic_server_config, QuicClientEndpointConfig, QuicFramedListener,
    QuicProvider, QuicServerEndpointConfig,
};
use serde_json::{json, Value};

use crate::wire_endpoint::{ReferenceTransport, WireReferenceEndpoint};

const REQUEST_BODY: &[u8] = b"wire-external-request";
const RESPONSE_BODY: &[u8] = b"wire-external-result";
const CAPABILITY_BODY: &[u8] = b"cap!";
const ROUTE_BODY: &[u8] = b"hint";
const CACHE_BODY: &[u8] = b"ref!";
const TRACE_BODY: &[u8] = b"trace";
const PROGRESS_BODY: &[u8] = b"stage";
const PARTIAL_BODY: &[u8] = b"partial";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireExternalCase {
    CancelAbortClient,
    PriorityDeadlineProxy,
    ProgressBackpressureServer,
    CapabilityRouteCacheClient,
    CancelAbortIpcClient,
    ProgressBackpressureWebSocketServer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireExternalMode {
    SuiteAsClient,
    SuiteAsServer,
    SuiteAsProxy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireExternalTerminal {
    Success,
    Cancelled,
    Dropped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireExternalDirection {
    SuiteToTarget,
    TargetToSuite,
    ProbeToSuiteProxy,
    SuiteProxyToTarget,
    TargetThroughSuiteProxyToProbe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireExternalFrame {
    Request,
    Cancel,
    PriorityUpdate,
    ExpireAt,
    Progress,
    CreditUpdate,
    PartialResult,
    CapabilityNegotiation,
    RouteHint,
    CacheReference,
    CacheMiss,
    TraceContext,
    ResultPush,
    ResultDropReason,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WireExternalObservedFrame {
    pub direction: WireExternalDirection,
    pub frame: WireExternalFrame,
    pub timestamp_us: u128,
    pub detail: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WireExternalCaseReport {
    pub scenario_id: &'static str,
    pub mode: WireExternalMode,
    pub transport: ReferenceTransport,
    pub terminal: WireExternalTerminal,
    pub elapsed_us: u128,
    pub observed_frames: Vec<WireExternalObservedFrame>,
    pub result_drop_reason: Option<ResultDropReasonMetadata>,
    pub trace_context: Option<TraceContextMetadata>,
    pub cache_miss: Option<CacheMissMetadata>,
}

impl WireExternalCase {
    pub fn scenario_id(self) -> &'static str {
        match self {
            Self::CancelAbortClient => "wire.control.cancel-abort.client",
            Self::PriorityDeadlineProxy => "wire.control.priority-deadline.proxy",
            Self::ProgressBackpressureServer => "wire.control.progress-backpressure.server",
            Self::CapabilityRouteCacheClient => "wire.control.capability-route-cache.client",
            Self::CancelAbortIpcClient => "wire.control.cancel-abort.ipc-client",
            Self::ProgressBackpressureWebSocketServer => {
                "wire.control.progress-backpressure.websocket-server"
            }
        }
    }

    pub fn mode(self) -> WireExternalMode {
        match self {
            Self::CancelAbortClient
            | Self::CapabilityRouteCacheClient
            | Self::CancelAbortIpcClient => WireExternalMode::SuiteAsClient,
            Self::ProgressBackpressureServer | Self::ProgressBackpressureWebSocketServer => {
                WireExternalMode::SuiteAsServer
            }
            Self::PriorityDeadlineProxy => WireExternalMode::SuiteAsProxy,
        }
    }

    pub fn transport(self) -> ReferenceTransport {
        match self {
            Self::CancelAbortClient | Self::ProgressBackpressureServer => ReferenceTransport::Tcp,
            Self::PriorityDeadlineProxy | Self::CapabilityRouteCacheClient => {
                ReferenceTransport::Quic
            }
            Self::CancelAbortIpcClient => ReferenceTransport::Ipc,
            Self::ProgressBackpressureWebSocketServer => ReferenceTransport::WebSocket,
        }
    }
}

#[derive(Debug)]
struct ObservedFrames {
    started: Instant,
    frames: Vec<WireExternalObservedFrame>,
}

impl ObservedFrames {
    fn new(started: Instant) -> Self {
        Self {
            started,
            frames: Vec::new(),
        }
    }

    fn push(&mut self, direction: WireExternalDirection, frame: WireExternalFrame, detail: Value) {
        self.frames.push(WireExternalObservedFrame {
            direction,
            frame,
            timestamp_us: self.started.elapsed().as_micros(),
            detail,
        });
    }
}

pub async fn run_wire_external_case(
    case: WireExternalCase,
    endpoint: &WireReferenceEndpoint,
) -> Result<WireExternalCaseReport, RuntimeError> {
    endpoint.validate()?;
    if endpoint.transport != case.transport() {
        return Err(RuntimeError::UnsupportedTransport(
            "wire case transport does not match the target endpoint",
        ));
    }

    match case {
        WireExternalCase::CancelAbortClient | WireExternalCase::CancelAbortIpcClient => {
            run_cancel_abort_client(case, endpoint).await
        }
        WireExternalCase::CapabilityRouteCacheClient => {
            run_capability_route_cache_client(case, endpoint).await
        }
        WireExternalCase::ProgressBackpressureServer
        | WireExternalCase::ProgressBackpressureWebSocketServer => {
            run_progress_backpressure_server(case, endpoint).await
        }
        WireExternalCase::PriorityDeadlineProxy => {
            run_priority_deadline_proxy(case, endpoint).await
        }
    }
}

async fn run_cancel_abort_client(
    case: WireExternalCase,
    endpoint: &WireReferenceEndpoint,
) -> Result<WireExternalCaseReport, RuntimeError> {
    let started = Instant::now();
    let mut observed = ObservedFrames::new(started);
    let mut session = endpoint.connect().await?.open_session().await?;
    let session_id = session.session_id();
    let operation_id = 101;
    let frame_id = session
        .submit_nowait(token_submit(operation_id), REQUEST_BODY.to_vec())
        .await?;
    observed.push(
        WireExternalDirection::SuiteToTarget,
        WireExternalFrame::Request,
        json!({ "session_id": session_id, "frame_id": frame_id, "operation_id": operation_id }),
    );
    session
        .cancel_operation(operation_id, RESULT_DROP_REASON_DEADLINE_EXPIRED)
        .await?;
    observed.push(
        WireExternalDirection::SuiteToTarget,
        WireExternalFrame::Cancel,
        json!({ "session_id": session_id, "operation_id": operation_id }),
    );

    let mut trace = None;
    let mut drop_reason = None;
    while trace.is_none() || drop_reason.is_none() {
        match session.await_event().await? {
            NnrpClientEvent::TraceContext {
                frame_id,
                metadata,
                body,
            } => {
                observed.push(
                    WireExternalDirection::TargetToSuite,
                    WireExternalFrame::TraceContext,
                    json!({
                        "session_id": session_id,
                        "frame_id": frame_id,
                        "trace_id": metadata.trace_id,
                        "span_id": metadata.span_id,
                        "body_bytes": body.len(),
                    }),
                );
                trace = Some(metadata);
            }
            NnrpClientEvent::ResultDropReason { metadata, body } => {
                observed.push(
                    WireExternalDirection::TargetToSuite,
                    WireExternalFrame::ResultDropReason,
                    json!({
                        "session_id": session_id,
                        "operation_id": metadata.operation_id,
                        "drop_reason_code": metadata.drop_reason_code,
                        "diagnostic_bytes": body.len(),
                    }),
                );
                drop_reason = Some(metadata);
            }
            _ => {
                return Err(RuntimeError::UnexpectedMessage(
                    "cancel scenario expected TRACE_CONTEXT or RESULT_DROP_REASON",
                ));
            }
        }
    }
    let trace = trace.expect("trace was observed");
    let drop_reason = drop_reason.expect("drop reason was observed");
    if drop_reason.operation_id != operation_id {
        return Err(RuntimeError::UnexpectedMessage(
            "cancel scenario received a drop reason for another operation",
        ));
    }
    session.close().await?;
    Ok(report(
        case,
        started,
        WireExternalTerminal::Cancelled,
        observed.frames,
        Some(drop_reason),
        Some(trace),
        None,
    ))
}

async fn run_capability_route_cache_client(
    case: WireExternalCase,
    endpoint: &WireReferenceEndpoint,
) -> Result<WireExternalCaseReport, RuntimeError> {
    let started = Instant::now();
    let mut observed = ObservedFrames::new(started);
    let mut session = endpoint.connect().await?.open_session().await?;
    let session_id = session.session_id();
    let operation_id = 401;
    let frame_id = session
        .submit_nowait(token_submit(operation_id), REQUEST_BODY.to_vec())
        .await?;
    observed.push(
        WireExternalDirection::SuiteToTarget,
        WireExternalFrame::Request,
        json!({ "session_id": session_id, "frame_id": frame_id, "operation_id": operation_id }),
    );
    session
        .send_capability(
            MessageType::CapabilityNegotiation,
            capability_metadata(),
            CAPABILITY_BODY.to_vec(),
        )
        .await?;
    observed.push(
        WireExternalDirection::SuiteToTarget,
        WireExternalFrame::CapabilityNegotiation,
        json!({ "session_id": session_id }),
    );
    session
        .send_route_hint(
            MessageType::RouteHint,
            route_hint(operation_id),
            ROUTE_BODY.to_vec(),
        )
        .await?;
    observed.push(
        WireExternalDirection::SuiteToTarget,
        WireExternalFrame::RouteHint,
        json!({ "session_id": session_id, "operation_id": operation_id }),
    );
    session
        .send_cache_reference(cache_reference(), CACHE_BODY.to_vec())
        .await?;
    observed.push(
        WireExternalDirection::SuiteToTarget,
        WireExternalFrame::CacheReference,
        json!({ "session_id": session_id }),
    );

    let miss = match session.await_event().await? {
        NnrpClientEvent::CacheMiss { metadata, body } => {
            observed.push(
                WireExternalDirection::TargetToSuite,
                WireExternalFrame::CacheMiss,
                json!({
                    "session_id": session_id,
                    "cache_namespace": metadata.cache_namespace,
                    "cache_key_hi": metadata.cache_key_hi,
                    "cache_key_lo": metadata.cache_key_lo,
                    "miss_reason": metadata.miss_reason as u16,
                    "diagnostic_bytes": body.len(),
                }),
            );
            metadata
        }
        _ => {
            return Err(RuntimeError::UnexpectedMessage(
                "capability/cache scenario expected CACHE_MISS",
            ));
        }
    };
    let result = match session.await_event().await? {
        NnrpClientEvent::Result(result) => result,
        _ => {
            return Err(RuntimeError::UnexpectedMessage(
                "capability/cache scenario expected RESULT_PUSH",
            ));
        }
    };
    observed.push(
        WireExternalDirection::TargetToSuite,
        WireExternalFrame::ResultPush,
        json!({ "session_id": session_id, "frame_id": result.frame_id }),
    );
    if miss != cache_miss() || result.body != RESPONSE_BODY {
        return Err(RuntimeError::UnexpectedMessage(
            "capability/cache scenario received unexpected target data",
        ));
    }
    session.close().await?;
    Ok(report(
        case,
        started,
        WireExternalTerminal::Success,
        observed.frames,
        None,
        None,
        Some(miss),
    ))
}

async fn run_progress_backpressure_server(
    case: WireExternalCase,
    endpoint: &WireReferenceEndpoint,
) -> Result<WireExternalCaseReport, RuntimeError> {
    let started = Instant::now();
    let mut observed = ObservedFrames::new(started);
    let server = endpoint.bind().await?;
    let mut session = server.accept().await?;
    let session_id = session.session_id();
    let submit = session.receive_submit().await?;
    observed.push(
        WireExternalDirection::TargetToSuite,
        WireExternalFrame::Request,
        json!({
            "session_id": session_id,
            "frame_id": submit.frame_id,
            "operation_id": submit.operation_id,
        }),
    );
    session
        .send_progress(progress(submit.operation_id), PROGRESS_BODY.to_vec())
        .await?;
    observed.push(
        WireExternalDirection::SuiteToTarget,
        WireExternalFrame::Progress,
        json!({ "session_id": session_id, "operation_id": submit.operation_id }),
    );
    session.send_credit_update(credit_update()).await?;
    observed.push(
        WireExternalDirection::SuiteToTarget,
        WireExternalFrame::CreditUpdate,
        json!({ "session_id": session_id, "max_in_flight": 1 }),
    );
    session
        .send_partial_result(partial_result(submit.operation_id), PARTIAL_BODY.to_vec())
        .await?;
    observed.push(
        WireExternalDirection::SuiteToTarget,
        WireExternalFrame::PartialResult,
        json!({ "session_id": session_id, "operation_id": submit.operation_id }),
    );
    session
        .send_result(submit.frame_id, token_result(), RESPONSE_BODY.to_vec())
        .await?;
    let close = session.receive_close().await?;
    session.ack_close(&close).await?;
    session.close().await?;
    Ok(report(
        case,
        started,
        WireExternalTerminal::Success,
        observed.frames,
        None,
        None,
        None,
    ))
}

async fn run_priority_deadline_proxy(
    case: WireExternalCase,
    upstream_endpoint: &WireReferenceEndpoint,
) -> Result<WireExternalCaseReport, RuntimeError> {
    let started = Instant::now();
    let (front_config, front_certificate) = QuicServerEndpointConfig::self_signed_localhost(
        "127.0.0.1:0"
            .parse()
            .expect("fixed proxy bind address must parse"),
    )?;
    let front_listener = QuicFramedListener::bind(&front_config)?;
    let front_address = front_listener.local_addr()?;
    let front_server =
        NnrpServer::from_listener(front_listener, quic_server_config(Default::default()))?;
    let front_certificate_der = front_certificate.certificate_der;

    let proxy = async {
        let mut downstream = front_server.accept().await?;
        let mut upstream = upstream_endpoint.connect().await?.open_session().await?;
        let submit = downstream.receive_submit().await?;
        let upstream_frame_id = upstream
            .submit_nowait(token_submit(submit.operation_id), submit.body)
            .await?;
        upstream.update_priority(submit.operation_id, 10, 0).await?;
        upstream.expire_at(submit.operation_id, 1).await?;
        let drop_reason = match upstream.await_event().await? {
            NnrpClientEvent::ResultDropReason { metadata, .. } => metadata,
            _ => {
                return Err(RuntimeError::UnexpectedMessage(
                    "priority/deadline proxy expected RESULT_DROP_REASON",
                ));
            }
        };
        if upstream_frame_id == 0 || drop_reason.operation_id != submit.operation_id {
            return Err(RuntimeError::UnexpectedMessage(
                "priority/deadline proxy received mismatched upstream state",
            ));
        }
        downstream.send_result_drop_reason(drop_reason).await?;
        upstream.close().await?;
        let close = downstream.receive_close().await?;
        downstream.ack_close(&close).await?;
        downstream.close().await?;
        Ok::<_, RuntimeError>(drop_reason)
    };

    let probe = async {
        let front_client_config =
            QuicClientEndpointConfig::localhost_with_root_certificate(front_certificate_der);
        let front_client = QuicProvider::connect(
            &front_address.to_string(),
            front_client_config,
            quic_client_config(Default::default()),
        )
        .await?;
        let mut session = front_client.open_session().await?;
        let operation_id = 201;
        let frame_id = session
            .submit_nowait(token_submit(operation_id), REQUEST_BODY.to_vec())
            .await?;
        let drop_reason = match session.await_event().await? {
            NnrpClientEvent::ResultDropReason { metadata, .. } => metadata,
            _ => {
                return Err(RuntimeError::UnexpectedMessage(
                    "priority/deadline probe expected RESULT_DROP_REASON",
                ));
            }
        };
        session.close().await?;
        Ok::<_, RuntimeError>((frame_id, drop_reason))
    };

    let (proxy_drop, (frame_id, probe_drop)) = tokio::try_join!(proxy, probe)?;
    if proxy_drop != probe_drop {
        return Err(RuntimeError::UnexpectedMessage(
            "priority/deadline proxy changed the typed drop reason",
        ));
    }
    let mut observed = ObservedFrames::new(started);
    observed.push(
        WireExternalDirection::ProbeToSuiteProxy,
        WireExternalFrame::Request,
        json!({ "frame_id": frame_id, "operation_id": probe_drop.operation_id }),
    );
    observed.push(
        WireExternalDirection::SuiteProxyToTarget,
        WireExternalFrame::PriorityUpdate,
        json!({ "operation_id": probe_drop.operation_id, "priority": 10 }),
    );
    observed.push(
        WireExternalDirection::SuiteProxyToTarget,
        WireExternalFrame::ExpireAt,
        json!({ "operation_id": probe_drop.operation_id, "unix_ms": 1 }),
    );
    observed.push(
        WireExternalDirection::TargetThroughSuiteProxyToProbe,
        WireExternalFrame::ResultDropReason,
        json!({
            "operation_id": probe_drop.operation_id,
            "drop_reason_code": probe_drop.drop_reason_code,
        }),
    );
    Ok(report(
        case,
        started,
        WireExternalTerminal::Dropped,
        observed.frames,
        Some(probe_drop),
        None,
        None,
    ))
}

fn report(
    case: WireExternalCase,
    started: Instant,
    terminal: WireExternalTerminal,
    observed_frames: Vec<WireExternalObservedFrame>,
    result_drop_reason: Option<ResultDropReasonMetadata>,
    trace_context: Option<TraceContextMetadata>,
    cache_miss: Option<CacheMissMetadata>,
) -> WireExternalCaseReport {
    WireExternalCaseReport {
        scenario_id: case.scenario_id(),
        mode: case.mode(),
        transport: case.transport(),
        terminal,
        elapsed_us: started.elapsed().as_micros(),
        observed_frames,
        result_drop_reason,
        trace_context,
        cache_miss,
    }
}

fn token_submit(operation_id: u64) -> FrameSubmitMetadata {
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
        operation_id,
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

fn progress(operation_id: u64) -> ProgressMetadata {
    ProgressMetadata {
        operation_id,
        progress_sequence: 1,
        stage_code: 1,
        percent_x100: 2_500,
        object_id: 0,
        body_bytes: PROGRESS_BODY.len() as u32,
    }
}

fn partial_result(operation_id: u64) -> PartialResultMetadata {
    PartialResultMetadata {
        operation_id,
        result_sequence: 1,
        object_id: 0,
        delta_sequence: 0,
        body_bytes: PARTIAL_BODY.len() as u32,
        flags: 0,
    }
}

fn credit_update() -> PressureMetadata {
    PressureMetadata {
        scope_id: 1,
        credit_window: 1,
        pressure_level: 0,
        pressure_reason: 0,
        retry_after_ms: 0,
        flags: 0,
    }
}

fn capability_metadata() -> CapabilityMetadata {
    CapabilityMetadata {
        profile_id: STANDARD_PROFILE_TOKEN,
        capability_count: 2,
        cost_model_id: 1,
        preference_rank: 1,
        limit_bytes: 4096,
        limit_units: 8,
        body_bytes: CAPABILITY_BODY.len() as u32,
        flags: 0,
    }
}

fn route_hint(operation_id: u64) -> RouteHintMetadata {
    RouteHintMetadata {
        operation_id,
        route_id: 92,
        executor_class: 3,
        affinity_class: 4,
        deadline_unix_ms: 1_800_000_000_000,
        body_bytes: ROUTE_BODY.len() as u32,
        flags: 0,
    }
}

fn cache_reference() -> CacheReferenceMetadata {
    CacheReferenceMetadata {
        cache_namespace: 1,
        cache_key_hi: 1_234_605_616_436_508_552,
        cache_key_lo: 11_072_869_122_414_935_808,
        profile_id: STANDARD_PROFILE_TOKEN,
        reuse_scope: CacheReuseScope::Session,
        lease_id: 0,
        producer_trace_id: 99,
        expiration_hint_ms: 1_000,
        metadata_bytes: CACHE_BODY.len() as u32,
        flags: 0,
    }
}

fn cache_miss() -> CacheMissMetadata {
    let reference = cache_reference();
    CacheMissMetadata {
        cache_namespace: reference.cache_namespace,
        cache_key_hi: reference.cache_key_hi,
        cache_key_lo: reference.cache_key_lo,
        miss_reason: CacheMissReason::NotFound,
        profile_id: reference.profile_id,
        diagnostic_bytes: 0,
    }
}

pub fn cancel_trace() -> TraceContextMetadata {
    TraceContextMetadata {
        trace_id: 0x1234,
        span_id: 0x5678,
        parent_span_id: 0,
        stage_code: 1,
        flags: 0,
        body_bytes: TRACE_BODY.len() as u32,
    }
}

pub fn cancel_drop_reason(operation_id: u64) -> ResultDropReasonMetadata {
    ResultDropReasonMetadata {
        operation_id,
        result_sequence: 1,
        drop_reason_code: RESULT_DROP_REASON_DEADLINE_EXPIRED,
        source_role: 2,
        flags: 0,
        diagnostic_bytes: 0,
    }
}

pub fn canonical_cache_miss() -> CacheMissMetadata {
    cache_miss()
}

pub fn canonical_result() -> ResultPushMetadata {
    token_result()
}

pub fn canonical_response_body() -> &'static [u8] {
    RESPONSE_BODY
}

pub fn canonical_trace_body() -> &'static [u8] {
    TRACE_BODY
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, str::FromStr, time::Duration};

    use nnrp_runtime::{NnrpClientEvent, NnrpServerEvent, RuntimeError};
    use nnrp_transport_ipc::IpcEndpoint;
    use nnrp_transport_quic::QuicServerEndpointConfig;

    use super::{
        cache_miss, cancel_drop_reason, cancel_trace, canonical_response_body,
        run_wire_external_case, token_result, token_submit, WireExternalCase, WireExternalMode,
        WireExternalTerminal, CACHE_BODY, CAPABILITY_BODY, PARTIAL_BODY, PROGRESS_BODY,
        REQUEST_BODY, RESPONSE_BODY, ROUTE_BODY, TRACE_BODY,
    };
    use crate::wire_endpoint::{ReferenceTransport, WireEndpointSecurity, WireReferenceEndpoint};

    #[tokio::test]
    async fn external_cancel_case_drives_a_real_tcp_target() {
        let endpoint =
            WireReferenceEndpoint::plain(ReferenceTransport::Tcp, free_tcp_address().to_string());
        let server = endpoint
            .bind()
            .await
            .expect("target TCP listener should bind");
        let target = tokio::spawn(cancel_target(server));

        let report = run_wire_external_case(WireExternalCase::CancelAbortClient, &endpoint)
            .await
            .expect("cancel case should pass");
        target
            .await
            .expect("target task should join")
            .expect("target should complete");
        assert_eq!(report.terminal, WireExternalTerminal::Cancelled);
        assert_eq!(report.mode, WireExternalMode::SuiteAsClient);
    }

    #[tokio::test]
    async fn external_ipc_cancel_case_uses_the_ipc_provider() {
        let ipc = unique_ipc_endpoint();
        let endpoint = WireReferenceEndpoint::plain(ReferenceTransport::Ipc, ipc.to_string());
        let server = endpoint
            .bind()
            .await
            .expect("target IPC listener should bind");
        let target = tokio::spawn(cancel_target(server));

        let report = run_wire_external_case(WireExternalCase::CancelAbortIpcClient, &endpoint)
            .await
            .expect("IPC cancel case should pass");
        target
            .await
            .expect("target task should join")
            .expect("target should complete");
        assert_eq!(report.transport, ReferenceTransport::Ipc);
        cleanup_ipc_endpoint(&ipc);
    }

    #[tokio::test]
    async fn external_cache_case_drives_a_real_quic_target() {
        let endpoint = secure_quic_endpoint();
        let server = endpoint
            .bind()
            .await
            .expect("target QUIC listener should bind");
        let target = tokio::spawn(cache_target(server));

        let report =
            run_wire_external_case(WireExternalCase::CapabilityRouteCacheClient, &endpoint)
                .await
                .expect("cache case should pass");
        target
            .await
            .expect("target task should join")
            .expect("target should complete");
        assert_eq!(report.terminal, WireExternalTerminal::Success);
        assert_eq!(report.transport, ReferenceTransport::Quic);
    }

    #[tokio::test]
    async fn external_progress_case_accepts_a_real_tcp_target_client() {
        let endpoint =
            WireReferenceEndpoint::plain(ReferenceTransport::Tcp, free_tcp_address().to_string());
        let target_endpoint = endpoint.clone();
        let target = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            progress_target(target_endpoint).await
        });

        let report =
            run_wire_external_case(WireExternalCase::ProgressBackpressureServer, &endpoint)
                .await
                .expect("progress case should pass");
        target
            .await
            .expect("target task should join")
            .expect("target should complete");
        assert_eq!(report.mode, WireExternalMode::SuiteAsServer);
    }

    #[tokio::test]
    async fn external_websocket_progress_case_uses_the_websocket_provider() {
        let endpoint = WireReferenceEndpoint::plain(
            ReferenceTransport::WebSocket,
            format!("ws://{}/nnrp", free_tcp_address()),
        );
        let target_endpoint = endpoint.clone();
        let target = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            progress_target(target_endpoint).await
        });

        let report = run_wire_external_case(
            WireExternalCase::ProgressBackpressureWebSocketServer,
            &endpoint,
        )
        .await
        .expect("WebSocket progress case should pass");
        target
            .await
            .expect("target task should join")
            .expect("target should complete");
        assert_eq!(report.transport, ReferenceTransport::WebSocket);
    }

    #[tokio::test]
    async fn external_secure_websocket_progress_case_uses_tls_material() {
        let endpoint = secure_websocket_endpoint();
        let target_endpoint = endpoint.clone();
        let target = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            progress_target(target_endpoint).await
        });

        let report = run_wire_external_case(
            WireExternalCase::ProgressBackpressureWebSocketServer,
            &endpoint,
        )
        .await
        .expect("secure WebSocket progress case should pass");
        target
            .await
            .expect("target task should join")
            .expect("target should complete");
        assert_eq!(report.transport, ReferenceTransport::WebSocket);
    }

    #[tokio::test]
    async fn external_priority_case_terminates_and_injects_through_a_real_quic_proxy() {
        let endpoint = secure_quic_endpoint();
        let server = endpoint
            .bind()
            .await
            .expect("target QUIC listener should bind");
        let target = tokio::spawn(priority_target(server));

        let report = run_wire_external_case(WireExternalCase::PriorityDeadlineProxy, &endpoint)
            .await
            .expect("priority/deadline proxy case should pass");
        target
            .await
            .expect("target task should join")
            .expect("target should complete");
        assert_eq!(report.terminal, WireExternalTerminal::Dropped);
        assert_eq!(report.mode, WireExternalMode::SuiteAsProxy);
    }

    async fn cancel_target(server: nnrp_runtime::NnrpServer) -> Result<(), RuntimeError> {
        let mut session = server.accept().await?;
        let submit = session.receive_submit().await?;
        match session.await_event().await? {
            NnrpServerEvent::Control(control)
                if control.message_type == nnrp_core::MessageType::Cancel => {}
            _ => {
                return Err(RuntimeError::UnexpectedMessage(
                    "cancel target expected CANCEL",
                ));
            }
        }
        session
            .send_trace_context(submit.frame_id, cancel_trace(), TRACE_BODY.to_vec())
            .await?;
        session
            .send_result_drop_reason(cancel_drop_reason(submit.operation_id))
            .await?;
        close_server_session(&mut session).await
    }

    async fn cache_target(server: nnrp_runtime::NnrpServer) -> Result<(), RuntimeError> {
        let mut session = server.accept().await?;
        let submit = session.receive_submit().await?;
        match session.await_event().await? {
            NnrpServerEvent::Capability { body, .. } if body == CAPABILITY_BODY => {}
            _ => {
                return Err(RuntimeError::UnexpectedMessage(
                    "target expected capability",
                ))
            }
        }
        match session.await_event().await? {
            NnrpServerEvent::RouteHint { body, .. } if body == ROUTE_BODY => {}
            _ => {
                return Err(RuntimeError::UnexpectedMessage(
                    "target expected route hint",
                ))
            }
        }
        match session.await_event().await? {
            NnrpServerEvent::CacheReference { body, .. } if body == CACHE_BODY => {}
            _ => {
                return Err(RuntimeError::UnexpectedMessage(
                    "target expected cache reference",
                ));
            }
        }
        session.send_cache_miss(cache_miss(), Vec::new()).await?;
        session
            .send_result(submit.frame_id, token_result(), RESPONSE_BODY.to_vec())
            .await?;
        close_server_session(&mut session).await
    }

    async fn priority_target(server: nnrp_runtime::NnrpServer) -> Result<(), RuntimeError> {
        let mut session = server.accept().await?;
        let submit = session.receive_submit().await?;
        for expected in [
            nnrp_core::MessageType::PriorityUpdate,
            nnrp_core::MessageType::ExpireAt,
        ] {
            match session.await_event().await? {
                NnrpServerEvent::Scheduling(update) if update.message_type == expected => {}
                _ => {
                    return Err(RuntimeError::UnexpectedMessage(
                        "priority target received unexpected scheduling frame",
                    ));
                }
            }
        }
        session
            .send_result_drop_reason(cancel_drop_reason(submit.operation_id))
            .await?;
        close_server_session(&mut session).await
    }

    async fn progress_target(endpoint: WireReferenceEndpoint) -> Result<(), RuntimeError> {
        let mut session = endpoint.connect().await?.open_session().await?;
        let operation_id = 301;
        session
            .submit_nowait(token_submit(operation_id), REQUEST_BODY.to_vec())
            .await?;
        match session.await_event().await? {
            NnrpClientEvent::Progress { body, .. } if body == PROGRESS_BODY => {}
            _ => return Err(RuntimeError::UnexpectedMessage("target expected progress")),
        }
        match session.await_event().await? {
            NnrpClientEvent::CreditUpdate(metadata) if metadata.credit_window == 1 => {}
            _ => {
                return Err(RuntimeError::UnexpectedMessage(
                    "target expected credit update",
                ));
            }
        }
        match session.await_event().await? {
            NnrpClientEvent::PartialResult { body, .. } if body == PARTIAL_BODY => {}
            _ => {
                return Err(RuntimeError::UnexpectedMessage(
                    "target expected partial result",
                ));
            }
        }
        match session.await_event().await? {
            NnrpClientEvent::Result(result)
                if result.metadata == token_result()
                    && result.body.as_slice() == canonical_response_body() => {}
            _ => return Err(RuntimeError::UnexpectedMessage("target expected result")),
        }
        session.close().await
    }

    async fn close_server_session(
        session: &mut nnrp_runtime::NnrpServerSession,
    ) -> Result<(), RuntimeError> {
        let close = session.receive_close().await?;
        session.ack_close(&close).await?;
        session.close_in_place().await
    }

    fn free_tcp_address() -> SocketAddr {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("temporary TCP listener should bind");
        listener
            .local_addr()
            .expect("temporary TCP listener should expose its address")
    }

    fn free_udp_address() -> SocketAddr {
        let socket =
            std::net::UdpSocket::bind("127.0.0.1:0").expect("temporary UDP socket should bind");
        socket
            .local_addr()
            .expect("temporary UDP socket should expose its address")
    }

    fn secure_quic_endpoint() -> WireReferenceEndpoint {
        let (config, certificate) =
            QuicServerEndpointConfig::self_signed_localhost(free_udp_address())
                .expect("test certificate should generate");
        WireReferenceEndpoint::secure(
            ReferenceTransport::Quic,
            config.bind_addr.to_string(),
            WireEndpointSecurity {
                server_name: "localhost".to_string(),
                trusted_certificate_der: certificate.certificate_der.clone(),
                certificate_der: certificate.certificate_der,
                private_key_pkcs8_der: certificate.private_key_pkcs8_der,
            },
        )
    }

    fn secure_websocket_endpoint() -> WireReferenceEndpoint {
        let (_, certificate) = QuicServerEndpointConfig::self_signed_localhost(free_udp_address())
            .expect("test certificate should generate");
        WireReferenceEndpoint::secure(
            ReferenceTransport::WebSocket,
            format!("wss://localhost:{}/nnrp", free_tcp_address().port()),
            WireEndpointSecurity {
                server_name: "localhost".to_string(),
                trusted_certificate_der: certificate.certificate_der.clone(),
                certificate_der: certificate.certificate_der,
                private_key_pkcs8_der: certificate.private_key_pkcs8_der,
            },
        )
    }

    fn unique_ipc_endpoint() -> IpcEndpoint {
        #[cfg(unix)]
        {
            IpcEndpoint::unix(std::env::temp_dir().join(format!(
                "nnrp-wire-external-{}-{}.sock",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock should be after Unix epoch")
                    .as_nanos()
            )))
        }
        #[cfg(windows)]
        {
            IpcEndpoint::named_pipe(format!(
                "nnrp-wire-external-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock should be after Unix epoch")
                    .as_nanos()
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

    #[test]
    fn external_case_contract_matches_frozen_scenario_roles() {
        assert_eq!(
            WireExternalCase::PriorityDeadlineProxy.mode(),
            WireExternalMode::SuiteAsProxy
        );
        assert_eq!(
            WireExternalCase::ProgressBackpressureServer.mode(),
            WireExternalMode::SuiteAsServer
        );
        assert_eq!(
            WireExternalCase::CapabilityRouteCacheClient.mode(),
            WireExternalMode::SuiteAsClient
        );
        assert!(IpcEndpoint::from_str("unix:///tmp/nnrp.sock").is_ok());
    }
}
