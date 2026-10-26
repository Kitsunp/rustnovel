use eframe::egui;
use visual_novel_engine::authoring::{AuthoringDocumentCommand, AuthoringPosition};
use visual_novel_gui::{
    editor::{NodeGraph, StoryNode},
    EditorWorkbench, VnConfig,
};

#[test]
fn delayed_field_details_cannot_relabel_a_later_command() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    workbench.queue_editor_operation(
        "field_edited",
        "Stale description",
        Some(format!("graph.nodes[{start}].text")),
    );
    let end = workbench
        .node_graph
        .add_node(StoryNode::End, egui::pos2(0.0, 100.0));
    workbench.record_pending_editor_operation();
    let operation = workbench.document().operation_log.last().unwrap();
    assert_eq!(operation.operation_kind, "node_created");
    assert_ne!(operation.details, "Stale description");
    assert_eq!(
        operation.field_paths[0].value,
        format!("graph.nodes[{end}]")
    );
}

#[test]
fn graph_actions_are_immediately_visible_in_the_owned_document() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    assert!(workbench.document().graph.get_node(start).is_some());
    assert_eq!(workbench.document().operation_log.len(), 1);
    workbench.node_graph.pan = egui::vec2(22.0, 33.0);
    workbench.node_graph.zoom = 0.8;
    workbench.node_graph.create_branch(start);
    let branched = workbench.document().graph.clone();
    assert_eq!(workbench.node_graph.authoring_session.undo_delta_count(), 2);
    assert!(workbench.apply_graph_undo());
    assert_eq!(workbench.document().graph.len(), 1);
    assert_eq!(workbench.node_graph.pan, egui::vec2(22.0, 33.0));
    assert_eq!(workbench.node_graph.zoom, 0.8);
    assert!(workbench.apply_graph_redo());
    assert!(workbench.document().graph.has_same_content(&branched));
}

#[test]
fn dragging_a_group_is_a_view_preview_until_one_document_commit() {
    let mut graph = NodeGraph::new();
    let a = graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let b = graph.add_node(StoryNode::End, egui::pos2(0.0, 100.0));
    graph.selected_nodes.extend([a, b]);
    let history = graph.authoring_session.undo_delta_count();
    for _ in 0..10 {
        graph.translate_selected_or_node_for_drag(a, egui::vec2(1.0, 2.0));
    }
    assert_eq!(graph.get_node_pos(a), Some(egui::pos2(10.0, 20.0)));
    assert_eq!(
        graph.authoring_graph().get_node_pos(a),
        Some(AuthoringPosition::default())
    );
    assert_eq!(graph.authoring_session.undo_delta_count(), history);
    assert!(graph.commit_drag_positions());
    assert_eq!(graph.authoring_session.undo_delta_count(), history + 1);
    assert_eq!(
        graph.authoring_graph().get_node_pos(b),
        Some(AuthoringPosition::new(10.0, 120.0))
    );
    graph
        .authoring_session
        .apply(AuthoringDocumentCommand::UndoLast)
        .unwrap();
    assert_eq!(graph.get_node_pos(a), Some(egui::pos2(0.0, 0.0)));
    graph.translate_selected_or_node_for_drag(a, egui::vec2(5.0, 0.0));
    graph.translate_selected_or_node_for_drag(a, egui::vec2(-5.0, 0.0));
    assert!(!graph.commit_drag_positions());
    assert_eq!(graph.authoring_session.redo_delta_count(), 1);
}

#[test]
fn duplicate_and_insert_are_single_reversible_edits() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let end = graph.add_node(StoryNode::End, egui::pos2(0.0, 100.0));
    graph.connect(start, end);
    let baseline = graph.authoring_graph().clone();
    let history = graph.authoring_session.undo_delta_count();
    graph.insert_after(
        start,
        StoryNode::Dialogue {
            speaker: "Ava".into(),
            text: "Hello".into(),
        },
    );
    assert_eq!(graph.authoring_session.undo_delta_count(), history + 1);
    graph
        .authoring_session
        .apply(AuthoringDocumentCommand::UndoLast)
        .unwrap();
    assert!(graph.authoring_graph().has_same_content(&baseline));
    graph.selected_nodes.extend([start, end]);
    let copies = graph.duplicate_selected_nodes();
    assert_eq!(copies.len(), 2);
    assert_eq!(graph.authoring_session.undo_delta_count(), history + 1);
    graph
        .authoring_session
        .apply(AuthoringDocumentCommand::UndoLast)
        .unwrap();
    assert!(graph.authoring_graph().has_same_content(&baseline));
}

#[test]
fn viewport_changes_do_not_invalidate_compilation_or_document_history() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let end = workbench
        .node_graph
        .add_node(StoryNode::End, egui::pos2(0.0, 100.0));
    workbench.node_graph.connect(start, end);
    workbench.compile_current_graph();
    let history = workbench.node_graph.authoring_session.undo_delta_count();
    workbench.node_graph.pan = egui::vec2(120.0, -40.0);
    workbench.node_graph.zoom = 0.5;
    workbench.node_graph.set_single_selection(Some(end));
    workbench.compile_current_graph();
    assert_eq!(workbench.compilation_cache_stats(), (1, 1));
    assert_eq!(
        workbench.node_graph.authoring_session.undo_delta_count(),
        history
    );
}
