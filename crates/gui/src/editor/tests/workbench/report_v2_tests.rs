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
        .with_blocked_by("authoring-diagnostic-v2:GRAPH:VAL_CHOICE_EMPTY:12:na:na")
        .with_operation_id("op:import-preserve")
        .with_evidence_trace(),
    );
    let original_envelope = source.validation_issues[0].envelope_v2();
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
    assert_eq!(
        issue.blocked_by.as_deref(),
        Some("authoring-diagnostic-v2:GRAPH:VAL_CHOICE_EMPTY:12:na:na")
    );
    let imported_envelope = issue.envelope_v2();
    assert_eq!(
        imported_envelope.operation_id.as_deref(),
        Some("op:import-preserve")
    );
    assert_eq!(imported_envelope.trace_id, original_envelope.trace_id);
    assert!(!target.imported_report_untrusted);
}

#[test]
fn report_v2_import_focuses_selected_issue_from_granular_target() {
    let config = VnConfig::default();
    let mut source = EditorWorkbench::new(config.clone());
    let choice_id = source.node_graph.add_node(
        StoryNode::Choice {
            prompt: "Choose".to_string(),
            options: vec!["Left".to_string(), "Right".to_string()],
        },
        egui::pos2(100.0, 140.0),
    );
    source.validation_issues.push(
        crate::editor::validator::LintIssue::warning(
            None,
            crate::editor::ValidationPhase::Graph,
            crate::editor::LintCode::ChoiceOptionUnlinked,
            "Choice option has no outgoing connection",
        )
        .with_target(
            visual_novel_engine::authoring::DiagnosticTarget::ChoiceOption {
                node_id: choice_id,
                option_index: 1,
            },
        )
        .with_field_path(format!("graph.nodes[{choice_id}].options[1].target"))
        .with_evidence_trace(),
    );
    source.selected_node = None;
    source.selected_issue = Some(0);
    let payload = source
        .diagnostic_report_json()
        .expect("report should serialize");

    let mut target = EditorWorkbench::new(config);
    target.node_graph = source.node_graph.clone();
    target
        .apply_diagnostic_report_json(&payload)
        .expect("report should import");

    assert_eq!(target.selected_issue, Some(0));
    assert_eq!(target.selected_node, Some(choice_id));
    assert!(!target.imported_report_stale);
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

#[test]
fn graph_field_edit_hint_records_before_after_values() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let node_id = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Before".to_string(),
        },
        egui::pos2(80.0, 120.0),
    );
    workbench.node_graph.clear_operation_hint();
    workbench.node_graph.clear_modified();
    workbench.refresh_operation_fingerprint();

    let before_graph = workbench.node_graph.clone();
    let before_node = workbench
        .node_graph
        .get_node(node_id)
        .cloned()
        .expect("node exists");
    let StoryNode::Dialogue { text, .. } = workbench
        .node_graph
        .get_node_mut(node_id)
        .expect("node exists")
    else {
        panic!("expected dialogue");
    };
    *text = "After".to_string();
    let after_node = workbench
        .node_graph
        .get_node(node_id)
        .cloned()
        .expect("node exists");
    workbench.node_graph.queue_operation_hint_with_values(
        "field_edited",
        format!("Edited node {node_id}"),
        Some(format!("graph.nodes[{node_id}]")),
        serde_json::to_string(&before_node).ok(),
        serde_json::to_string(&after_node).ok(),
        true,
    );
    workbench.node_graph.mark_modified();
    workbench.commit_modified_graph(before_graph);

    let entry = workbench
        .operation_log
        .last()
        .expect("field edit should be logged");
    assert_eq!(entry.operation_kind, "field_edited");
    assert!(matches!(
        entry.operation_kind_v2,
        Some(visual_novel_engine::authoring::OperationKind::FieldEdited)
    ));
    let expected_path = format!("graph.nodes[{node_id}]");
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some(expected_path.as_str())
    );
    assert!(entry
        .before_value
        .as_deref()
        .is_some_and(|value| value.contains("Before")));
    assert!(entry
        .after_value
        .as_deref()
        .is_some_and(|value| value.contains("After")));
}

#[test]
fn fragment_port_refresh_is_logged_as_typed_field_edit() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let mid = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Inside".to_string(),
        },
        egui::pos2(0.0, 100.0),
    );
    let end = workbench
        .node_graph
        .add_node(StoryNode::End, egui::pos2(0.0, 200.0));
    workbench.node_graph.connect(start, mid);
    assert!(workbench
        .node_graph
        .authoring
        .create_fragment("frag", "Fragment", vec![mid]));

    workbench.node_graph.connect(mid, end);
    workbench.node_graph.clear_operation_hint();
    workbench.node_graph.clear_modified();
    workbench.refresh_operation_fingerprint();

    let before = workbench.node_graph.clone();
    assert!(workbench.node_graph.refresh_fragment_ports("frag"));
    workbench.node_graph.mark_modified();
    workbench.queue_editor_operation(
        "field_edited",
        "Refreshed ports for fragment frag",
        Some("graph.fragments[frag].ports".to_string()),
    );
    workbench.commit_modified_graph(before);

    let entry = workbench
        .operation_log
        .last()
        .expect("fragment refresh should be logged");
    assert_eq!(entry.operation_kind, "field_edited");
    assert!(matches!(
        entry.operation_kind_v2,
        Some(visual_novel_engine::authoring::OperationKind::FieldEdited)
    ));
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some("graph.fragments[frag].ports")
    );
}

#[test]
fn composer_layer_changes_are_typed_operations() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    workbench.refresh_operation_fingerprint();

    let before = workbench.node_graph.clone();
    workbench.composer_layer_overrides.insert(
        "node:7:Background:0:graph_nodes_7_visual_background".to_string(),
        visual_novel_engine::authoring::composer::LayerOverride {
            visible: false,
            locked: false,
        },
    );
    workbench.node_graph.mark_modified();
    workbench.queue_editor_operation_with_values(
        "layer_visibility_changed",
        "Set layer node:7 visible=false",
        Some("composer.layers[node:7].visible".to_string()),
        Some("true".to_string()),
        Some("false".to_string()),
    );
    workbench.commit_modified_graph(before);

    let entry = workbench
        .operation_log
        .last()
        .expect("layer operation should be logged");
    assert_eq!(entry.operation_kind, "layer_visibility_changed");
    assert!(matches!(
        entry.operation_kind_v2,
        Some(visual_novel_engine::authoring::OperationKind::LayerVisibilityChanged)
    ));
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some("composer.layers[node:7].visible")
    );
    assert_ne!(
        entry.before_fingerprint_sha256, entry.after_fingerprint_sha256,
        "layer overrides must be reflected by operation fingerprints"
    );
    assert_eq!(entry.before_value.as_deref(), Some("true"));
    assert_eq!(entry.after_value.as_deref(), Some("false"));
    assert_eq!(workbench.verification_runs.len(), 1);
}

#[test]
fn editor_undo_redo_are_typed_operations_and_keep_redo_available() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    workbench.refresh_operation_fingerprint();

    let before_create = workbench.node_graph.clone();
    let _created = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(80.0, 120.0),
    );
    workbench.commit_modified_graph(before_create);
    assert_eq!(workbench.node_graph.len(), 1);
    assert_eq!(
        workbench
            .operation_log
            .last()
            .map(|entry| entry.operation_kind.as_str()),
        Some("node_created")
    );

    assert!(workbench.apply_graph_undo());

    assert_eq!(workbench.node_graph.len(), 0);
    assert!(workbench.undo_stack.can_redo());
    assert_eq!(
        workbench
            .operation_log
            .last()
            .map(|entry| entry.operation_kind.as_str()),
        Some("undo")
    );

    assert!(workbench.apply_graph_redo());

    assert_eq!(workbench.node_graph.len(), 1);
    assert_eq!(
        workbench
            .operation_log
            .last()
            .map(|entry| entry.operation_kind.as_str()),
        Some("redo")
    );
    assert_eq!(workbench.verification_runs.len(), 3);
}
