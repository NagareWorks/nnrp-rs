use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const WIRE_DRY_RUN_REPORT_SCHEMA: &str =
    "https://raw.githubusercontent.com/NagareWorks/nnrp-conformance/main/schemas/wire-dry-run-results.schema.json";

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
            "mode": case.get("mode").and_then(Value::as_str).unwrap_or("suite-as-client"),
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

#[cfg(test)]
mod tests {
    use super::{build_wire_dry_run_report, build_wire_execution_plan};
    use serde_json::json;

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
}
