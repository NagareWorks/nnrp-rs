use std::{env, fs, process::Command};

use serde_json::{json, Value};

#[test]
fn adapter_cli_writes_results_report() {
    let temp_directory = env::temp_dir().join(format!(
        "nnrp-adapter-cli-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir_all(&temp_directory).expect("temp directory should be created");

    let plan_path = temp_directory.join("adapter-plan.json");
    let output_path = temp_directory.join("adapter-results.json");
    fs::write(
        &plan_path,
        json!({
            "protocol_version": "nnrp-1-preview3",
            "cases": [{ "id": "l1.handshake.basic" }]
        })
        .to_string(),
    )
    .expect("plan should be written");

    let status = Command::new(env!("CARGO_BIN_EXE_nnrp-conformance-adapter"))
        .arg("--plan")
        .arg(&plan_path)
        .arg("--output")
        .arg(&output_path)
        .status()
        .expect("adapter command should run");

    assert!(status.success());

    let output: Value =
        serde_json::from_str(&fs::read_to_string(&output_path).expect("output should exist"))
            .expect("output should be valid JSON");
    assert_eq!(
        output["results"][0]["id"],
        Value::String("l1.handshake.basic".to_string())
    );

    fs::remove_dir_all(&temp_directory).expect("temp directory should be removed");
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos()
}
