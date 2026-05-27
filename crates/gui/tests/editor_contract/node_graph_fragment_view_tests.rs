use super::*;

fn pos(x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(x, y)
}

fn dialogue(text: &str) -> StoryNode {
    StoryNode::Dialogue {
        speaker: "Narrator".to_string(),
        text: text.to_string(),
    }
}

#[test]
fn active_fragment_hides_external_nodes_from_gui_view() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let inner_a = graph.add_node(dialogue("inside a"), pos(100.0, 0.0));
    let inner_b = graph.add_node(dialogue("inside b"), pos(200.0, 0.0));
    let end = graph.add_node(StoryNode::End, pos(300.0, 0.0));

    graph.connect(start, inner_a);
    graph.connect(inner_a, inner_b);
    graph.connect(inner_b, end);
    graph.set_single_selection(Some(inner_a));
    graph.toggle_multi_selection(inner_b);
    assert!(graph.create_fragment_from_selection("frag", "Fragment"));

    let root_nodes = graph
        .visible_nodes()
        .map(|(id, _, _)| id)
        .collect::<Vec<_>>();
    assert_eq!(root_nodes, vec![start, end]);
    assert!(graph.visible_connections().next().is_none());

    assert!(graph.enter_fragment("frag"));
    let fragment_nodes = graph
        .visible_nodes()
        .map(|(id, _, _)| id)
        .collect::<Vec<_>>();
    assert_eq!(fragment_nodes, vec![inner_a, inner_b]);
    let fragment_edges = graph.visible_connections().collect::<Vec<_>>();
    assert_eq!(fragment_edges.len(), 1);
    assert_eq!(fragment_edges[0].from, inner_a);
    assert_eq!(fragment_edges[0].to, inner_b);
}
