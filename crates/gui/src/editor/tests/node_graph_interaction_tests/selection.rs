use super::*;

#[test]
fn selected_group_drag_moves_all_selected_nodes() {
    let mut graph = NodeGraph::new();
    let a = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let b = graph.add_node(StoryNode::End, pos(100.0, 100.0));
    graph.toggle_multi_selection(a);
    graph.toggle_multi_selection(b);

    assert_eq!(
        graph.translate_selected_or_node(a, egui::vec2(10.0, 15.0)),
        2
    );
    assert_eq!(graph.get_node_pos(a), Some(pos(10.0, 15.0)));
    assert_eq!(graph.get_node_pos(b), Some(pos(110.0, 115.0)));
}

#[test]
fn drag_translation_marks_move_without_requesting_extra_undo_snapshot() {
    let mut graph = NodeGraph::new();
    let node = graph.add_node(StoryNode::Start, pos(0.0, 0.0));

    assert_eq!(
        graph.translate_selected_or_node_for_drag(node, egui::vec2(10.0, 5.0)),
        1
    );

    assert!(graph.is_modified());
    assert_eq!(graph.get_node_pos(node), Some(pos(10.0, 5.0)));
    let hint = graph
        .take_operation_hint()
        .expect("drag movement should leave an operation hint");
    assert_eq!(hint.kind, "node_moved");
    assert!(
        !hint.push_undo_snapshot,
        "drag start already owns the undo snapshot"
    );
}

#[test]
fn duplicating_selection_preserves_internal_edges_and_group_selection() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Copied branch".to_string(),
        },
        pos(0.0, 100.0),
    );
    let outside = graph.add_node(StoryNode::End, pos(0.0, 220.0));
    graph.connect(start, dialogue);
    graph.connect(dialogue, outside);
    graph.toggle_multi_selection(start);
    graph.toggle_multi_selection(dialogue);

    let copied = graph.duplicate_selected_nodes();

    assert_eq!(copied.len(), 2);
    assert_eq!(graph.selected_node_ids(), copied);
    let copied_start = copied
        .iter()
        .copied()
        .find(|id| matches!(graph.get_node(*id), Some(StoryNode::Start)))
        .expect("copied start should exist");
    let copied_dialogue = copied
        .iter()
        .copied()
        .find(|id| matches!(graph.get_node(*id), Some(StoryNode::Dialogue { .. })))
        .expect("copied dialogue should exist");
    assert!(graph
        .connections()
        .any(|conn| conn.from == copied_start && conn.to == copied_dialogue));
    assert!(
        !graph
            .connections()
            .any(|conn| conn.from == copied_dialogue && conn.to == outside),
        "duplicate should not silently keep external outgoing edges"
    );
    let hint = graph
        .take_operation_hint()
        .expect("batch duplicate should leave one operation hint");
    assert_eq!(hint.kind, "node_created");
    assert_eq!(hint.field_path.as_deref(), Some("graph.nodes"));
}

#[test]
fn marquee_selection_replaces_or_extends_selection() {
    let mut graph = NodeGraph::new();
    let a = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let b = graph.add_node(StoryNode::End, pos(300.0, 0.0));
    let rect = egui::Rect::from_min_max(pos(-10.0, -10.0), pos(160.0, 120.0));

    assert_eq!(graph.select_nodes_in_rect(rect, false), 1);
    assert_eq!(graph.selected_node_ids(), vec![a]);

    let rect_b = egui::Rect::from_min_max(pos(280.0, -10.0), pos(460.0, 120.0));
    assert_eq!(graph.select_nodes_in_rect(rect_b, true), 1);
    let mut selected = graph.selected_node_ids();
    selected.sort_unstable();
    assert_eq!(selected, vec![a, b]);
}

#[test]
fn ctrl_click_can_deselect_last_selected_node() {
    let mut graph = NodeGraph::new();
    let a = graph.add_node(StoryNode::Start, pos(0.0, 0.0));

    graph.toggle_multi_selection(a);
    assert_eq!(graph.selected_node_ids(), vec![a]);

    graph.toggle_multi_selection(a);
    assert!(graph.selected_node_ids().is_empty());
    assert_eq!(graph.selected, None);
}

#[test]
fn single_selection_clears_previous_multi_selection() {
    let mut graph = NodeGraph::new();
    let a = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let b = graph.add_node(StoryNode::End, pos(100.0, 0.0));

    graph.toggle_multi_selection(a);
    graph.toggle_multi_selection(b);
    assert_eq!(graph.selected_node_ids().len(), 2);

    graph.set_single_selection(Some(b));

    assert_eq!(graph.selected, Some(b));
    assert_eq!(graph.selected_node_ids(), vec![b]);
}

#[test]
fn removing_node_clears_transient_mouse_state_for_that_node() {
    let mut graph = NodeGraph::new();
    let a = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    graph.dragging_node = Some(a);
    graph.start_connection_pick(a, 0);
    graph.context_menu = Some(ContextMenu::for_node(a, pos(1.0, 1.0)));

    graph.remove_node(a);

    assert_eq!(graph.dragging_node, None);
    assert_eq!(graph.connecting_from, None);
    assert!(!graph.connecting_sticky);
    assert!(graph.context_menu.is_none());
}

#[test]
fn sticky_connect_to_mode_finishes_by_clicking_target_and_clears_state() {
    let mut graph = NodeGraph::new();
    let source = graph.add_node(
        StoryNode::Dialogue {
            speaker: "N".to_string(),
            text: "Go".to_string(),
        },
        pos(0.0, 0.0),
    );
    let first = graph.add_node(StoryNode::End, pos(-120.0, 180.0));
    let second = graph.add_node(StoryNode::End, pos(120.0, 180.0));

    graph.connect(source, first);
    graph.start_connection_pick(source, 0);

    assert_eq!(graph.connecting_from, Some((source, 0)));
    assert!(graph.connecting_sticky);
    assert!(graph.finish_connection_to(second));
    assert_eq!(graph.connecting_from, None);
    assert!(!graph.connecting_sticky);

    let hub = graph
        .connections()
        .find(|conn| conn.from == source && conn.from_port == 0)
        .map(|conn| conn.to)
        .expect("source should point to explicit branch hub");
    assert!(matches!(
        graph.get_node(hub),
        Some(StoryNode::Choice { .. })
    ));
    assert!(graph
        .connections()
        .any(|conn| conn.from == hub && conn.from_port == 0 && conn.to == first));
    assert!(graph
        .connections()
        .any(|conn| conn.from == hub && conn.from_port == 1 && conn.to == second));
}

#[test]
fn sticky_connect_to_mode_can_be_cancelled_without_touching_edges() {
    let mut graph = NodeGraph::new();
    let source = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let target = graph.add_node(StoryNode::End, pos(0.0, 100.0));
    graph.start_connection_pick(source, 0);

    graph.cancel_connection();

    assert_eq!(graph.connecting_from, None);
    assert!(!graph.connecting_sticky);
    assert_eq!(graph.connection_count(), 0);
    assert!(!graph.connections().any(|conn| conn.to == target));
}
