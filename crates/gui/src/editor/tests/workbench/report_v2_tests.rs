use super::super::*;
use crate::editor::StoryNode;

#[test]
fn workbench_diagnostic_report_json_contains_bilingual_fields() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    workbench.validation_issues.push(
        crate::editor::validator::LintIssue::error(
            Some(7),
            crate::editor::ValidationPhase::Graph,
            crate::editor::LintCode::EmptySpeakerName,
            "Speaker is empty",
        )
        .with_event_ip(Some(3))
        .with_target(
            visual_novel_engine::authoring::DiagnosticTarget::Character {
                node_id: Some(7),
                name: String::new(),
                field_path: Some(visual_novel_engine::authoring::FieldPath::new(
                    "graph.nodes[7].speaker",
                )),
            },
        )
        .with_field_path("graph.nodes[7].speaker")
        .with_evidence_trace(),
    );

    let payload = workbench
        .diagnostic_report_json()
        .expect("report json should be built");
    let parsed: serde_json::Value = serde_json::from_str(&payload).expect("valid json");
    let issue = &parsed["issues"][0];

    assert!(issue["diagnostic_id"]
        .as_str()
        .is_some_and(|id| id.starts_with("authoring-diagnostic-v2:GRAPH:VAL_SPEAKER_EMPTY:7:3")));
    assert!(issue["target"].is_object());
    assert!(issue["evidence_trace"].is_object());
    assert!(issue["message_es"].as_str().is_some());
    assert!(issue["message_en"].as_str().is_some());
    assert!(issue["why_failed_es"].as_str().is_some());
    assert!(issue["why_failed_en"].as_str().is_some());
    assert_eq!(
        parsed["fingerprints"]["fingerprint_schema_version"],
        "vnengine.authoring.fingerprint.v2"
    );
    assert_eq!(
        parsed["fingerprints"]["script_sha256"]
            .as_str()
            .expect("script hash")
            .len(),
        64
    );
    assert_eq!(
        parsed["fingerprints"]["graph_sha256"]
            .as_str()
            .expect("graph hash")
            .len(),
        64
    );
    assert_eq!(parsed["schema"], "vnengine.authoring_validation_report.v2");
    assert_eq!(
        parsed["fingerprints"]["story_semantic_sha256"]
            .as_str()
            .expect("semantic hash")
            .len(),
        64
    );
}

#[test]
fn report_v2_import_preserves_target_field_path_and_stale_state() {
    let config = VnConfig::default();
    let mut source = EditorWorkbench::new(config.clone());
    source.validation_issues.push(
        crate::editor::validator::LintIssue::warning(
            Some(12),
            crate::editor::ValidationPhase::Graph,
            crate::editor::LintCode::ChoiceOptionUnlinked,
            "Choice option 2 has no outgoing connection",
        )
        .with_target(
            visual_novel_engine::authoring::DiagnosticTarget::ChoiceOption {
                node_id: 12,
                option_index: 1,
            },
        )
        .with_field_path("graph.nodes[12].options[1].target")
        .with_evidence_trace(),
    );
    let payload = source
        .diagnostic_report_json()
        .expect("report should serialize");

    let mut target = EditorWorkbench::new(config);
    target
        .apply_diagnostic_report_json(&payload)
        .expect("report should import");

    let issue = &target.validation_issues[0];
    assert!(matches!(
        issue.target,
        Some(
            visual_novel_engine::authoring::DiagnosticTarget::ChoiceOption {
                node_id: 12,
                option_index: 1,
            }
        )
    ));
    assert_eq!(
        issue.field_path.as_ref().map(|path| path.value.as_str()),
        Some("graph.nodes[12].options[1].target")
    );
    assert!(!target.imported_report_untrusted);
}

#[test]
fn editor_mutation_operation_log_records_before_after_fingerprints() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    workbench.refresh_operation_fingerprint();

    let created = workbench.add_composer_created_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(80.0, 120.0),
    );
    workbench.queue_editor_operation(
        "test_create_node",
        "Created node in test",
        Some(format!("graph.nodes[{created}]")),
    );
    workbench.record_pending_editor_operation();

    let entry = workbench
        .operation_log
        .last()
        .expect("operation should be logged");
    assert_eq!(entry.operation_kind, "test_create_node");
    assert!(entry.before_fingerprint_sha256.is_some());
    assert!(entry.after_fingerprint_sha256.is_some());
    let expected_path = format!("graph.nodes[{created}]");
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some(expected_path.as_str())
    );
}
