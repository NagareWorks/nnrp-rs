use std::{collections::BTreeSet, env, fs, path::PathBuf};

use nnrp_conformance::{
    adapter_conformance::build_results_report, preview4_capability_tokens,
    preview4_public_case_ids, PREVIEW4_PROTOCOL_VERSION,
};
use serde_json::{json, Value};

#[test]
fn preview4_public_suite_manifest_capabilities_and_adapter_stay_equal() {
    let suite_root = locate_suite_root().expect(
        "nnrp-conformance checkout is required; set NNRP_CONFORMANCE_SUITE_REPO or checkout it beside nnrp-rs",
    );
    let protocol_root = suite_root.join("protocol").join(PREVIEW4_PROTOCOL_VERSION);
    let manifest: Value = read_json(protocol_root.join("manifest.json"));
    let mut suite_case_ids = Vec::new();
    let mut required_capabilities = BTreeSet::new();

    for relative_path in manifest["case_manifests"]
        .as_array()
        .expect("case_manifests should be an array")
    {
        let case_manifest = read_json(
            protocol_root.join(
                relative_path
                    .as_str()
                    .expect("case manifest path should be a string"),
            ),
        );
        for case in case_manifest["cases"]
            .as_array()
            .expect("case manifest should contain cases")
        {
            suite_case_ids.push(
                case["id"]
                    .as_str()
                    .expect("case id should be a string")
                    .to_string(),
            );
            for capability in case["required_capabilities"]
                .as_array()
                .expect("required_capabilities should be an array")
            {
                required_capabilities.insert(
                    capability
                        .as_str()
                        .expect("capability should be a string")
                        .to_string(),
                );
            }
        }
    }

    let adapter_case_ids: Vec<String> = preview4_public_case_ids()
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    assert_eq!(adapter_case_ids, suite_case_ids);

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .ancestors()
        .nth(2)
        .expect("crate should live under crates/nnrp-conformance");
    let capabilities = read_json(
        repo_root
            .join("conformance")
            .join("nnrp-1-preview4.capabilities.json"),
    );
    assert_eq!(capabilities["protocol_version"], PREVIEW4_PROTOCOL_VERSION);
    let declared_capabilities: Vec<String> = capabilities["supports"]
        .as_array()
        .expect("supports should be an array")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("capability should be a string")
                .to_string()
        })
        .collect();
    let code_capabilities: Vec<String> = preview4_capability_tokens()
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    assert_eq!(declared_capabilities, code_capabilities);
    assert!(required_capabilities.is_subset(&declared_capabilities.into_iter().collect()));

    let cases: Vec<Value> = suite_case_ids
        .iter()
        .map(|id| json!({ "id": id }))
        .collect();
    let report = build_results_report(&json!({
        "protocol_version": PREVIEW4_PROTOCOL_VERSION,
        "cases": cases
    }))
    .expect("adapter report should build for public suite cases");
    for result in report["results"].as_array().expect("results array") {
        assert_eq!(result["outcome"], "pass", "case did not pass: {result}");
    }
}

fn locate_suite_root() -> Option<PathBuf> {
    if let Some(path) = env::var_os("NNRP_CONFORMANCE_SUITE_REPO") {
        return Some(PathBuf::from(path));
    }

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate should live under crates/nnrp-conformance")
        .to_path_buf();
    [
        repo_root.join("nnrp-conformance-action"),
        repo_root
            .parent()
            .expect("repo root should have parent")
            .join("nnrp-conformance"),
    ]
    .into_iter()
    .find(|path| {
        path.join("protocol")
            .join(PREVIEW4_PROTOCOL_VERSION)
            .exists()
    })
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_str(&fs::read_to_string(&path).expect("json file should be readable"))
        .expect("json file should parse")
}
