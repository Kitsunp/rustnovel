use super::*;
use crate::editor::node_types::{node_visual_height, StoryNode, NODE_WIDTH};
use eframe::egui;

#[test]
fn test_node_editor_panel_creation() {
    let mut graph = NodeGraph::new();
    let mut undo = UndoStack::new();
    let _panel = NodeEditorPanel::new(&mut graph, &mut undo);
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
                egui::Rect::from_min_size(*pos, egui::vec2(NODE_WIDTH, node_visual_height(node)));
            (*id, rect)
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
