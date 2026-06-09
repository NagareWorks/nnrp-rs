use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const WIRE_DRY_RUN_REPORT_SCHEMA: &str =
    "https://raw.githubusercontent.com/NagareWorks/nnrp-conformance/main/schemas/wire-dry-run-results.schema.json";
const WIRE_TRANSPORTS: &[&str] = &["tcp", "quic", "ipc", "websocket"];
const WIRE_SCENARIO_MODES: &[&str] = &["suite-as-client", "suite-as-server", "suite-as-proxy"];
const WIRE_TARGET_ENDPOINT_MODES: &[&str] = &["client", "server"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireRunnerArguments {
    pub suite_manifest: PathBuf,
    pub target_manifest: PathBuf,
    pub output: PathBuf,
    pub selected_case_ids: Vec<String>,
}

pub fn parse_wire_arguments<I, S>(args: I) -> Result<WireRunnerArguments, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut suite_manifest = None;
    let mut target_manifest = None;
    let mut output = None;
    let mut selected_case_ids = Vec::new();
    let mut iter = args.into_iter().map(Into::into);

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--suite" => suite_manifest = Some(next_path(&mut iter, "--suite")?),
            "--target" => target_manifest = Some(next_path(&mut iter, "--target")?),
            "--output" => output = Some(next_path(&mut iter, "--output")?),
            "--case" => selected_case_ids.push(next_value(&mut iter, "--case")?),
            "--help" | "-h" => return Err(wire_usage()),
            value => return Err(format!("unknown wire conformance argument '{value}'")),
        }
    }

    Ok(WireRunnerArguments {
        suite_manifest: suite_manifest
            .ok_or_else(|| "wire suite manifest path is required via --suite".to_string())?,
        target_manifest: target_manifest
            .ok_or_else(|| "wire target manifest path is required via --target".to_string())?,
        output: output.ok_or_else(|| "wire result path is required via --output".to_string())?,
        selected_case_ids,
    })
}

pub fn write_wire_dry_run_report(
    suite_manifest_path: &Path,
    target_manifest_path: &Path,
    output_path: &Path,
    selected_case_ids: &[String],
) -> Result<(), String> {
    let suite_manifest = read_json_file(suite_manifest_path, "wire suite manifest")?;
    let target_manifest = read_json_file(target_manifest_path, "wire target manifest")?;
    let selected: Vec<&str> = selected_case_ids.iter().map(String::as_str).collect();
    let plan = build_wire_execution_plan(&suite_manifest, &target_manifest, &selected)?;
    let report = build_wire_dry_run_report(&plan)?;
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create wire result directory '{}': {error}",
                    parent.display()
                )
            })?;
        }
    }
    let bytes = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("failed to serialize wire dry-run report: {error}"))?;
    fs::write(output_path, bytes).map_err(|error| {
        format!(
            "failed to write wire dry-run report '{}': {error}",
            output_path.display()
        )
    })
}

pub fn build_wire_execution_plan(
    suite_manifest: &Value,
    target_manifest: &Value,
    selected_case_ids: &[&str],
) -> Result<Value, String> {
    let protocol_version = required_string(suite_manifest, "protocol_version")?;
    let scenarios = required_array(suite_manifest, "scenarios")?;
    let target_name = required_string(target_manifest, "target_name")?;
    let target_protocol = required_string(target_manifest, "protocol_version")?;
    if target_protocol != protocol_version {
        return Err(format!(
            "target protocol_version '{target_protocol}' does not match suite protocol_version '{protocol_version}'"
        ));
    }

    let endpoints = target_endpoint_transports(target_manifest)?;
    let capabilities = target_capabilities(target_manifest)?;
    let selected = selected_ids_or_all(scenarios, selected_case_ids)?;

    let mut planned_cases = Vec::new();
    for scenario in scenarios {
        let case = scenario
            .as_object()
            .ok_or_else(|| "wire suite scenarios must be JSON objects".to_string())?;
        let id = case.get("id").and_then(Value::as_str).ok_or_else(|| {
            "wire suite scenario field 'id' must be a non-empty string".to_string()
        })?;
        if !selected.contains(id) {
            continue;
        }

        let required_transport = case
            .get("required_transport")
            .and_then(Value::as_str)
            .unwrap_or("any");
        validate_wire_transport(
            required_transport,
            &format!("scenario '{id}' required_transport"),
            true,
        )?;
        let mode = case
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("suite-as-client");
        validate_wire_scenario_mode(mode, &format!("scenario '{id}' mode"))?;
        let required_capabilities = string_array_field(case.get("required_capabilities"))
            .map_err(|error| format!("scenario '{id}' {error}"))?;
        let missing_capabilities: Vec<&str> = required_capabilities
            .iter()
            .copied()
            .filter(|capability| !capabilities.contains(*capability))
            .collect();

        let unsupported_transport =
            required_transport != "any" && !endpoints.contains(required_transport);
        let runnable = !unsupported_transport && missing_capabilities.is_empty();
        let skip_reason = if unsupported_transport {
            Some(format!(
                "target does not claim required transport '{required_transport}'"
            ))
        } else if !missing_capabilities.is_empty() {
            Some(format!(
                "target is missing required capabilities: {}",
                missing_capabilities.join(", ")
            ))
        } else {
            None
        };

        planned_cases.push(json!({
            "id": id,
            "mode": mode,
            "required_transport": required_transport,
            "required_capabilities": required_capabilities,
            "status": if runnable { "ready" } else { "skipped" },
            "skip_reason": skip_reason,
            "evidence": {
                "frame_log": format!("wire/{id}/frames.ndjson"),
                "timing_trace": format!("wire/{id}/timing.json")
            }
        }));
    }

    Ok(json!({
        "protocol_version": protocol_version,
        "target_name": target_name,
        "case_count": planned_cases.len(),
        "cases": planned_cases,
    }))
}

pub fn build_wire_dry_run_report(plan: &Value) -> Result<Value, String> {
    let protocol_version = required_string(plan, "protocol_version")?;
    let target_name = required_string(plan, "target_name")?;
    let cases = required_array(plan, "cases")?;
    let results: Result<Vec<Value>, String> = cases
        .iter()
        .map(|case| {
            let case = case
                .as_object()
                .ok_or_else(|| "wire execution plan cases must be JSON objects".to_string())?;
            let id = case.get("id").and_then(Value::as_str).ok_or_else(|| {
                "wire execution plan case field 'id' must be a non-empty string".to_string()
            })?;
            let status = case.get("status").and_then(Value::as_str).ok_or_else(|| {
                "wire execution plan case field 'status' must be a non-empty string".to_string()
            })?;
            let outcome = if status == "ready" {
                "dry_run"
            } else {
                "skipped"
            };
            Ok(json!({
                "id": id,
                "outcome": outcome,
                "skip_reason": case.get("skip_reason").cloned().unwrap_or(Value::Null),
                "evidence": case.get("evidence").cloned().unwrap_or_else(|| json!({})),
            }))
        })
        .collect();

    Ok(json!({
        "$schema": WIRE_DRY_RUN_REPORT_SCHEMA,
        "protocol_version": protocol_version,
        "target_name": target_name,
        "mode": "dry-run",
        "results": results?,
    }))
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| format!("field '{field}' must be a non-empty string"))
}

fn required_array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("field '{field}' must be an array"))
}

fn target_endpoint_transports(target_manifest: &Value) -> Result<BTreeSet<String>, String> {
    let endpoints = required_array(target_manifest, "endpoints")?;
    let mut transports = BTreeSet::new();
    for endpoint in endpoints {
        let endpoint = endpoint
            .as_object()
            .ok_or_else(|| "target endpoints must be JSON objects".to_string())?;
        let transport = endpoint
            .get("transport")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                "target endpoint field 'transport' must be a non-empty string".to_string()
            })?;
        validate_wire_transport(transport, "target endpoint transport", false)?;
        let mode = endpoint
            .get("mode")
            .and_then(Value::as_str)
            .ok_or_else(|| "target endpoint field 'mode' must be a non-empty string".to_string())?;
        validate_wire_target_endpoint_mode(mode, "target endpoint mode")?;
        transports.insert(transport.to_string());
    }
    Ok(transports)
}

fn target_capabilities(target_manifest: &Value) -> Result<BTreeSet<String>, String> {
    let capabilities = string_array_field(target_manifest.get("capabilities"))?;
    Ok(capabilities
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>())
}

fn selected_ids_or_all(
    scenarios: &[Value],
    selected_case_ids: &[&str],
) -> Result<BTreeSet<String>, String> {
    let scenario_ids: BTreeMap<String, ()> = scenarios
        .iter()
        .map(|scenario| {
            let id = scenario.get("id").and_then(Value::as_str).ok_or_else(|| {
                "wire suite scenario field 'id' must be a non-empty string".to_string()
            })?;
            Ok((id.to_string(), ()))
        })
        .collect::<Result<_, String>>()?;

    if selected_case_ids.is_empty() {
        return Ok(scenario_ids.keys().cloned().collect());
    }

    let mut selected = BTreeSet::new();
    for id in selected_case_ids {
        if !scenario_ids.contains_key(*id) {
            return Err(format!(
                "selected wire scenario id '{id}' is not in the suite manifest"
            ));
        }
        selected.insert((*id).to_string());
    }
    Ok(selected)
}

fn string_array_field(value: Option<&Value>) -> Result<Vec<&str>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let entries = value
        .as_array()
        .ok_or_else(|| "field must be an array of strings".to_string())?;
    entries
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "field entries must be non-empty strings".to_string())
        })
        .collect()
}

fn validate_wire_transport(value: &str, label: &str, allow_any: bool) -> Result<(), String> {
    if (allow_any && value == "any") || WIRE_TRANSPORTS.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "{label} '{value}' must be one of {}",
            if allow_any {
                "any, tcp, quic, ipc, websocket"
            } else {
                "tcp, quic, ipc, websocket"
            }
        ))
    }
}

fn validate_wire_scenario_mode(value: &str, label: &str) -> Result<(), String> {
    if WIRE_SCENARIO_MODES.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "{label} '{value}' must be one of suite-as-client, suite-as-server, suite-as-proxy"
        ))
    }
}

fn validate_wire_target_endpoint_mode(value: &str, label: &str) -> Result<(), String> {
    if WIRE_TARGET_ENDPOINT_MODES.contains(&value) {
        Ok(())
    } else {
        Err(format!("{label} '{value}' must be one of client, server"))
    }
}

fn read_json_file(path: &Path, label: &str) -> Result<Value, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {label} '{}': {error}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("{label} '{}' must be valid JSON: {error}", path.display()))
}

fn next_path(iter: &mut impl Iterator<Item = String>, flag: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(next_value(iter, flag)?))
}

fn next_value(iter: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    iter.next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn wire_usage() -> String {
    "usage: nnrp-conformance-wire --suite <manifest.json> --target <target.json> --output <results.json> [--case <id>]..."
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        build_wire_dry_run_report, build_wire_execution_plan, parse_wire_arguments,
        write_wire_dry_run_report,
    };
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    fn suite_manifest() -> serde_json::Value {
        json!({
            "protocol_version": "nnrp-1-preview4",
            "scenarios": [
                {
                    "id": "wire.cancel.ipc",
                    "mode": "suite-as-client",
                    "required_transport": "ipc",
                    "required_capabilities": ["control.cancel_abort"]
                },
                {
                    "id": "wire.progress.websocket",
                    "mode": "suite-as-server",
                    "required_transport": "websocket",
                    "required_capabilities": ["control.progress_partial", "control.credit_backpressure"]
                }
            ]
        })
    }

    fn target_manifest() -> serde_json::Value {
        json!({
            "target_name": "reference-rs",
            "protocol_version": "nnrp-1-preview4",
            "endpoints": [
                {"transport": "ipc", "mode": "client", "uri": "nnrp+ipc://runtime.sock"},
                {"transport": "websocket", "mode": "server", "uri": "ws://127.0.0.1:0/nnrp"}
            ],
            "capabilities": [
                "control.cancel_abort",
                "control.progress_partial",
                "control.credit_backpressure"
            ]
        })
    }

    #[test]
    fn wire_execution_plan_preserves_selected_scenario_ids_and_evidence_paths() {
        let plan =
            build_wire_execution_plan(&suite_manifest(), &target_manifest(), &["wire.cancel.ipc"])
                .expect("plan");
        let cases = plan["cases"].as_array().expect("cases");
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0]["id"], "wire.cancel.ipc");
        assert_eq!(cases[0]["status"], "ready");
        assert_eq!(
            cases[0]["evidence"]["frame_log"],
            "wire/wire.cancel.ipc/frames.ndjson"
        );
        assert_eq!(plan["case_count"], 1);
    }

    #[test]
    fn wire_execution_plan_skips_missing_transport_or_capability_claims() {
        let target = json!({
            "target_name": "partial",
            "protocol_version": "nnrp-1-preview4",
            "endpoints": [{"transport": "ipc", "mode": "client", "uri": "nnrp+ipc://runtime.sock"}],
            "capabilities": ["control.cancel_abort"]
        });
        let plan = build_wire_execution_plan(&suite_manifest(), &target, &[]).expect("plan");
        let cases = plan["cases"].as_array().expect("cases");
        assert_eq!(cases[0]["status"], "ready");
        assert_eq!(cases[1]["status"], "skipped");
        assert!(cases[1]["skip_reason"]
            .as_str()
            .expect("skip reason")
            .contains("required transport 'websocket'"));
    }

    #[test]
    fn wire_dry_run_report_keeps_ready_and_skipped_outcomes_explicit() {
        let target = json!({
            "target_name": "partial",
            "protocol_version": "nnrp-1-preview4",
            "endpoints": [{"transport": "ipc", "mode": "client", "uri": "nnrp+ipc://runtime.sock"}],
            "capabilities": ["control.cancel_abort"]
        });
        let plan = build_wire_execution_plan(&suite_manifest(), &target, &[]).expect("plan");
        let report = build_wire_dry_run_report(&plan).expect("report");
        let results = report["results"].as_array().expect("results");
        assert_eq!(report["mode"], "dry-run");
        assert_eq!(results[0]["outcome"], "dry_run");
        assert_eq!(results[1]["outcome"], "skipped");
        assert_eq!(results[0]["id"], "wire.cancel.ipc");
        assert_eq!(results[1]["id"], "wire.progress.websocket");
    }

    #[test]
    fn wire_execution_plan_rejects_unknown_selected_scenarios() {
        assert_eq!(
            build_wire_execution_plan(&suite_manifest(), &target_manifest(), &["missing"])
                .unwrap_err(),
            "selected wire scenario id 'missing' is not in the suite manifest"
        );
    }

    #[test]
    fn wire_execution_plan_rejects_unknown_suite_modes_and_transports() {
        let suite = json!({
            "protocol_version": "nnrp-1-preview4",
            "scenarios": [{
                "id": "wire.invalid.mode",
                "mode": "sdk-adapter",
                "required_transport": "ipc"
            }]
        });
        assert_eq!(
            build_wire_execution_plan(&suite, &target_manifest(), &[]).unwrap_err(),
            "scenario 'wire.invalid.mode' mode 'sdk-adapter' must be one of suite-as-client, suite-as-server, suite-as-proxy"
        );

        let suite = json!({
            "protocol_version": "nnrp-1-preview4",
            "scenarios": [{
                "id": "wire.invalid.transport",
                "mode": "suite-as-client",
                "required_transport": "named-pipe"
            }]
        });
        assert_eq!(
            build_wire_execution_plan(&suite, &target_manifest(), &[]).unwrap_err(),
            "scenario 'wire.invalid.transport' required_transport 'named-pipe' must be one of any, tcp, quic, ipc, websocket"
        );
    }

    #[test]
    fn wire_execution_plan_rejects_unknown_target_endpoint_modes_and_transports() {
        let target = json!({
            "target_name": "bad-target-mode",
            "protocol_version": "nnrp-1-preview4",
            "endpoints": [{"transport": "ipc", "mode": "adapter", "uri": "nnrp+ipc://runtime.sock"}],
            "capabilities": ["control.cancel_abort"]
        });
        assert_eq!(
            build_wire_execution_plan(&suite_manifest(), &target, &[]).unwrap_err(),
            "target endpoint mode 'adapter' must be one of client, server"
        );

        let target = json!({
            "target_name": "bad-target-transport",
            "protocol_version": "nnrp-1-preview4",
            "endpoints": [{"transport": "stdio", "mode": "client", "uri": "stdio://runtime"}],
            "capabilities": ["control.cancel_abort"]
        });
        assert_eq!(
            build_wire_execution_plan(&suite_manifest(), &target, &[]).unwrap_err(),
            "target endpoint transport 'stdio' must be one of tcp, quic, ipc, websocket"
        );
    }

    #[test]
    fn wire_argument_parser_accepts_multiple_selected_cases() {
        let args = parse_wire_arguments([
            "--suite",
            "suite.json",
            "--target",
            "target.json",
            "--output",
            "results.json",
            "--case",
            "wire.cancel.ipc",
            "--case",
            "wire.progress.websocket",
        ])
        .expect("args");
        assert_eq!(args.suite_manifest, PathBuf::from("suite.json"));
        assert_eq!(args.target_manifest, PathBuf::from("target.json"));
        assert_eq!(args.output, PathBuf::from("results.json"));
        assert_eq!(
            args.selected_case_ids,
            vec!["wire.cancel.ipc", "wire.progress.websocket"]
        );
    }

    #[test]
    fn wire_argument_parser_reports_help_and_unknown_arguments() {
        let usage = parse_wire_arguments(["--help"]).expect_err("usage error");
        assert!(usage.starts_with("usage: nnrp-conformance-wire"));

        let unknown = parse_wire_arguments(["--bogus"]).expect_err("unknown argument");
        assert_eq!(unknown, "unknown wire conformance argument '--bogus'");
    }

    #[test]
    fn wire_writer_reads_manifests_and_writes_dry_run_report() {
        let root = temp_dir("nnrp-wire-dry-run");
        let suite_path = root.join("suite.json");
        let target_path = root.join("target.json");
        let output_path = root.join("nested").join("results.json");
        fs::create_dir_all(&root).expect("temp dir");
        fs::write(
            &suite_path,
            serde_json::to_vec(&suite_manifest()).expect("suite json"),
        )
        .expect("write suite");
        fs::write(
            &target_path,
            serde_json::to_vec(&target_manifest()).expect("target json"),
        )
        .expect("write target");

        write_wire_dry_run_report(
            &suite_path,
            &target_path,
            &output_path,
            &[String::from("wire.cancel.ipc")],
        )
        .expect("write report");

        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(&output_path).expect("read report"))
                .expect("report json");
        assert_eq!(report["mode"], "dry-run");
        assert_eq!(report["results"][0]["id"], "wire.cancel.ipc");

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn wire_writer_reports_output_directory_creation_failures() {
        let root = temp_dir("nnrp-wire-dry-run-create-error");
        let suite_path = root.join("suite.json");
        let target_path = root.join("target.json");
        let blocked_parent = root.join("blocked-parent");
        let output_path = blocked_parent.join("results.json");
        fs::create_dir_all(&root).expect("temp dir");
        fs::write(
            &suite_path,
            serde_json::to_vec(&suite_manifest()).expect("suite json"),
        )
        .expect("write suite");
        fs::write(
            &target_path,
            serde_json::to_vec(&target_manifest()).expect("target json"),
        )
        .expect("write target");
        fs::write(&blocked_parent, b"file").expect("write blocking file");

        let error = write_wire_dry_run_report(&suite_path, &target_path, &output_path, &[])
            .expect_err("create dir error");
        assert!(error.starts_with("failed to create wire result directory"));
        assert!(error.contains("blocked-parent"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn wire_writer_reports_output_write_failures() {
        let root = temp_dir("nnrp-wire-dry-run-write-error");
        let suite_path = root.join("suite.json");
        let target_path = root.join("target.json");
        let output_path = root.join("results-as-directory");
        fs::create_dir_all(&output_path).expect("output dir");
        fs::write(
            &suite_path,
            serde_json::to_vec(&suite_manifest()).expect("suite json"),
        )
        .expect("write suite");
        fs::write(
            &target_path,
            serde_json::to_vec(&target_manifest()).expect("target json"),
        )
        .expect("write target");

        let error = write_wire_dry_run_report(&suite_path, &target_path, &output_path, &[])
            .expect_err("write error");
        assert!(error.starts_with("failed to write wire dry-run report"));
        assert!(error.contains("results-as-directory"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "{}-{}-{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        path
    }
}
