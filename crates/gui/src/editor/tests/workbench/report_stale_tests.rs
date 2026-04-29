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
    assert!(workbench
        .validation_issues
        .iter()
        .any(|issue| issue.code == LintCode::EmptySpeakerName));
    assert!(workbench.prepare_autofix_batch_confirmation(false).is_err());
    assert_eq!(workbench.apply_all_safe_fixes(), 0);
}
