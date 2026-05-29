use visual_novel_engine::{runtime::ScriptRaw, SchemaPolicy, VnError, SCRIPT_SCHEMA_VERSION};

fn fixture(name: &str) -> &'static str {
    match name {
        "valid_current" => include_str!("../../../tests/fixtures/schema_policy/valid_current.json"),
        "corrupt" => include_str!("../../../tests/fixtures/schema_policy/corrupt.json"),
        "legacy_runtime_no_schema" => {
            include_str!("../../../tests/fixtures/schema_policy/legacy_runtime_no_schema.json")
        }
        "authoring_corrupt" => {
            include_str!("../../../tests/fixtures/schema_policy/authoring_corrupt.vnauthoring")
        }
        "old_schema" => include_str!("../../../tests/fixtures/schema_policy/old_schema.json"),
        "missing_schema" => {
            include_str!("../../../tests/fixtures/schema_policy/missing_schema.json")
        }
        _ => unreachable!("unknown schema fixture"),
    }
}

#[test]
fn script_json_rejects_missing_schema_version_for_legacy_inputs() {
    let script_json = r#"{
        "events": [],
        "labels": {"start": 0}
    }"#;

    let err = ScriptRaw::from_json(script_json).expect_err("missing schema must be rejected");
    assert!(
        err.to_string().contains("missing script_schema_version"),
        "unexpected error: {err}"
    );
}

#[test]
fn script_json_accepts_missing_schema_only_with_explicit_legacy_policy() {
    let script_json = r#"{
        "events": [],
        "labels": {"start": 0}
    }"#;

    let script = ScriptRaw::from_json_with_policy(script_json, SchemaPolicy::LegacyReadOnly)
        .expect("legacy policy should accept missing schema");
    assert_eq!(script.labels.get("start"), Some(&0));
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

#[test]
fn script_json_accepts_legacy_schema_only_with_explicit_migration_policy() {
    let script_json = r#"{
        "script_schema_version": "0.9",
        "events": [],
        "labels": {"start": 0}
    }"#;

    let script = ScriptRaw::from_json_with_policy(script_json, SchemaPolicy::Migrating)
        .expect("migration policy should accept legacy schema");
    assert_eq!(script.labels.get("start"), Some(&0));
}

#[test]
fn schema_policy_fixture_matrix_matches_core_contract() {
    let strict_cases = [
        ("valid_current", true),
        ("corrupt", false),
        ("legacy_runtime_no_schema", false),
        ("authoring_corrupt", false),
        ("old_schema", false),
        ("missing_schema", false),
    ];
    for (name, accepted) in strict_cases {
        let result = ScriptRaw::from_json(fixture(name));
        assert_eq!(
            result.is_ok(),
            accepted,
            "strict schema acceptance mismatch for fixture {name}: {result:?}"
        );
    }

    let legacy_cases = [
        ("legacy_runtime_no_schema", true),
        ("old_schema", true),
        ("missing_schema", true),
        ("valid_current", true),
        ("corrupt", false),
        ("authoring_corrupt", false),
    ];
    for (name, accepted) in legacy_cases {
        let result = ScriptRaw::from_json_with_policy(fixture(name), SchemaPolicy::LegacyReadOnly);
        assert_eq!(
            result.is_ok(),
            accepted,
            "legacy schema acceptance mismatch for fixture {name}: {result:?}"
        );
    }
}
