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
    assert!(
        branch_a_pos.y - start_pos.y > (branch_a_pos.x - start_pos.x).abs(),
        "default branch layout should read primarily top-to-bottom"
    );
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
    assert!(
        unique_x.len() > 1,
        "long default vertical flows should wrap into additional columns"
    );
}

#[test]
fn auto_layout_hierarchical_can_use_horizontal_flow() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Route".to_string(),
            options: vec!["A".to_string(), "B".to_string(), "C".to_string()],
        },
        pos(0.0, 0.0),
    );
    graph.connect(start, choice);
    for idx in 0..3 {
        let branch = graph.add_node(
            StoryNode::Dialogue {
                speaker: format!("B{idx}"),
                text: "line".to_string(),
            },
            pos(0.0, 0.0),
        );
        graph.connect_port(choice, idx, branch);
    }

    graph.auto_layout_hierarchical_with_orientation(GraphLayoutOrientation::Horizontal);

    let mut min = egui::pos2(f32::MAX, f32::MAX);
    let mut max = egui::pos2(f32::MIN, f32::MIN);
    for (_, node, node_pos) in graph.nodes() {
        min.x = min.x.min(node_pos.x);
        min.y = min.y.min(node_pos.y);
        max.x = max.x.max(node_pos.x + node_visual_width(&node));
        max.y = max
            .y
            .max(node_pos.y + crate::editor::node_types::node_visual_height(&node));
    }

    assert!(
        (max.x - min.x) > (max.y - min.y),
        "auto-layout should produce a wider-than-tall graph for ordinary branching"
    );
}

#[test]
fn horizontal_auto_layout_accounts_for_wide_choice_nodes() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Route".to_string(),
            options: vec![
                "A".to_string(),
                "B".to_string(),
                "C".to_string(),
                "D".to_string(),
            ],
        },
        pos(0.0, 0.0),
    );
    let branch = graph.add_node(
        StoryNode::Dialogue {
            speaker: "B".to_string(),
            text: "line".to_string(),
        },
        pos(0.0, 0.0),
    );

    graph.connect(start, choice);
    graph.connect_port(choice, 3, branch);
    graph.auto_layout_hierarchical_with_orientation(GraphLayoutOrientation::Horizontal);

    let choice_pos = node_pos(&graph, choice);
    let branch_pos = node_pos(&graph, branch);
    let choice_width = node_visual_width(graph.get_node(choice).expect("choice node"));

    assert!(
        branch_pos.x >= choice_pos.x + choice_width + 80.0,
        "horizontal layout should place later layers after the full visual Choice width"
    );
}

#[test]
fn horizontal_auto_layout_orders_choice_branches_by_output_port() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Route".to_string(),
            options: vec!["Top".to_string(), "Bottom".to_string()],
        },
        pos(0.0, 0.0),
    );
    let bottom_branch = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Bottom".to_string(),
            text: "line".to_string(),
        },
        pos(0.0, 0.0),
    );
    let top_branch = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Top".to_string(),
            text: "line".to_string(),
        },
        pos(0.0, 0.0),
    );

    graph.connect(start, choice);
    graph.connect_port(choice, 1, bottom_branch);
    graph.connect_port(choice, 0, top_branch);
    graph.auto_layout_hierarchical_with_orientation(GraphLayoutOrientation::Horizontal);

    assert!(
        node_pos(&graph, top_branch).y < node_pos(&graph, bottom_branch).y,
        "horizontal Choice branches should follow output-port order, not node id order"
    );
}

#[test]
fn auto_layout_hierarchical_can_use_vertical_flow() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Route".to_string(),
            options: vec!["A".to_string(), "B".to_string(), "C".to_string()],
        },
        pos(0.0, 0.0),
    );
    graph.connect(start, choice);
    for idx in 0..3 {
        let branch = graph.add_node(
            StoryNode::Dialogue {
                speaker: format!("B{idx}"),
                text: "line".to_string(),
            },
            pos(0.0, 0.0),
        );
        graph.connect_port(choice, idx, branch);
    }

    graph.auto_layout_hierarchical_with_orientation(GraphLayoutOrientation::Vertical);

    let start_pos = node_pos(&graph, start);
    let choice_pos = node_pos(&graph, choice);
    let branch_positions = graph
        .nodes()
        .filter_map(|(_, node, node_pos)| {
            matches!(node, StoryNode::Dialogue { .. }).then_some(node_pos)
        })
        .collect::<Vec<_>>();

    assert!(choice_pos.y > start_pos.y);
    assert!(branch_positions.iter().all(|pos| pos.y > choice_pos.y));
    assert!(
        branch_positions
            .windows(2)
            .any(|pair| (pair[0].x - pair[1].x).abs() > 1.0),
        "vertical auto-layout should fan sibling branches horizontally"
    );
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
                    node_visual_width(&node),
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
