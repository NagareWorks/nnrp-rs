use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const WIRE_DRY_RUN_REPORT_SCHEMA: &str =
    "https://raw.githubusercontent.com/NagareWorks/nnrp-conformance/main/schemas/wire-dry-run-results.schema.json";
const WIRE_TRANSPORTS: &[&str] = &["tcp", "quic", "ipc", "websocket"];
const WIRE_SCENARIO_MODES: &[&str] = &["suite_as_client", "suite_as_server", "suite_as_proxy"];

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
    let suite_manifest = read_expanded_suite_manifest(suite_manifest_path)?;
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

    let target_modes = target_modes(target_manifest)?;
    let target_transports = target_transports(target_manifest)?;
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

        let transport = case
            .get("transport")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!("scenario '{id}' field 'transport' must be a non-empty string")
            })?;
        validate_wire_transport(transport, &format!("scenario '{id}' transport"))?;
        let mode = case
            .get("mode")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("scenario '{id}' field 'mode' must be a non-empty string"))?;
        validate_wire_scenario_mode(mode, &format!("scenario '{id}' mode"))?;
        let required_capabilities = string_array_field(case.get("required_capabilities"))
            .map_err(|error| format!("scenario '{id}' {error}"))?;
        let missing_capabilities: Vec<&str> = required_capabilities
            .iter()
            .copied()
            .filter(|capability| !capabilities.contains(*capability))
            .collect();

        let unsupported_mode = !target_modes.contains(mode);
        let unsupported_transport = !target_transports.contains(transport);
        let runnable =
            !unsupported_mode && !unsupported_transport && missing_capabilities.is_empty();
        let skip_reason = if unsupported_mode {
            Some(format!("target does not claim required mode '{mode}'"))
        } else if unsupported_transport {
            Some(format!(
                "target does not claim required transport '{transport}'"
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
            "transport": transport,
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

fn target_modes(target_manifest: &Value) -> Result<BTreeSet<String>, String> {
    let wire_conformance = required_object(target_manifest, "wire_conformance")?;
    let modes = string_array_field(wire_conformance.get("modes"))?;
    let mut declared_modes = BTreeSet::new();
    for mode in modes {
        validate_wire_scenario_mode(mode, "target wire_conformance mode")?;
        declared_modes.insert(mode.to_string());
    }
    Ok(declared_modes)
}

fn target_transports(target_manifest: &Value) -> Result<BTreeSet<String>, String> {
    let wire_conformance = required_object(target_manifest, "wire_conformance")?;
    let transport_entries = required_array_value(wire_conformance, "transports")?;
    let mut transports = BTreeSet::new();
    for entry in transport_entries {
        let entry = entry
            .as_object()
            .ok_or_else(|| "target wire_conformance transports must be JSON objects".to_string())?;
        let transport = entry.get("name").and_then(Value::as_str).ok_or_else(|| {
            "target wire_conformance transport field 'name' must be a non-empty string".to_string()
        })?;
        validate_wire_transport(transport, "target wire_conformance transport")?;
        transports.insert(transport.to_string());
    }
    Ok(transports)
}

fn target_capabilities(target_manifest: &Value) -> Result<BTreeSet<String>, String> {
    let wire_conformance = required_object(target_manifest, "wire_conformance")?;
    let capabilities = string_array_field(wire_conformance.get("capabilities"))?;
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

fn validate_wire_transport(value: &str, label: &str) -> Result<(), String> {
    if WIRE_TRANSPORTS.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "{label} '{value}' must be one of {}",
            "tcp, quic, ipc, websocket"
        ))
    }
}

fn validate_wire_scenario_mode(value: &str, label: &str) -> Result<(), String> {
    if WIRE_SCENARIO_MODES.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "{label} '{value}' must be one of suite_as_client, suite_as_server, suite_as_proxy"
        ))
    }
}

fn read_json_file(path: &Path, label: &str) -> Result<Value, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {label} '{}': {error}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("{label} '{}' must be valid JSON: {error}", path.display()))
}

fn read_expanded_suite_manifest(path: &Path) -> Result<Value, String> {
    let mut suite_manifest = read_json_file(path, "wire suite manifest")?;
    if suite_manifest.get("scenarios").is_some() {
        return Ok(suite_manifest);
    }

    let protocol_version = required_string(&suite_manifest, "protocol_version")?.to_string();
    let manifest_entries = required_array(&suite_manifest, "scenario_manifests")?;
    let suite_root = path.parent().unwrap_or_else(|| Path::new("."));
    let mut scenarios = Vec::new();
    for entry in manifest_entries {
        let relative_path = entry
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "wire suite scenario_manifests entries must be non-empty strings".to_string()
            })?;
        let scenario_path = suite_root.join(relative_path);
        let scenario_manifest = read_json_file(&scenario_path, "wire scenario manifest")?;
        let scenario_protocol = required_string(&scenario_manifest, "protocol_version")?;
        if scenario_protocol != protocol_version {
            return Err(format!(
                "wire scenario manifest '{}' protocol_version '{scenario_protocol}' does not match suite protocol_version '{protocol_version}'",
                scenario_path.display()
            ));
        }
        scenarios.extend(
            required_array(&scenario_manifest, "scenarios")?
                .iter()
                .cloned(),
        );
    }
    suite_manifest["scenarios"] = Value::Array(scenarios);
    Ok(suite_manifest)
}

fn required_object<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a serde_json::Map<String, Value>, String> {
    value
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("field '{field}' must be an object"))
}

fn required_array_value<'a>(
    value: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a Vec<Value>, String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("field '{field}' must be an array"))
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
        read_expanded_suite_manifest, write_wire_dry_run_report,
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
                    "mode": "suite_as_client",
                    "transport": "ipc",
                    "required_capabilities": ["control.cancel_abort"]
                },
                {
                    "id": "wire.progress.websocket",
                    "mode": "suite_as_server",
                    "transport": "websocket",
                    "required_capabilities": ["control.progress_partial", "control.credit_backpressure"]
                }
            ]
        })
    }

    fn target_manifest() -> serde_json::Value {
        json!({
            "target_name": "reference-rs",
            "protocol_version": "nnrp-1-preview4",
            "wire_conformance": {
                "modes": ["suite_as_client", "suite_as_server", "suite_as_proxy"],
                "transports": [
                    {"name": "ipc", "endpoint": "npipe://reference-rs", "tls": false},
                    {"name": "websocket", "endpoint": "ws://127.0.0.1:0/nnrp", "tls": false}
                ],
                "capabilities": [
                    "control.cancel_abort",
                    "control.progress_partial",
                    "control.credit_backpressure"
                ],
                "limits": {
                    "max_frame_bytes": 16777216,
                    "max_in_flight": 256
                }
            }
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
            "wire_conformance": {
                "modes": ["suite_as_client", "suite_as_server"],
                "transports": [{"name": "ipc", "endpoint": "npipe://partial", "tls": false}],
                "capabilities": ["control.cancel_abort"],
                "limits": {
                    "max_frame_bytes": 16777216,
                    "max_in_flight": 256
                }
            }
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
    fn wire_execution_plan_skips_missing_target_modes() {
        let target = json!({
            "target_name": "client-only",
            "protocol_version": "nnrp-1-preview4",
            "wire_conformance": {
                "modes": ["suite_as_client"],
                "transports": [
                    {"name": "ipc", "endpoint": "npipe://client-only", "tls": false},
                    {"name": "websocket", "endpoint": "ws://127.0.0.1:0/nnrp", "tls": false}
                ],
                "capabilities": [
                    "control.cancel_abort",
                    "control.progress_partial",
                    "control.credit_backpressure"
                ],
                "limits": {
                    "max_frame_bytes": 16777216,
                    "max_in_flight": 256
                }
            }
        });
        let plan = build_wire_execution_plan(&suite_manifest(), &target, &[]).expect("plan");
        let cases = plan["cases"].as_array().expect("cases");
        assert_eq!(cases[0]["status"], "ready");
        assert_eq!(cases[1]["status"], "skipped");
        assert!(cases[1]["skip_reason"]
            .as_str()
            .expect("skip reason")
            .contains("required mode 'suite_as_server'"));
    }

    #[test]
    fn wire_dry_run_report_keeps_ready_and_skipped_outcomes_explicit() {
        let target = json!({
            "target_name": "partial",
            "protocol_version": "nnrp-1-preview4",
            "wire_conformance": {
                "modes": ["suite_as_client", "suite_as_server"],
                "transports": [{"name": "ipc", "endpoint": "npipe://partial", "tls": false}],
                "capabilities": ["control.cancel_abort"],
                "limits": {
                    "max_frame_bytes": 16777216,
                    "max_in_flight": 256
                }
            }
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
                "transport": "ipc"
            }]
        });
        assert_eq!(
            build_wire_execution_plan(&suite, &target_manifest(), &[]).unwrap_err(),
            "scenario 'wire.invalid.mode' mode 'sdk-adapter' must be one of suite_as_client, suite_as_server, suite_as_proxy"
        );

        let suite = json!({
            "protocol_version": "nnrp-1-preview4",
            "scenarios": [{
                "id": "wire.invalid.transport",
                "mode": "suite_as_client",
                "transport": "named-pipe"
            }]
        });
        assert_eq!(
            build_wire_execution_plan(&suite, &target_manifest(), &[]).unwrap_err(),
            "scenario 'wire.invalid.transport' transport 'named-pipe' must be one of tcp, quic, ipc, websocket"
        );
    }

    #[test]
    fn wire_execution_plan_rejects_unknown_target_endpoint_modes_and_transports() {
        let target = json!({
            "target_name": "bad-target-mode",
            "protocol_version": "nnrp-1-preview4",
            "wire_conformance": {
                "modes": ["adapter"],
                "transports": [{"name": "ipc", "endpoint": "npipe://bad-target-mode", "tls": false}],
                "capabilities": ["control.cancel_abort"],
                "limits": {
                    "max_frame_bytes": 16777216,
                    "max_in_flight": 256
                }
            }
        });
        assert_eq!(
            build_wire_execution_plan(&suite_manifest(), &target, &[]).unwrap_err(),
            "target wire_conformance mode 'adapter' must be one of suite_as_client, suite_as_server, suite_as_proxy"
        );

        let target = json!({
            "target_name": "bad-target-transport",
            "protocol_version": "nnrp-1-preview4",
            "wire_conformance": {
                "modes": ["suite_as_client"],
                "transports": [{"name": "stdio", "endpoint": "stdio://runtime", "tls": false}],
                "capabilities": ["control.cancel_abort"],
                "limits": {
                    "max_frame_bytes": 16777216,
                    "max_in_flight": 256
                }
            }
        });
        assert_eq!(
            build_wire_execution_plan(&suite_manifest(), &target, &[]).unwrap_err(),
            "target wire_conformance transport 'stdio' must be one of tcp, quic, ipc, websocket"
        );
    }

    #[test]
    fn wire_execution_plan_rejects_missing_scenario_fields_and_malformed_target_transports() {
        let suite = json!({
            "protocol_version": "nnrp-1-preview4",
            "scenarios": [{
                "id": "wire.missing.transport",
                "mode": "suite_as_client"
            }]
        });
        assert_eq!(
            build_wire_execution_plan(&suite, &target_manifest(), &[]).unwrap_err(),
            "scenario 'wire.missing.transport' field 'transport' must be a non-empty string"
        );

        let target = json!({
            "target_name": "bad-transport-entry",
            "protocol_version": "nnrp-1-preview4",
            "wire_conformance": {
                "modes": ["suite_as_client"],
                "transports": ["ipc"],
                "capabilities": ["control.cancel_abort"],
                "limits": {
                    "max_frame_bytes": 16777216,
                    "max_in_flight": 256
                }
            }
        });
        assert_eq!(
            build_wire_execution_plan(&suite_manifest(), &target, &[]).unwrap_err(),
            "target wire_conformance transports must be JSON objects"
        );

        let target = json!({
            "target_name": "bad-transport-name",
            "protocol_version": "nnrp-1-preview4",
            "wire_conformance": {
                "modes": ["suite_as_client"],
                "transports": [{"endpoint": "npipe://bad-transport-name", "tls": false}],
                "capabilities": ["control.cancel_abort"],
                "limits": {
                    "max_frame_bytes": 16777216,
                    "max_in_flight": 256
                }
            }
        });
        assert_eq!(
            build_wire_execution_plan(&suite_manifest(), &target, &[]).unwrap_err(),
            "target wire_conformance transport field 'name' must be a non-empty string"
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
    fn wire_writer_expands_formal_suite_scenario_manifests() {
        let root = temp_dir("nnrp-wire-formal-suite");
        let cases_dir = root.join("cases");
        let suite_path = root.join("manifest.json");
        let scenario_path = cases_dir.join("runtime-control-e2e.json");
        let target_path = root.join("target.json");
        let output_path = root.join("results.json");
        fs::create_dir_all(&cases_dir).expect("cases dir");
        fs::write(
            &suite_path,
            serde_json::to_vec(&json!({
                "protocol_version": "nnrp-1-preview4",
                "scenario_manifests": ["cases/runtime-control-e2e.json"]
            }))
            .expect("suite json"),
        )
        .expect("write suite");
        fs::write(
            &scenario_path,
            serde_json::to_vec(&suite_manifest()).expect("scenario json"),
        )
        .expect("write scenario");
        fs::write(
            &target_path,
            serde_json::to_vec(&target_manifest()).expect("target json"),
        )
        .expect("write target");

        let expanded = read_expanded_suite_manifest(&suite_path).expect("expanded suite");
        assert_eq!(
            expanded["scenarios"].as_array().expect("scenarios").len(),
            2
        );

        write_wire_dry_run_report(&suite_path, &target_path, &output_path, &[])
            .expect("write report");
        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(&output_path).expect("read report"))
                .expect("report json");
        assert_eq!(report["results"].as_array().expect("results").len(), 2);
        assert_eq!(report["results"][0]["id"], "wire.cancel.ipc");

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn wire_suite_expansion_rejects_bad_manifest_entries_and_protocol_mismatch() {
        let root = temp_dir("nnrp-wire-formal-suite-errors");
        let cases_dir = root.join("cases");
        let suite_path = root.join("manifest.json");
        let scenario_path = cases_dir.join("bad.json");
        fs::create_dir_all(&cases_dir).expect("cases dir");
        fs::write(
            &suite_path,
            serde_json::to_vec(&json!({
                "protocol_version": "nnrp-1-preview4",
                "scenario_manifests": [""]
            }))
            .expect("suite json"),
        )
        .expect("write suite");
        assert_eq!(
            read_expanded_suite_manifest(&suite_path).unwrap_err(),
            "wire suite scenario_manifests entries must be non-empty strings"
        );

        fs::write(
            &suite_path,
            serde_json::to_vec(&json!({
                "protocol_version": "nnrp-1-preview4",
                "scenario_manifests": ["cases/bad.json"]
            }))
            .expect("suite json"),
        )
        .expect("write suite");
        fs::write(
            &scenario_path,
            serde_json::to_vec(&json!({
                "protocol_version": "nnrp-1-preview3",
                "scenarios": []
            }))
            .expect("scenario json"),
        )
        .expect("write scenario");
        let error = read_expanded_suite_manifest(&suite_path).unwrap_err();
        assert!(error.contains("protocol_version 'nnrp-1-preview3' does not match"));

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
