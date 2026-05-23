use std::{env, fs, path::PathBuf};

use nnrp_conformance::adapter_conformance::build_results_report;
use serde_json::{json, Value};

#[test]
fn preview3_public_suite_cases_execute_through_adapter() {
    let suite_root = locate_suite_root().expect(
        "nnrp-conformance checkout is required; set NNRP_CONFORMANCE_SUITE_REPO or checkout it beside nnrp-rs",
    );
    let protocol_root = suite_root.join("protocol").join("nnrp-1-preview3");
    let manifest: Value = read_json(protocol_root.join("manifest.json"));
    let case_manifests = manifest["case_manifests"]
        .as_array()
        .expect("case_manifests should be an array");
    let mut cases = Vec::new();

    for relative_path in case_manifests {
        let relative_path = relative_path
            .as_str()
            .expect("case manifest path should be a string");
        let case_manifest = read_json(protocol_root.join(relative_path));
        for case in case_manifest["cases"]
            .as_array()
            .expect("case manifest should contain cases")
        {
            cases.push(json!({
                "id": case["id"].as_str().expect("case id should be a string"),
            }));
        }
    }

    let report = build_results_report(&json!({
        "protocol_version": "nnrp-1-preview3",
        "cases": cases
    }))
    .expect("adapter report should build for public suite cases");

    let results = report["results"].as_array().expect("results array");
    assert!(!results.is_empty());
    for result in results {
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
    .find(|path| path.join("protocol").join("nnrp-1-preview3").exists())
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_str(&fs::read_to_string(&path).expect("json file should be readable"))
        .expect("json file should parse")
}
