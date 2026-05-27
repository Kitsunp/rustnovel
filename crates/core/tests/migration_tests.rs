use std::collections::BTreeMap;

use serde_json::json;
use visual_novel_engine::{
    migrate_script_json_to_current, migrate_script_json_value,
    runtime::{EventRaw, ScriptRaw},
    SCRIPT_SCHEMA_VERSION,
};

#[test]
fn migration_rejects_legacy_0_9_payload() {
    let legacy = json!({
        "script_schema_version": "0.9",
        "events": [
            {
                "type": "extcall",
                "command": "boot_minigame"
            },
            {
                "type": "dialogue",
                "speaker": "Narrador",
                "text": "Inicio"
            }
        ],
        "labels": { "start": 0 }
    });

    let err = migrate_script_json_to_current(&legacy.to_string())
        .expect_err("legacy script migration must be rejected");
    assert!(
        err.to_string()
            .contains("unsupported script schema version '0.9'"),
        "unexpected error: {err}"
    );
}

#[test]
fn migration_idempotencia() {
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0usize);
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(
            visual_novel_engine::runtime::DialogueRaw {
                speaker: "Narrador".to_string(),
                text: "Hola".to_string(),
            },
        )],
        labels,
    );
    let input = script.to_json().expect("serialize current script");

    let (first, first_report) =
        migrate_script_json_to_current(&input).expect("first migration must succeed");
    let (second, second_report) =
        migrate_script_json_to_current(&first).expect("second migration must succeed");

    assert_eq!(first, second);
    assert!(first_report.entries.is_empty());
    assert!(second_report.entries.is_empty());
    assert_eq!(first_report.from_version, SCRIPT_SCHEMA_VERSION);
    assert_eq!(second_report.from_version, SCRIPT_SCHEMA_VERSION);
}

#[test]
fn rollback_on_failure() {
    let mut invalid = json!({
        "script_schema_version": "0.9",
        "events": {
            "type": "dialogue"
        },
        "labels": { "start": 0 }
    });
    let snapshot = invalid.clone();

    let err = migrate_script_json_value(&mut invalid)
        .expect_err("invalid legacy payload must fail migration");
    assert_eq!(
        invalid, snapshot,
        "migration must rollback original payload"
    );
    assert!(
        err.to_string()
            .contains("unsupported script schema version '0.9'"),
        "unexpected error: {err}"
    );
}
