use visual_novel_engine::authoring::{
    OperationKind, OperationLogEntry, OperationStatus, OPERATION_LOG_SCHEMA_V2,
};

#[test]
fn operation_log_status_is_typed_and_rejects_legacy_status() {
    let entry = OperationLogEntry::new_typed(
        OperationKind::ComposerObjectMoved,
        OperationStatus::AppliedWithWarnings,
        "object moved and validation introduced a warning",
    )
    .with_status(OperationStatus::AppliedWithWarnings);

    assert_eq!(entry.schema, OPERATION_LOG_SCHEMA_V2);
    assert_eq!(entry.status, "applied_with_warnings");
    assert_eq!(entry.status_v2, OperationStatus::AppliedWithWarnings);
    assert!(entry.operation_id.starts_with("op:"));

    let json = serde_json::to_string(&entry).expect("serialize operation log");
    let roundtrip: OperationLogEntry =
        serde_json::from_str(&json).expect("roundtrip operation log");
    assert_eq!(roundtrip.status_v2, OperationStatus::AppliedWithWarnings);

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
    let err = serde_json::from_value::<OperationLogEntry>(legacy)
        .expect_err("legacy operation entry without status_v2 must be rejected");
    assert!(err.to_string().contains("status_v2"));
}
