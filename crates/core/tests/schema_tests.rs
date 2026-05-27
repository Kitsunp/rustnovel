use visual_novel_engine::{runtime::ScriptRaw, VnError, SCRIPT_SCHEMA_VERSION};

#[test]
fn script_json_rejects_missing_schema_version_for_legacy_inputs() {
    let script_json = r#"{
        "events": [],
        "labels": {"start": 0}
    }"#;

    let err = ScriptRaw::from_json(script_json).expect_err("missing schema must be rejected");
    assert!(
        err.to_string()
            .contains("missing field `script_schema_version`"),
        "unexpected error: {err}"
    );
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
fn script_json_rejects_legacy_major_schema_version() {
    let script_json = r#"{
        "script_schema_version": "0.9",
        "events": [],
        "labels": {"start": 0}
    }"#;

    let err = ScriptRaw::from_json(script_json).expect_err("legacy schema must be rejected");
    match err {
        VnError::InvalidScript(message) => {
            assert!(message.contains("schema incompatible"));
            assert!(message.contains(SCRIPT_SCHEMA_VERSION));
        }
        _ => panic!("expected schema error"),
    }
}
