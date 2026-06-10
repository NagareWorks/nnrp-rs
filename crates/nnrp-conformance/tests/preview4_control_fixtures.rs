use nnrp_conformance::{
    execute_preview4_case, preview4_case_ids, preview4_fixture_manifest, PREVIEW4_PROTOCOL_VERSION,
};

#[test]
fn preview4_fixture_manifest_lists_executable_cases() {
    let manifest = preview4_fixture_manifest();
    let cases = manifest["cases"].as_array().expect("cases array");

    assert_eq!(manifest["protocol_version"], PREVIEW4_PROTOCOL_VERSION);
    assert_eq!(cases.len(), preview4_case_ids().len());
    for case_id in preview4_case_ids() {
        assert!(
            cases.iter().any(|case| case["id"] == *case_id),
            "manifest should contain {case_id}"
        );
        assert!(
            cases.iter().any(|case| {
                case["id"] == *case_id && case["suite_type"] == "control-frame-fixture"
            }),
            "manifest should classify {case_id} as a control-frame fixture"
        );
    }
}

#[test]
fn preview4_control_fixture_cases_execute() {
    for case_id in preview4_case_ids() {
        assert_eq!(execute_preview4_case(case_id), Some(Ok(())));
    }
}
