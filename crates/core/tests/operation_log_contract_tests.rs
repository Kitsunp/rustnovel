use visual_novel_engine::authoring::{
    OperationKind, OperationLogEntry, OperationStatus, OPERATION_LOG_SCHEMA_V2,
};

#[test]
fn operation_log_status_is_typed_while_legacy_status_remains_readable() {
    let entry = OperationLogEntry::new_typed(
        OperationKind::ComposerObjectMoved,
        "applied_with_warnings",
        "object moved and validation introduced a warning",
    )
    .with_status(OperationStatus::AppliedWithWarnings);

    assert_eq!(entry.schema, OPERATION_LOG_SCHEMA_V2);
    assert_eq!(entry.status, "applied_with_warnings");
    assert_eq!(entry.status_v2, Some(OperationStatus::AppliedWithWarnings));
    assert!(entry.operation_id.starts_with("op:"));

    let json = serde_json::to_string(&entry).expect("serialize operation log");
    let roundtrip: OperationLogEntry =
        serde_json::from_str(&json).expect("roundtrip operation log");
    assert_eq!(
        roundtrip.status_v2,
        Some(OperationStatus::AppliedWithWarnings)
    );

    let legacy = serde_json::json!({
        "schema": OPERATION_LOG_SCHEMA_V2,
        "operation_id": "op:legacy",
        "created_unix_ms": 1,
        "operation_kind": "quick_fix",
        "diagnostic_id": null,
        "semantic_fingerprint_sha256": null,
        "status": "applied",
        "details": "legacy entry"
    });
    let parsed: OperationLogEntry = serde_json::from_value(legacy).expect("legacy operation entry");
    assert_eq!(parsed.status, "applied");
    assert_eq!(parsed.status_v2, None);
}
