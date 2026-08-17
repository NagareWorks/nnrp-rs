use std::{fs, path::Path};

use serde_json::{json, Value};

use crate::preview4_vectors::{
    execute_preview4_public_case_with_parameters, preview4_case_parameter_keys,
    PREVIEW4_PROTOCOL_VERSION,
};

pub const RESULTS_SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/NagareWorks/nnrp-conformance/main/schemas/adapter-case-results.schema.json";
pub const DEFAULT_IMPLEMENTATION_NAME: &str = "nnrp-rs";
pub const NOT_IMPLEMENTED_MESSAGE: &str =
    "Preview4 adapter execution is not implemented in nnrp-rs yet.";

pub struct AdapterOptions {
    pub plan_path: String,
    pub output_path: String,
}

pub fn write_results_report(plan_path: &Path, output_path: &Path) -> Result<(), String> {
    let raw_plan = fs::read_to_string(plan_path).map_err(|error| {
        format!(
            "failed to read adapter execution plan '{}': {error}",
            plan_path.display()
        )
    })?;
    let plan: Value = serde_json::from_str(&raw_plan)
        .map_err(|error| format!("adapter execution plan must be valid JSON: {error}"))?;
    let report = build_results_report(&plan)?;

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create adapter result directory '{}': {error}",
                    parent.display()
                )
            })?;
        }
    }

    let rendered = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize adapter case results report: {error}"))?;
    fs::write(output_path, format!("{rendered}\n")).map_err(|error| {
        format!(
            "failed to write adapter case results report '{}': {error}",
            output_path.display()
        )
    })?;
    Ok(())
}

pub fn build_results_report(plan: &Value) -> Result<Value, String> {
    let plan_object = plan
        .as_object()
        .ok_or_else(|| "adapter execution plan must be a JSON object".to_string())?;
    let protocol_version = plan_object
        .get("protocol_version")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "adapter execution plan field 'protocol_version' must be a non-empty string".to_string()
        })?;
    if protocol_version != PREVIEW4_PROTOCOL_VERSION {
        return Err(format!(
            "adapter execution plan protocol_version must be '{PREVIEW4_PROTOCOL_VERSION}', got '{protocol_version}'"
        ));
    }
    let cases = plan_object
        .get("cases")
        .and_then(Value::as_array)
        .ok_or_else(|| "adapter execution plan must contain a cases array".to_string())?;

    let mut results = Vec::with_capacity(cases.len());
    for case in cases {
        let case_object = case
            .as_object()
            .ok_or_else(|| "adapter execution plan cases must be JSON objects".to_string())?;
        let case_id = case_object
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "adapter execution plan case field 'id' must be a non-empty string".to_string()
            })?;

        let parameters = case_object.get("parameters").and_then(Value::as_object);
        if let Err(message) = validate_case_parameters(case_id, parameters) {
            results.push(json!({
                "id": case_id,
                "outcome": "fail",
                "failure_kind": "invalid_frozen_parameters",
                "message": message,
            }));
            continue;
        }

        results.push(
            match execute_preview4_public_case_with_parameters(case_id, parameters) {
                Some(Ok(())) => json!({
                    "id": case_id,
                    "outcome": "pass",
                }),
                Some(Err(message)) => json!({
                    "id": case_id,
                    "outcome": "fail",
                    "failure_kind": "assertion_failed",
                    "message": message,
                }),
                None => json!({
                    "id": case_id,
                    "outcome": "error",
                    "failure_kind": "not_implemented",
                    "message": NOT_IMPLEMENTED_MESSAGE,
                }),
            },
        );
    }

    Ok(json!({
        "$schema": RESULTS_SCHEMA_URL,
        "protocol_version": protocol_version,
        "implementation_name": DEFAULT_IMPLEMENTATION_NAME,
        "results": results,
    }))
}

fn validate_case_parameters(
    case_id: &str,
    parameters: Option<&serde_json::Map<String, Value>>,
) -> Result<(), String> {
    let expected = preview4_case_parameter_keys(case_id);
    if expected.is_empty() {
        if parameters.is_some_and(|actual| !actual.is_empty()) {
            return Err(format!("{case_id} does not accept parameters"));
        }
        return Ok(());
    }

    let actual = parameters.ok_or_else(|| format!("{case_id} requires its parameters object"))?;
    if actual.len() != expected.len() {
        return Err(format!(
            "{case_id} requires exactly {} parameter(s)",
            expected.len()
        ));
    }
    for name in expected {
        if actual.get(*name).and_then(Value::as_str).is_none() {
            return Err(format!("{case_id} requires the string {name} parameter"));
        }
    }
    Ok(())
}

pub fn parse_arguments(
    args: impl IntoIterator<Item = String>,
    env_plan_path: Option<String>,
    env_output_path: Option<String>,
) -> Result<AdapterOptions, String> {
    let mut plan_path = env_plan_path;
    let mut output_path = env_output_path;
    let arguments: Vec<String> = args.into_iter().collect();
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--plan" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| "missing value for --plan".to_string())?;
                plan_path = Some(value.clone());
            }
            "--output" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| "missing value for --output".to_string())?;
                output_path = Some(value.clone());
            }
            unknown => return Err(format!("unknown argument: {unknown}")),
        }
        index += 1;
    }

    let plan_path = plan_path.filter(|value| !value.is_empty()).ok_or_else(|| {
        "adapter execution plan path is required via --plan or NNRP_CONFORMANCE_ADAPTER_PLAN"
            .to_string()
    })?;
    let output_path = output_path
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "adapter result path is required via --output or NNRP_CONFORMANCE_ADAPTER_RESULTS"
                .to_string()
        })?;

    Ok(AdapterOptions {
        plan_path,
        output_path,
    })
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::{build_results_report, parse_arguments, write_results_report, RESULTS_SCHEMA_URL};
    use serde_json::{json, Value};

    #[test]
    fn build_results_report_marks_cases_as_not_implemented() {
        let report = build_results_report(&json!({
            "protocol_version": "nnrp-1-preview4",
            "cases": [
                { "id": "l9.unknown.public.case" },
                { "id": "l9.unknown.public.case.two" }
            ]
        }))
        .expect("report should build");

        assert_eq!(
            report["implementation_name"],
            Value::String("nnrp-rs".to_string())
        );
        assert_eq!(
            report["results"].as_array().expect("results array").len(),
            2
        );
        assert_eq!(
            report["results"][0]["id"],
            Value::String("l9.unknown.public.case".to_string())
        );
        assert_eq!(
            report["results"][0]["failure_kind"],
            Value::String("not_implemented".to_string())
        );
    }

    #[test]
    fn build_results_report_passes_preview4_public_suite_cases() {
        let cases: Vec<Value> = crate::preview4_public_case_ids()
            .iter()
            .map(|id| public_case(id))
            .collect();

        let report = build_results_report(&json!({
            "protocol_version": "nnrp-1-preview4",
            "cases": cases
        }))
        .expect("preview4 public report should build");

        let results = report["results"].as_array().expect("results array");
        assert_eq!(results.len(), crate::preview4_public_case_ids().len());
        for result in results {
            assert_eq!(result["outcome"], Value::String("pass".to_string()));
            assert!(result.get("failure_kind").is_none());
        }
    }

    fn public_case(id: &str) -> Value {
        let parameters = match id {
            "l0.control.client_hello.golden" => {
                json!({ "metadata_hex": "01010100010000000100000003000000030000002100000003000000010007000100020040000000000001007017640002000000000000006000000000000000" })
            }
            "l0.control.session_patch_ack.golden" => {
                json!({ "metadata_hex": "010003001100000044000000000000000200000028230000680105000300000000000000010000000300000010000000" })
            }
            "l0.flow_update.packet.golden" => {
                json!({ "packet_hex": "4e4e5250010017280000000020000000000000001500000000000000000006000d000000000000000104020000000100000000000000000000000000280000000500000003000000" })
            }
            "l0.result_hint.packet.golden" => {
                json!({ "packet_hex": "4e4e525001001828000000001000000000000000150000002f010000000007000e000000000000000300000003000000030000003c000000" })
            }
            "l0.frame_submit.metadata.golden" => {
                json!({ "metadata_hex": "80026801200020005400020001020000640070170700000000000000c000000000000000000000000807060504030201000000000205ff0003000000290000001100000002000000" })
            }
            "l0.result_push.metadata.golden" => {
                json!({ "metadata_hex": "0000050001005400020000004b0302004e030000000000001000000000000000000000000000000000000000010100002900000035001f000300000003000000" })
            }
            "l0.body_region.prelude.golden" => {
                json!({ "metadata_hex": "1800000018000000180000000e00000010000000050000000000000000000000" })
            }
            "l0.object_reference.block.golden" => {
                json!({ "metadata_hex": "020000000700000044332211000000008877665500000000" })
            }
            "l0.typed_payload.descriptor.golden" => {
                json!({ "descriptor_hex": "10000300040000000700000000000000" })
            }
            "l0.header.fixed_shape.golden" => {
                json!({ "header_hex": "4e4e525001001028210000003000000000100000070000000b0000000200000015cd5b0700000000" })
            }
            "l0.typed_payload.frame_regions.golden" => json!({
                "descriptor_region_hex": "020001000000000003000000000000000400020003000000020000000000000008000300050000000500000000000000100004000a0000000300000000000000",
                "payload_hex": "746f6b6175766964656f657674"
            }),
            "l0.typed_payload.descriptor.current.golden" => {
                json!({ "descriptor_hex": "020002020110000003000000020000000800000018000000" })
            }
            _ => json!({}),
        };
        json!({ "id": id, "parameters": parameters })
    }

    #[test]
    fn build_results_report_rejects_missing_extra_or_unusable_case_parameters() {
        let missing = build_results_report(&json!({
            "protocol_version": "nnrp-1-preview4",
            "cases": [{ "id": "l0.frame_submit.metadata.golden" }]
        }))
        .expect("report should build");
        assert_eq!(missing["results"][0]["outcome"], "fail");
        assert_eq!(
            missing["results"][0]["failure_kind"],
            "invalid_frozen_parameters"
        );

        let changed = build_results_report(&json!({
            "protocol_version": "nnrp-1-preview4",
            "cases": [{
                "id": "l0.result_push.metadata.golden",
                "parameters": { "metadata_hex": "00" }
            }]
        }))
        .expect("report should build");
        assert_eq!(changed["results"][0]["outcome"], "fail");
        assert_eq!(changed["results"][0]["failure_kind"], "assertion_failed");

        let invalid_hex = build_results_report(&json!({
            "protocol_version": "nnrp-1-preview4",
            "cases": [{
                "id": "l0.header.fixed_shape.golden",
                "parameters": { "header_hex": "not-hex" }
            }]
        }))
        .expect("report should build");
        assert_eq!(invalid_hex["results"][0]["outcome"], "fail");
        assert_eq!(
            invalid_hex["results"][0]["failure_kind"],
            "assertion_failed"
        );

        let extra = build_results_report(&json!({
            "protocol_version": "nnrp-1-preview4",
            "cases": [{
                "id": "l0.typed_payload.frame_regions.golden",
                "parameters": {
                    "descriptor_region_hex": "020001000000000003000000000000000400020003000000020000000000000008000300050000000500000000000000100004000a0000000300000000000000",
                    "payload_hex": "746f6b6175766964656f657674",
                    "hidden_default": true
                }
            }]
        }))
        .expect("report should build");
        assert_eq!(extra["results"][0]["outcome"], "fail");

        let unexpected = build_results_report(&json!({
            "protocol_version": "nnrp-1-preview4",
            "cases": [{
                "id": "l1.control.cancel-abort",
                "parameters": { "hidden_default": true }
            }]
        }))
        .expect("report should build");
        assert_eq!(unexpected["results"][0]["outcome"], "fail");
        assert_eq!(
            unexpected["results"][0]["failure_kind"],
            "invalid_frozen_parameters"
        );
    }

    #[test]
    fn parse_arguments_prefers_cli_values_and_accepts_environment_defaults() {
        let options = parse_arguments(
            vec![
                "--plan".to_string(),
                "cli-plan.json".to_string(),
                "--output".to_string(),
                "cli-results.json".to_string(),
            ],
            Some("env-plan.json".to_string()),
            Some("env-results.json".to_string()),
        )
        .expect("arguments should parse");

        assert_eq!(options.plan_path, "cli-plan.json");
        assert_eq!(options.output_path, "cli-results.json");
    }

    #[test]
    fn build_results_report_rejects_invalid_plan_shapes() {
        assert_eq!(
            build_results_report(&json!(null)),
            Err("adapter execution plan must be a JSON object".to_string())
        );
        assert_eq!(
            build_results_report(&json!({ "cases": [] })),
            Err(
                "adapter execution plan field 'protocol_version' must be a non-empty string"
                    .to_string()
            )
        );
        assert_eq!(
            build_results_report(&json!({ "protocol_version": "nnrp-1-preview4" })),
            Err("adapter execution plan must contain a cases array".to_string())
        );
        assert_eq!(
            build_results_report(&json!({
                "protocol_version": "nnrp-1-preview4",
                "cases": [null]
            })),
            Err("adapter execution plan cases must be JSON objects".to_string())
        );
        assert_eq!(
            build_results_report(&json!({
                "protocol_version": "nnrp-1-preview4",
                "cases": [{}]
            })),
            Err("adapter execution plan case field 'id' must be a non-empty string".to_string())
        );
        assert_eq!(
            build_results_report(&json!({
                "protocol_version": "nnrp-1-preview3",
                "cases": []
            })),
            Err("adapter execution plan protocol_version must be 'nnrp-1-preview4', got 'nnrp-1-preview3'".to_string())
        );
    }

    #[test]
    fn build_results_report_sets_schema_and_protocol_version() {
        let report = build_results_report(&json!({
            "protocol_version": "nnrp-1-preview4",
            "cases": []
        }))
        .expect("report should build");

        assert_eq!(
            report["$schema"],
            Value::String(RESULTS_SCHEMA_URL.to_string())
        );
        assert_eq!(
            report["protocol_version"],
            Value::String("nnrp-1-preview4".to_string())
        );
        assert_eq!(
            report["results"].as_array().expect("results array").len(),
            0
        );
    }

    #[test]
    fn parse_arguments_uses_environment_defaults_when_cli_is_absent() {
        let options = parse_arguments(
            Vec::<String>::new(),
            Some("env-plan.json".to_string()),
            Some("env-results.json".to_string()),
        )
        .expect("environment defaults should parse");

        assert_eq!(options.plan_path, "env-plan.json");
        assert_eq!(options.output_path, "env-results.json");
    }

    #[test]
    fn parse_arguments_rejects_missing_required_values_and_unknown_flags() {
        assert_eq!(
            parse_error(Vec::<String>::new(), None, Some("results.json".to_string())),
            "adapter execution plan path is required via --plan or NNRP_CONFORMANCE_ADAPTER_PLAN"
        );
        assert_eq!(
            parse_error(Vec::<String>::new(), Some("plan.json".to_string()), None),
            "adapter result path is required via --output or NNRP_CONFORMANCE_ADAPTER_RESULTS"
        );
        assert_eq!(
            parse_error(vec!["--plan".to_string()], None, None),
            "missing value for --plan"
        );
        assert_eq!(
            parse_error(vec!["--output".to_string()], None, None),
            "missing value for --output"
        );
        assert_eq!(
            parse_error(vec!["--bogus".to_string()], None, None),
            "unknown argument: --bogus"
        );
    }

    #[test]
    fn write_results_report_rejects_invalid_input_files() {
        let temp_directory = env::temp_dir().join(format!(
            "nnrp-adapter-invalid-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&temp_directory).expect("temp directory should be created");

        let missing_plan_path = temp_directory.join("missing-plan.json");
        let output_path = temp_directory.join("adapter-results.json");
        let error = write_results_report(&missing_plan_path, &output_path)
            .expect_err("missing plan should fail");
        assert!(error.starts_with("failed to read adapter execution plan"));

        let invalid_plan_path = temp_directory.join("invalid-plan.json");
        fs::write(&invalid_plan_path, "{").expect("invalid plan should be written");
        assert_eq!(
            write_results_report(&invalid_plan_path, &output_path),
            Err("adapter execution plan must be valid JSON: EOF while parsing an object at line 1 column 1".to_string())
        );

        fs::remove_dir_all(&temp_directory).expect("temp directory should be removed");
    }

    #[test]
    fn write_results_report_reads_plan_and_writes_output_file() {
        let temp_directory = env::temp_dir().join(format!(
            "nnrp-adapter-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&temp_directory).expect("temp directory should be created");

        let plan_path = temp_directory.join("adapter-plan.json");
        let output_path = temp_directory
            .join("artifacts")
            .join("adapter-results.json");
        fs::write(
            &plan_path,
            json!({
                "protocol_version": "nnrp-1-preview4",
                "cases": [{ "id": "l9.unknown.public.case" }]
            })
            .to_string(),
        )
        .expect("plan should be written");

        write_results_report(&plan_path, &output_path).expect("report should be written");

        let output: Value =
            serde_json::from_str(&fs::read_to_string(&output_path).expect("output should exist"))
                .expect("output should be valid JSON");
        assert_eq!(
            output["results"][0]["message"],
            Value::String(
                "Preview4 adapter execution is not implemented in nnrp-rs yet.".to_string()
            )
        );

        fs::remove_dir_all(&temp_directory).expect("temp directory should be removed");
    }

    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos()
    }

    fn parse_error(
        args: impl IntoIterator<Item = String>,
        env_plan_path: Option<String>,
        env_output_path: Option<String>,
    ) -> String {
        match parse_arguments(args, env_plan_path, env_output_path) {
            Ok(_) => panic!("arguments should fail"),
            Err(error) => error,
        }
    }
}
