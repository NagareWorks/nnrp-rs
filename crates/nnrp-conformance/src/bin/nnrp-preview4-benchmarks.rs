use std::{
    error::Error,
    hint::black_box,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use nnrp_core::{
    CacheMissMetadata, CacheMissReason, CacheReferenceMetadata, CacheReuseScope,
    FrameSubmitMetadata, InputProfile, MemoryLocationHint, ObjectDescriptorMetadata,
    ObjectReferenceMetadata, ObjectReleaseMetadata, ObjectReleaseReason, OwnershipHint,
    PartialResultMetadata, PayloadKindBitmap, PressureMetadata, ProgressMetadata, ResultClass,
    ResultPushMetadata, RuntimeObjectKind, RuntimeRole, SchedulingMetadata, SubmitMode,
    TileIndexMode, STANDARD_PROFILE_TOKEN,
};
use nnrp_runtime::{NnrpClient, NnrpClientConfig, NnrpResult, NnrpServerConfig, RuntimeError};
use nnrp_transport_ipc::{IpcEndpoint, IpcProvider};
use nnrp_transport_websocket::{WebSocketEndpoint, WebSocketProvider};
use serde_json::json;

#[derive(Debug, Clone, Copy)]
struct BenchCase {
    iterations: u64,
    operations: u64,
    elapsed: Duration,
}

impl BenchCase {
    fn to_json(self) -> serde_json::Value {
        let elapsed_ns = self.elapsed.as_nanos();
        let ops_per_second = if elapsed_ns == 0 {
            0.0
        } else {
            (self.operations as f64) * 1_000_000_000.0 / elapsed_ns as f64
        };
        json!({
            "iterations": self.iterations,
            "operations": self.operations,
            "elapsed_ms": elapsed_ns as f64 / 1_000_000.0,
            "ops_per_second": ops_per_second,
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = BenchArgs::parse(std::env::args().skip(1))?;
    let control = bench_control_frames(args.iterations)?;
    let runtime_objects = bench_runtime_objects(args.iterations)?;
    let ipc = bench_ipc_loopback(args.transport_iterations).await?;
    let websocket = bench_websocket_loopback(args.transport_iterations).await?;

    let report = json!({
        "protocol_version": "nnrp-1-preview4",
        "iterations": {
            "metadata_hot_path": args.iterations,
            "transport_loopback": args.transport_iterations,
        },
        "benchmarks": {
            "control_frame_hot_path": control.to_json(),
            "runtime_object_declare_ref_release": runtime_objects.to_json(),
            "ipc_loopback": ipc.to_json(),
            "websocket_loopback": websocket.to_json(),
        }
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct BenchArgs {
    iterations: u64,
    transport_iterations: u64,
}

impl BenchArgs {
    fn parse<I, S>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut iterations = 100_000;
        let mut transport_iterations = 1_000;
        let mut iter = args.into_iter().map(Into::into);
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--iterations" => {
                    iterations = parse_positive(next_arg(&mut iter, "--iterations")?)?
                }
                "--transport-iterations" => {
                    transport_iterations =
                        parse_positive(next_arg(&mut iter, "--transport-iterations")?)?
                }
                "--help" | "-h" => return Err(usage()),
                value => return Err(format!("unknown benchmark argument '{value}'")),
            }
        }
        Ok(Self {
            iterations,
            transport_iterations,
        })
    }
}

fn next_arg(iter: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    iter.next()
        .ok_or_else(|| format!("{flag} requires a positive integer value"))
}

fn parse_positive(value: String) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("'{value}' is not a positive integer"))?;
    if parsed == 0 {
        return Err("benchmark iteration count must be greater than zero".to_string());
    }
    Ok(parsed)
}

fn usage() -> String {
    "usage: nnrp-preview4-benchmarks [--iterations <n>] [--transport-iterations <n>]".to_string()
}

fn bench_control_frames(iterations: u64) -> Result<BenchCase, Box<dyn Error>> {
    let progress_body = b"stage";
    let progress = ProgressMetadata {
        operation_id: 7,
        progress_sequence: 3,
        stage_code: 2,
        percent_x100: 2_500,
        object_id: 42,
        body_bytes: progress_body.len() as u32,
    };
    let partial_body = b"partial";
    let partial = PartialResultMetadata {
        operation_id: 7,
        result_sequence: 4,
        object_id: 42,
        delta_sequence: 1,
        body_bytes: partial_body.len() as u32,
        flags: 0,
    };
    let pressure = PressureMetadata {
        scope_id: 7,
        credit_window: 16,
        pressure_level: 1,
        pressure_reason: 2,
        retry_after_ms: 8,
        flags: 0,
    };
    let scheduling = SchedulingMetadata {
        operation_id: 7,
        control_sequence: 5,
        priority_class: 2,
        priority_delta: -1,
        deadline_unix_ms: 4_102_444_800_000,
        flags: 0,
    };

    let start = Instant::now();
    for _ in 0..iterations {
        let progress_bytes = progress.to_vec_with_body(progress_body)?;
        let (metadata, body) = ProgressMetadata::parse_with_body(black_box(&progress_bytes))?;
        black_box((metadata, body.len()));

        let partial_bytes = partial.to_vec_with_body(partial_body)?;
        let (metadata, body) = PartialResultMetadata::parse_with_body(black_box(&partial_bytes))?;
        black_box((metadata, body.len()));

        let pressure_bytes = pressure.to_bytes()?;
        black_box(PressureMetadata::parse(black_box(&pressure_bytes))?);

        let scheduling_bytes = scheduling.to_bytes()?;
        black_box(SchedulingMetadata::parse(black_box(&scheduling_bytes))?);
    }
    Ok(BenchCase {
        iterations,
        operations: iterations * 4,
        elapsed: start.elapsed(),
    })
}

fn bench_runtime_objects(iterations: u64) -> Result<BenchCase, Box<dyn Error>> {
    let object_meta = br#"{"shape":[1,1024],"dtype":"f16"}"#;
    let object = ObjectDescriptorMetadata {
        object_id: 91,
        object_kind: RuntimeObjectKind::Tensor,
        producer_role: RuntimeRole::Runtime,
        consumer_role: RuntimeRole::Client,
        session_id: 7,
        byte_size: 2048,
        compute_cost_units: 32,
        memory_location_hint: MemoryLocationHint::DeviceMemory,
        ownership_hint: OwnershipHint::Borrowed,
        lifetime_hint_ms: 250,
        metadata_bytes: object_meta.len() as u32,
    };
    let object_ref_meta = br#"{"range":"full"}"#;
    let object_ref = ObjectReferenceMetadata {
        object_id: 91,
        operation_id: 7,
        object_version: 1,
        offset: 0,
        length: 2048,
        flags: 0,
        metadata_bytes: object_ref_meta.len() as u32,
    };
    let release_diag = b"complete";
    let release = ObjectReleaseMetadata {
        object_id: 91,
        operation_id: 7,
        release_reason: ObjectReleaseReason::Completed,
        source_role: RuntimeRole::Runtime,
        flags: 0,
        diagnostic_bytes: release_diag.len() as u32,
    };
    let cache_meta = br#"{"tenant":"bench"}"#;
    let cache = CacheReferenceMetadata {
        cache_namespace: 42,
        cache_key_hi: 1,
        cache_key_lo: 2,
        profile_id: STANDARD_PROFILE_TOKEN,
        reuse_scope: CacheReuseScope::Session,
        lease_id: 9,
        producer_trace_id: 11,
        expiration_hint_ms: 500,
        metadata_bytes: cache_meta.len() as u32,
        flags: 0,
    };
    let miss_diag = b"not-found";
    let miss = CacheMissMetadata {
        cache_namespace: 42,
        cache_key_hi: 1,
        cache_key_lo: 3,
        miss_reason: CacheMissReason::NotFound,
        profile_id: STANDARD_PROFILE_TOKEN,
        diagnostic_bytes: miss_diag.len() as u32,
    };

    let start = Instant::now();
    for _ in 0..iterations {
        let bytes = object.to_vec_with_extension(object_meta)?;
        let (metadata, extension) =
            ObjectDescriptorMetadata::parse_with_extension(black_box(&bytes))?;
        black_box((metadata, extension.len()));

        let bytes = object_ref.to_vec_with_extension(object_ref_meta)?;
        let (metadata, extension) =
            ObjectReferenceMetadata::parse_with_extension(black_box(&bytes))?;
        black_box((metadata, extension.len()));

        let bytes = release.to_vec_with_diagnostics(release_diag)?;
        let (metadata, diagnostics) =
            ObjectReleaseMetadata::parse_with_diagnostics(black_box(&bytes))?;
        black_box((metadata, diagnostics.len()));

        let bytes = cache.to_vec_with_extension(cache_meta)?;
        let (metadata, extension) =
            CacheReferenceMetadata::parse_with_extension(black_box(&bytes))?;
        black_box((metadata, extension.len()));

        let bytes = miss.to_vec_with_diagnostics(miss_diag)?;
        let (metadata, diagnostics) = CacheMissMetadata::parse_with_diagnostics(black_box(&bytes))?;
        black_box((metadata, diagnostics.len()));
    }
    Ok(BenchCase {
        iterations,
        operations: iterations * 5,
        elapsed: start.elapsed(),
    })
}

async fn bench_ipc_loopback(iterations: u64) -> Result<BenchCase, Box<dyn Error>> {
    let endpoint = unique_ipc_endpoint();
    cleanup_ipc_endpoint(&endpoint);
    let server = IpcProvider::bind(&endpoint, NnrpServerConfig::default()).await?;
    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        for _ in 0..iterations {
            let submit = session.receive_submit().await?;
            session
                .send_result(submit.frame_id, token_result(), b"ok".to_vec())
                .await?;
        }
        Ok::<(), RuntimeError>(())
    });

    let start = Instant::now();
    let client = connect_ipc_client_with_retry(&endpoint).await?;
    let mut session = client.open_session().await?;
    for operation_id in 1..=iterations {
        session
            .submit(token_submit(operation_id), b"hello".to_vec())
            .await?;
        let NnrpResult { body, .. } = session.await_result().await?;
        black_box(body);
    }
    server_task
        .await
        .map_err(|_| RuntimeError::Internal("IPC benchmark server task panicked"))??;
    cleanup_ipc_endpoint(&endpoint);
    Ok(BenchCase {
        iterations,
        operations: iterations,
        elapsed: start.elapsed(),
    })
}

async fn bench_websocket_loopback(iterations: u64) -> Result<BenchCase, Box<dyn Error>> {
    let server = WebSocketProvider::bind("127.0.0.1:0", NnrpServerConfig::default()).await?;
    let endpoint = WebSocketEndpoint::ws(format!("ws://{}", server.local_addr()?))?;
    let server_task = tokio::spawn(async move {
        let mut session = server.accept().await?;
        for _ in 0..iterations {
            let submit = session.receive_submit().await?;
            session
                .send_result(submit.frame_id, token_result(), b"ok".to_vec())
                .await?;
        }
        Ok::<(), RuntimeError>(())
    });

    let start = Instant::now();
    let client = WebSocketProvider::connect(&endpoint, NnrpClientConfig::default()).await?;
    let mut session = client.open_session().await?;
    for operation_id in 1..=iterations {
        session
            .submit(token_submit(operation_id), b"hello".to_vec())
            .await?;
        let NnrpResult { body, .. } = session.await_result().await?;
        black_box(body);
    }
    server_task
        .await
        .map_err(|_| RuntimeError::Internal("WebSocket benchmark server task panicked"))??;
    Ok(BenchCase {
        iterations,
        operations: iterations,
        elapsed: start.elapsed(),
    })
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

async fn connect_ipc_client_with_retry(endpoint: &IpcEndpoint) -> Result<NnrpClient, RuntimeError> {
    let mut last_error = None;
    for _ in 0..50 {
        match IpcProvider::connect(endpoint, NnrpClientConfig::default()).await {
            Ok(client) => return Ok(client),
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }
    Err(last_error.unwrap_or(RuntimeError::Internal(
        "IPC benchmark client connection retry exhausted",
    )))
}

#[cfg(unix)]
fn unique_ipc_endpoint() -> IpcEndpoint {
    let path = std::env::temp_dir().join(format!(
        "nnrp-preview4-bench-{}-{}.sock",
        std::process::id(),
        monotonic_suffix()
    ));
    IpcEndpoint::unix(path)
}

#[cfg(windows)]
fn unique_ipc_endpoint() -> IpcEndpoint {
    IpcEndpoint::named_pipe(format!(
        "nnrp-preview4-bench-{}-{}",
        std::process::id(),
        monotonic_suffix()
    ))
}

fn monotonic_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn cleanup_ipc_endpoint(endpoint: &IpcEndpoint) {
    if let Some(path) = endpoint.as_unix_path() {
        let _ = std::fs::remove_file(path);
    }
}
