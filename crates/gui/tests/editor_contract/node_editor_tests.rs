use super::*;
use crate::editor::node_types::{node_visual_height, StoryNode, NODE_WIDTH};
use eframe::egui;

#[test]
fn node_editor_panel_creation_paints_canvas_and_keeps_graph_state() {
    let mut graph = NodeGraph::new();
    let mut undo = UndoStack::new();
    let node_id = graph.add_node(StoryNode::Start, egui::pos2(120.0, 90.0));
    let ctx = egui::Context::default();
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 420.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                NodeEditorPanel::new(&mut graph, &mut undo).ui(ui);
            });
        },
    );

    assert!(
        !output.shapes.is_empty(),
        "NodeEditorPanel should paint the grid and nodes in a real egui frame"
    );
    assert_eq!(graph.get_node(node_id), Some(&StoryNode::Start));
    assert!(
        graph.context_menu.is_none(),
        "passive render should not synthesize context menus"
    );
}

#[test]
fn extended_node_palette_exposes_runtime_authoring_nodes() {
    let labels = extended_node_palette_items()
        .into_iter()
        .map(|(label, _)| label)
        .collect::<Vec<_>>();

    for required in [
        "Scene Patch",
        "Branch If",
        "Set Variable",
        "Set Flag",
        "Audio",
        "Transition",
        "Character Placement",
        "ExtCall",
        "Subgraph Call",
    ] {
        assert!(
            labels.contains(&required),
            "missing authoring palette node {required}"
        );
    }
}

#[test]
fn node_at_position_respects_dynamic_choice_height() {
    let mut graph = NodeGraph::new();
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Pick".to_string(),
            options: vec![
                "A".to_string(),
                "B".to_string(),
                "C".to_string(),
                "D".to_string(),
            ],
        },
        egui::pos2(100.0, 100.0),
    );
    let choice_height = node_visual_height(graph.get_node(choice).expect("choice node"));
    let probe = egui::pos2(100.0 + NODE_WIDTH * 0.5, 100.0 + choice_height - 4.0);
    assert_eq!(graph.node_at_position(probe), Some(choice));
}

#[test]
fn auto_layout_hierarchical_resolves_overlaps_in_dense_graph() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let mut layer = Vec::new();
    for idx in 0..8 {
        let node = graph.add_node(
            StoryNode::Choice {
                prompt: format!("Choice {idx}"),
                options: vec![
                    "A".to_string(),
                    "B".to_string(),
                    "C".to_string(),
                    "D".to_string(),
                ],
            },
            egui::pos2(0.0, 0.0),
        );
        graph.connect(start, node);
        layer.push(node);
    }

    graph.auto_layout_hierarchical();

    let positioned: Vec<(u32, egui::Rect)> = graph
        .nodes()
        .map(|(id, node, pos)| {
            let rect =
                egui::Rect::from_min_size(pos, egui::vec2(NODE_WIDTH, node_visual_height(&node)));
            (id, rect)
        })
        .collect();

    for i in 0..positioned.len() {
        for j in (i + 1)..positioned.len() {
            let (a_id, a_rect) = positioned[i];
            let (b_id, b_rect) = positioned[j];
            assert!(
                !a_rect.intersects(b_rect),
                "auto-layout should avoid overlap between node {a_id} and node {b_id}"
            );
        }
    }
}

#[test]
fn keyboard_undo_redo_restore_graph_and_leave_typed_operation_hint() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let mut undo = UndoStack::new();
    undo.push(graph.clone());
    let end = graph.add_node(StoryNode::End, egui::pos2(0.0, 120.0));
    graph.connect(start, end);

    {
        let mut panel = NodeEditorPanel::new(&mut graph, &mut undo);
        assert!(panel.apply_undo_shortcut());
    }

    assert_eq!(graph.len(), 1);
    let hint = graph
        .take_operation_hint()
        .expect("undo should create an operation hint");
    assert_eq!(hint.kind, "undo");
    assert!(!hint.push_undo_snapshot);

    {
        let mut panel = NodeEditorPanel::new(&mut graph, &mut undo);
        assert!(panel.apply_redo_shortcut());
    }

    assert_eq!(graph.len(), 2);
    assert_eq!(
        graph
            .take_operation_hint()
            .expect("redo should create an operation hint")
            .kind,
        "redo"
    );
}

#[test]
fn graph_shortcuts_are_scoped_to_hover_or_active_graph_interaction() {
    assert!(graph_shortcut_scope_active(true, false));
    assert!(graph_shortcut_scope_active(false, true));
    assert!(
        !graph_shortcut_scope_active(false, false),
        "node editor shortcuts must not steal keys from composer/inspector panels"
    );
}

#[test]
fn canvas_drag_modes_keep_pan_and_marquee_selection_distinct() {
    let none = egui::Modifiers::default();
    let ctrl = egui::Modifiers {
        ctrl: true,
        ..Default::default()
    };

    assert_eq!(
        canvas_drag_mode(true, false, false, none, false),
        CanvasDragMode::MarqueeSelect,
        "plain left drag on empty canvas keeps marquee multi-select"
    );
    assert_eq!(
        canvas_drag_mode(true, false, false, ctrl, false),
        CanvasDragMode::Pan,
        "Ctrl+left drag pans for keyboard users"
    );
    assert_eq!(
        canvas_drag_mode(true, false, false, none, true),
        CanvasDragMode::Pan,
        "Space+left drag pans like common graph editors"
    );
    assert_eq!(
        canvas_drag_mode(false, true, false, none, false),
        CanvasDragMode::Pan,
        "middle mouse drag pans without stealing marquee selection"
    );
    assert_eq!(
        canvas_drag_mode(false, false, true, none, false),
        CanvasDragMode::Pan,
        "right mouse drag pans while right click remains available for menu"
    );
}

#[test]
fn logic_graph_labels_shrink_inside_narrow_docks() {
    assert_eq!(logic_graph_heading_label(180.0), "Graph");
    assert_eq!(logic_graph_heading_label(320.0), "Logic Graph");
    assert_eq!(node_editor_heading_label(180.0), "Nodes");
    assert_eq!(node_editor_heading_label(320.0), "Node Editor");
    assert!(node_toolbar_is_compact(320.0));
    assert!(!node_toolbar_is_compact(520.0));
}

#[test]
fn node_status_hint_uses_short_copy_when_graph_panel_is_narrow() {
    assert_eq!(
        node_status_hint(260.0, false, false),
        "Left select | Space pan"
    );
    assert_eq!(node_status_hint(260.0, true, false), "Connect - Esc");
    assert_eq!(node_status_hint(260.0, false, true), "Release to select");
    assert!(
        node_status_hint(640.0, false, false).len()
            > node_status_hint(260.0, false, false).len()
    );
}

#[test]
fn active_graph_interaction_keeps_shortcuts_available_after_pointer_leaves_canvas() {
    let mut graph = NodeGraph::new();
    assert!(!graph.has_active_interaction());

    let node = graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    graph.start_connection_pick(node, 0);

    assert!(graph.has_active_interaction());
}
