use super::*;

#[test]
fn auto_layout_hierarchical_creates_non_flat_branch_layout() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Ruta".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
        },
        pos(0.0, 0.0),
    );
    let branch_a = graph.add_node(
        StoryNode::Dialogue {
            speaker: "A".to_string(),
            text: "A".to_string(),
        },
        pos(0.0, 0.0),
    );
    let branch_b = graph.add_node(
        StoryNode::Dialogue {
            speaker: "B".to_string(),
            text: "B".to_string(),
        },
        pos(0.0, 0.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 0.0));

    graph.connect(start, choice);
    graph.connect_port(choice, 0, branch_a);
    graph.connect_port(choice, 1, branch_b);
    graph.connect(branch_a, end);
    graph.connect(branch_b, end);

    graph.auto_layout_hierarchical();

    let start_pos = node_pos(&graph, start);
    let choice_pos = node_pos(&graph, choice);
    let branch_a_pos = node_pos(&graph, branch_a);
    let branch_b_pos = node_pos(&graph, branch_b);

    assert!(choice_pos.y > start_pos.y);
    assert!(branch_a_pos.y > choice_pos.y && branch_b_pos.y > choice_pos.y);
    assert_ne!(branch_a_pos.x, branch_b_pos.x);
}

#[test]
fn auto_layout_hierarchical_wraps_long_linear_flows() {
    let mut graph = NodeGraph::new();
    let mut prev = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    for idx in 0..12 {
        let next = graph.add_node(
            StoryNode::Dialogue {
                speaker: "N".to_string(),
                text: format!("line {idx}"),
            },
            pos(0.0, 0.0),
        );
        graph.connect(prev, next);
        prev = next;
    }
    let end = graph.add_node(StoryNode::End, pos(0.0, 0.0));
    graph.connect(prev, end);

    graph.auto_layout_hierarchical();

    let mut unique_x: Vec<i32> = graph
        .nodes()
        .map(|(_, _, pos)| pos.x.round() as i32)
        .collect();
    unique_x.sort_unstable();
    unique_x.dedup();
    assert!(unique_x.len() > 2);
}

#[test]
fn auto_layout_hierarchical_avoids_overlap_for_mixed_heights() {
    let mut graph = NodeGraph::new();
    let mut prev = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    for idx in 0..18 {
        let node = if idx % 3 == 0 {
            StoryNode::Choice {
                prompt: format!("choice {idx}"),
                options: vec![
                    "A".to_string(),
                    "B".to_string(),
                    "C".to_string(),
                    "D".to_string(),
                    "E".to_string(),
                ],
            }
        } else {
            StoryNode::Dialogue {
                speaker: "N".to_string(),
                text: format!("line {idx}"),
            }
        };
        let next = graph.add_node(node, pos(0.0, 0.0));
        graph.connect(prev, next);
        prev = next;
    }
    let end = graph.add_node(StoryNode::End, pos(0.0, 0.0));
    graph.connect(prev, end);

    graph.auto_layout_hierarchical();

    let rects: Vec<(u32, egui::Rect)> = graph
        .nodes()
        .map(|(id, node, node_pos)| {
            let rect = egui::Rect::from_min_size(
                node_pos,
                egui::vec2(
                    NODE_WIDTH,
                    crate::editor::node_types::node_visual_height(&node),
                ),
            );
            (id, rect)
        })
        .collect();

    for left in 0..rects.len() {
        for right in (left + 1)..rects.len() {
            let (left_id, left_rect) = rects[left];
            let (right_id, right_rect) = rects[right];
            assert!(
                !left_rect.intersects(right_rect),
                "auto-layout overlap detected between nodes {left_id} and {right_id}"
            );
        }
    }
}

fn node_pos(graph: &NodeGraph, target: u32) -> egui::Pos2 {
    graph
        .nodes()
        .find(|(id, _, _)| *id == target)
        .map(|(_, _, pos)| pos)
        .expect("node position")
}
