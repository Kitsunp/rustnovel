//! Thin adapter between the headless authoring graph and the egui wrapper.
//!
//! The GUI `NodeGraph` now owns a `visual_novel_engine::authoring::NodeGraph`
//! directly. This module remains as a small compatibility boundary for code
//! paths that still ask for an explicit conversion.

use visual_novel_engine::authoring::NodeGraph as AuthoringGraph;

use super::node_graph::NodeGraph;

pub(crate) fn to_authoring_graph(graph: &NodeGraph) -> AuthoringGraph {
    graph.authoring_graph().clone()
}

pub(crate) fn from_authoring_graph(authoring: &AuthoringGraph) -> NodeGraph {
    NodeGraph::from_authoring_graph(authoring.clone())
}

pub(crate) fn replace_gui_semantics_from_authoring(
    graph: &mut NodeGraph,
    authoring: &AuthoringGraph,
) {
    let selected = graph.selected;
    let selected_node = selected.and_then(|id| graph.get_node(id).cloned());
    let pan = graph.pan;
    let zoom = graph.zoom;
    let editing = graph.editing;
    let editing_node = editing.and_then(|id| graph.get_node(id).cloned());
    let dragging_node = graph.dragging_node;
    let dragged_node = dragging_node.and_then(|id| graph.get_node(id).cloned());
    let connecting_from = graph.connecting_from;
    let connecting_node = connecting_from.and_then(|(id, _)| graph.get_node(id).cloned());
    let context_menu = graph.context_menu.clone();
    let context_node = context_menu
        .as_ref()
        .and_then(|menu| graph.get_node(menu.node_id).cloned());

    graph.replace_authoring_graph(authoring.clone());
    graph.selected = selected.filter(|id| same_node(graph, *id, selected_node.as_ref()));
    graph.pan = pan;
    graph.zoom = zoom;
    graph.editing = editing.filter(|id| same_node(graph, *id, editing_node.as_ref()));
    graph.dragging_node = dragging_node.filter(|id| same_node(graph, *id, dragged_node.as_ref()));
    graph.connecting_from =
        connecting_from.filter(|(id, _)| same_node(graph, *id, connecting_node.as_ref()));
    graph.context_menu =
        context_menu.filter(|menu| same_node(graph, menu.node_id, context_node.as_ref()));
}

fn same_node(graph: &NodeGraph, node_id: u32, previous: Option<&crate::editor::StoryNode>) -> bool {
    graph
        .get_node(node_id)
        .is_some_and(|current| Some(current) == previous)
}

#[cfg(test)]
mod tests {
    use eframe::egui;
    use visual_novel_engine::{authoring::NodeGraph as AuthoringGraph, CharacterPlacementRaw};

    use super::*;
    use crate::editor::StoryNode;

    #[test]
    fn adapter_preserves_view_state_while_replacing_semantics() {
        let mut graph = NodeGraph::new();
        let old = graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
        graph.selected = Some(old);
        graph.pan = egui::vec2(8.0, 9.0);
        graph.zoom = 1.7;

        let mut next = AuthoringGraph::new();
        let scene = next.add_node(
            StoryNode::Scene {
                profile: None,
                background: Some("bg/room.png".to_string()),
                music: None,
                characters: vec![CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    ..Default::default()
                }],
            },
            visual_novel_engine::authoring::AuthoringPosition::new(4.0, 5.0),
        );

        replace_gui_semantics_from_authoring(&mut graph, &next);

        assert_eq!(graph.selected, None);
        assert_eq!(graph.pan, egui::vec2(8.0, 9.0));
        assert_eq!(graph.zoom, 1.7);
        assert!(matches!(
            graph.get_node(scene),
            Some(StoryNode::Scene { .. })
        ));
    }
}
