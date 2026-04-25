use std::collections::BTreeMap;

use serde_json::json;
use visual_novel_engine::{
    migrate_script_json_to_current, migrate_script_json_value, EventRaw, ScriptRaw,
    SCRIPT_SCHEMA_VERSION,
};

#[test]
fn migration_n_to_current_roundtrip() {
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

    let (migrated_json, report) = migrate_script_json_to_current(&legacy.to_string())
        .expect("legacy migration should succeed");
    assert_eq!(report.from_version, "0.9");
    assert_eq!(report.to_version, SCRIPT_SCHEMA_VERSION);
    assert!(!report.entries.is_empty());

    let script = ScriptRaw::from_json(&migrated_json).expect("migrated script should parse");
    assert_eq!(script.labels.get("start"), Some(&0usize));
    match &script.events[0] {
        EventRaw::ExtCall { command, args } => {
            assert_eq!(command, "boot_minigame");
            assert!(args.is_empty());
        }
        other => panic!("expected ext_call after migration, got {other:?}"),
    }

    let stable_roundtrip = ScriptRaw::from_json(&script.to_json().expect("json"))
        .expect("roundtrip parse should succeed");
    assert_eq!(stable_roundtrip.labels, script.labels);
    assert_eq!(stable_roundtrip.events.len(), script.events.len());
}

#[test]
fn migration_idempotencia() {
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0usize);
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(visual_novel_engine::DialogueRaw {
            speaker: "Narrador".to_string(),
            text: "Hola".to_string(),
        })],
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
        err.to_string().contains("events must be an array"),
        "unexpected error: {err}"
    );
}
