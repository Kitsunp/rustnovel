use super::super::*;
use crate::editor::StoryNode;

#[test]
fn workbench_autofix_batch_can_prepare_and_apply_complete_mode() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 0.0),
    );
    assert!(workbench
        .node_graph
        .get_node(dialogue)
        .is_some_and(|node| matches!(node, StoryNode::Dialogue { .. })));

    let _ = workbench.run_dry_validation();
    let planned = workbench
        .prepare_autofix_batch_confirmation(true)
        .expect("autofix batch should be planned");
    assert!(planned >= 1);
    assert!(workbench.show_fix_confirm);
    assert!(workbench.pending_auto_fix_batch.is_some());

    let result = workbench
        .apply_pending_autofix_batch()
        .expect("autofix batch should apply");
    assert!(result.applied >= 1);

    assert!(workbench
        .node_graph
        .nodes()
        .any(|(_, node, _)| matches!(node, StoryNode::Start)));
    let fixed_speaker = workbench.node_graph.nodes().find_map(|(id, node, _)| {
        if *id == dialogue {
            if let StoryNode::Dialogue { speaker, .. } = node {
                return Some(speaker.clone());
            }
        }
        None
    });
    assert_eq!(fixed_speaker.as_deref(), Some("Narrator"));
}

#[test]
fn autofix_rollback() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "".to_string(),
            text: "Linea".to_string(),
        },
        egui::pos2(0.0, 100.0),
    );
    workbench.node_graph.connect(start, dialogue);
    let _ = workbench.run_dry_validation();

    let idx = workbench
        .validation_issues
        .iter()
        .position(|issue| issue.code == LintCode::EmptySpeakerName)
        .expect("expected EmptySpeakerName");

    workbench
        .apply_best_fix_for_issue(idx, false)
        .expect("autofix should apply");
    assert!(workbench.revert_last_fix());

    let Some(StoryNode::Dialogue { speaker, .. }) = workbench.node_graph.get_node(dialogue) else {
        panic!("expected dialogue node");
    };
    assert_eq!(speaker, "");
}

#[test]
fn no_silent_mutations_trace() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "".to_string(),
            text: "Linea".to_string(),
        },
        egui::pos2(0.0, 100.0),
    );
    workbench.node_graph.connect(start, dialogue);
    let _ = workbench.run_dry_validation();

    let idx = workbench
        .validation_issues
        .iter()
        .position(|issue| issue.code == LintCode::EmptySpeakerName)
        .expect("expected EmptySpeakerName");

    workbench
        .apply_best_fix_for_issue(idx, false)
        .expect("autofix should apply");
    let last = workbench
        .quick_fix_audit
        .last()
        .expect("quick-fix must be audited");
    assert_ne!(last.before_crc32, last.after_crc32);
    assert!(!last.diagnostic_id.is_empty());
    assert!(!last.fix_id.is_empty());
}

#[test]
fn language_switch_persistence() {
    let config = VnConfig::default();
    let mut source = EditorWorkbench::new(config.clone());
    source.diagnostic_language = crate::editor::DiagnosticLanguage::En;
    source.player_locale = "es".to_string();
    source
        .validation_issues
        .push(crate::editor::validator::LintIssue::warning(
            None,
            crate::editor::ValidationPhase::Graph,
            crate::editor::LintCode::MissingStart,
            "Missing Start node",
        ));

    let payload = source
        .diagnostic_report_json()
        .expect("report should serialize");

    let mut target = EditorWorkbench::new(config);
    target
        .apply_diagnostic_report_json(&payload)
        .expect("report should import");
    assert_eq!(
        target.diagnostic_language,
        crate::editor::DiagnosticLanguage::En
    );
    assert_eq!(target.player_locale, "es");
}

#[test]
fn apply_revert_fix_flow() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 100.0),
    );
    workbench.node_graph.connect(start, dialogue);
    let _ = workbench.run_dry_validation();

    let idx = workbench
        .validation_issues
        .iter()
        .position(|issue| issue.code == LintCode::EmptySpeakerName)
        .expect("expected EmptySpeakerName");
    workbench
        .apply_issue_fix(idx, "dialogue_fill_speaker")
        .expect("fix must apply");
    assert!(workbench.revert_last_fix());
}

#[test]
fn debug_panel_flow() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 0.0),
    );
    let _ = workbench.run_dry_validation();
    assert!(!workbench.validation_issues.is_empty());

    workbench.selected_issue = workbench
        .validation_issues
        .iter()
        .position(|issue| issue.code == LintCode::MissingStart);
    workbench.selected_node = Some(dialogue);
    assert!(workbench.selected_issue.is_some());
    assert_eq!(workbench.selected_node, Some(dialogue));
}

#[test]
fn sync_graph_keeps_validation_panel_closed_for_warnings_only() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 100.0),
    );
    let end = workbench
        .node_graph
        .add_node(StoryNode::End, egui::pos2(0.0, 200.0));
    workbench.node_graph.connect(start, dialogue);
    workbench.node_graph.connect(dialogue, end);

    let _ = workbench.run_dry_validation();
    assert!(workbench.show_validation);
    assert!(workbench
        .validation_issues
        .iter()
        .all(|issue| issue.severity != LintSeverity::Error));

    workbench.show_validation = false;
    let _ = workbench.sync_graph_to_script();
    assert!(
        !workbench.show_validation,
        "warnings should not force-open validation panel after manual close"
    );
}

#[test]
fn sync_graph_reopens_validation_panel_on_errors() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrador".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 0.0),
    );

    workbench.show_validation = false;
    let _ = workbench.sync_graph_to_script();
    assert!(
        workbench.show_validation,
        "blocking errors must force-open validation panel"
    );
    assert!(workbench
        .validation_issues
        .iter()
        .any(|issue| issue.severity == LintSeverity::Error));
}
