use super::*;
use eframe::egui::pos2;
use visual_novel_gui::editor::{LintCode, ValidationPhase};

#[test]
fn py_lint_issue_preserves_traceability_fields() {
    let issue = LintIssue::error(
        Some(7),
        ValidationPhase::DryRun,
        LintCode::DryRunParityMismatch,
        "mismatch",
    )
    .with_event_ip(Some(3));
    let mapped = PyLintIssue::from(issue);

    assert_eq!(mapped.phase, "DRYRUN");
    assert_eq!(mapped.code, "DRY_PARITY_MISMATCH");
    assert_eq!(mapped.node_id, Some(7));
    assert_eq!(mapped.event_ip, Some(3));
    assert_eq!(mapped.edge_from, None);
    assert_eq!(mapped.edge_to, None);
    assert_eq!(mapped.asset_path, None);
    assert_eq!(mapped.diagnostic_id, "DRYRUN:DRY_PARITY_MISMATCH:7:3");
    assert!(!mapped.message_es.is_empty());
    assert!(!mapped.message_en.is_empty());
    assert!(!mapped.root_cause_es.is_empty());
    assert!(!mapped.root_cause_en.is_empty());
    assert!(!mapped.why_failed_es.is_empty());
    assert!(!mapped.why_failed_en.is_empty());
    assert!(!mapped.how_to_fix_es.is_empty());
    assert!(!mapped.how_to_fix_en.is_empty());
    assert!(mapped.docs_ref.starts_with("docs/"));
}

#[test]
fn autofix_helper_selects_review_when_requested() {
    let graph = NodeGraph::new();
    let issue = validate_graph(&graph)
        .into_iter()
        .find(|entry| entry.code == LintCode::MissingStart)
        .expect("missing start issue expected");

    assert!(select_fix_candidate(&issue, &graph, false).is_none());
    let candidate = select_fix_candidate(&issue, &graph, true)
        .expect("review selection should include structural candidate");
    assert_eq!(candidate.fix_id, "graph_add_start");
}

#[test]
fn autofix_safe_pass_applies_deterministic_fix() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos2(0.0, 0.0));
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: "".to_string(),
            text: "Hola".to_string(),
        },
        pos2(0.0, 120.0),
    );
    let end = graph.add_node(StoryNode::End, pos2(0.0, 240.0));
    graph.connect(start, dialogue);
    graph.connect(dialogue, end);

    let applied = apply_autofix_pass(&mut graph, false).expect("safe autofix pass should complete");
    assert!(applied >= 1);

    let remaining = validate_graph(&graph);
    assert!(
        remaining
            .iter()
            .all(|issue| issue.code != LintCode::EmptySpeakerName),
        "empty speaker issue should be fixed"
    );
}
