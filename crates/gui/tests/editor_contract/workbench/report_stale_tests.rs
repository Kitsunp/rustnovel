use super::super::*;
use crate::editor::StoryNode;

#[test]
fn stale_imported_report_blocks_automatic_fixes_but_keeps_issues_readable() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: String::new(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 90.0),
    );
    let end = workbench
        .node_graph
        .add_node(StoryNode::End, egui::pos2(0.0, 180.0));
    workbench.node_graph.connect(start, dialogue);
    workbench.node_graph.connect(dialogue, end);
    let _ = workbench.run_dry_validation();
    let payload = workbench
        .diagnostic_report_json()
        .expect("diagnostic report");

    let Some(StoryNode::Dialogue { speaker, .. }) = workbench.node_graph.get_node_mut(dialogue)
    else {
        panic!("dialogue should exist");
    };
    *speaker = "Narrator".to_string();

    workbench
        .apply_diagnostic_report_json(&payload)
        .expect("stale report still imports");
    assert!(workbench.imported_report_stale);
    assert!(!workbench.imported_report_untrusted);
    assert!(workbench
        .validation_issues
        .iter()
        .any(|issue| issue.code == LintCode::EmptySpeakerName));
    assert!(workbench.prepare_autofix_batch_confirmation(false).is_err());
    assert!(workbench
        .apply_issue_fix(0, "dialogue_fill_speaker")
        .is_err());
    assert_eq!(workbench.apply_all_safe_fixes(), 0);
}

#[test]
fn report_without_fingerprint_imports_as_untrusted_and_blocks_all_fix_paths() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: String::new(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 0.0),
    );
    let _ = workbench.run_dry_validation();
    let payload = workbench
        .diagnostic_report_json()
        .expect("diagnostic report");
    let mut parsed: serde_json::Value = serde_json::from_str(&payload).expect("report json");
    parsed
        .as_object_mut()
        .expect("report object")
        .remove("fingerprints");
    let payload_without_fingerprints =
        serde_json::to_string(&parsed).expect("report should serialize");

    workbench
        .apply_diagnostic_report_json(&payload_without_fingerprints)
        .expect("untrusted report remains readable");

    assert!(workbench.imported_report_stale);
    assert!(workbench.imported_report_untrusted);
    assert!(!workbench.validation_issues.is_empty());
    assert!(workbench.prepare_autofix_batch_confirmation(true).is_err());
    assert!(workbench.apply_best_fix_for_issue(0, true).is_err());
    assert!(workbench
        .apply_issue_fix(0, "dialogue_fill_speaker")
        .is_err());
    assert_eq!(workbench.apply_all_safe_fixes(), 0);
}

#[test]
fn report_stale_check_uses_semantic_fingerprint_not_build_info() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: String::new(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 0.0),
    );
    let _ = workbench.run_dry_validation();
    let payload = workbench
        .diagnostic_report_json()
        .expect("diagnostic report");
    let mut parsed: serde_json::Value = serde_json::from_str(&payload).expect("report json");
    parsed["fingerprints"]["build"]["target_os"] = serde_json::Value::String("other-os".into());
    let payload_with_foreign_build =
        serde_json::to_string(&parsed).expect("report should serialize");

    workbench
        .apply_diagnostic_report_json(&payload_with_foreign_build)
        .expect("semantically matching report imports");

    assert!(!workbench.imported_report_stale);
    assert!(!workbench.imported_report_untrusted);
}
