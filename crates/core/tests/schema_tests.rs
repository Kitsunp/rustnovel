use visual_novel_engine::{ScriptRaw, VnError, SCRIPT_SCHEMA_VERSION};

#[test]
fn script_json_allows_missing_schema_version_for_legacy_inputs() {
    let script_json = r#"{
        "events": [],
        "labels": {"start": 0}
    }"#;

    let parsed = ScriptRaw::from_json(script_json).expect("legacy schema should be accepted");
    assert!(parsed.events.is_empty());
    assert_eq!(parsed.labels.get("start"), Some(&0usize));
}

#[test]
fn script_json_rejects_incompatible_schema_version() {
    let script_json = r#"{
        "script_schema_version": "9.9",
        "events": [],
        "labels": {"start": 0}
    }"#;

    let err = ScriptRaw::from_json(script_json).expect_err("should reject bad schema");
    match err {
        VnError::InvalidScript(message) => {
            assert!(message.contains("schema incompatible"));
            assert!(message.contains(SCRIPT_SCHEMA_VERSION));
        }
        _ => panic!("expected schema error"),
    }
}

#[test]
fn script_json_accepts_legacy_major_schema_version() {
    let script_json = r#"{
        "script_schema_version": "0.9",
        "events": [],
        "labels": {"start": 0}
    }"#;

    let parsed = ScriptRaw::from_json(script_json).expect("legacy schema should be accepted");
    assert!(parsed.events.is_empty());
    assert_eq!(parsed.labels.get("start"), Some(&0usize));
}
