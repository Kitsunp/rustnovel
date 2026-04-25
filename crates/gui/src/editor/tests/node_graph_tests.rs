use super::*;

fn pos(x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(x, y)
}

#[test]
fn test_node_graph_new_is_empty() {
    let graph = NodeGraph::new();
    assert!(graph.is_empty());
    assert_eq!(graph.len(), 0);
    assert_eq!(graph.connection_count(), 0);
    assert!(!graph.is_modified());
}

#[test]
fn test_node_graph_add_node() {
    let mut graph = NodeGraph::new();
    let id1 = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let id2 = graph.add_node(StoryNode::End, pos(100.0, 100.0));
    assert_eq!(graph.len(), 2);
    assert_ne!(id1, id2);
    assert!(graph.is_modified());
}

#[test]
fn test_node_graph_remove_node() {
    let mut graph = NodeGraph::new();
    let id1 = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let id2 = graph.add_node(StoryNode::End, pos(100.0, 100.0));
    graph.connect(id1, id2);
    graph.remove_node(id1);
    assert_eq!(graph.len(), 1);
    assert_eq!(graph.connection_count(), 0);
}

#[test]
fn test_node_graph_connect() {
    let mut graph = NodeGraph::new();
    let a = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let b = graph.add_node(StoryNode::End, pos(100.0, 100.0));
    graph.connect(a, b);
    assert_eq!(graph.connection_count(), 1);
    graph.connect(a, b); // Duplicate
    assert_eq!(graph.connection_count(), 1);
}

#[test]
fn test_node_graph_self_loop_prevented() {
    let mut graph = NodeGraph::new();
    let a = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    graph.connect(a, a);
    assert_eq!(graph.connection_count(), 0);
}

#[test]
fn test_zoom_clamp() {
    let mut graph = NodeGraph::new();
    graph.set_zoom(0.0);
    assert_eq!(graph.zoom(), ZOOM_MIN);
    graph.set_zoom(10.0);
    assert_eq!(graph.zoom(), ZOOM_MAX);
}

#[test]
fn test_insert_before() {
    let mut graph = NodeGraph::new();
    let a = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let c = graph.add_node(StoryNode::End, pos(0.0, 100.0));
    graph.connect(a, c);
    graph.insert_before(c, StoryNode::default());
    assert_eq!(graph.len(), 3);
    assert_eq!(graph.connection_count(), 2);
}

#[test]
fn test_insert_after() {
    let mut graph = NodeGraph::new();
    let a = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let c = graph.add_node(StoryNode::End, pos(0.0, 100.0));
    graph.connect(a, c);
    graph.insert_after(a, StoryNode::default());
    assert_eq!(graph.len(), 3);
    assert_eq!(graph.connection_count(), 2);
}

#[test]
fn test_create_branch() {
    let mut graph = NodeGraph::new();
    let a = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    graph.create_branch(a);
    assert_eq!(graph.len(), 4);
    assert_eq!(graph.connection_count(), 3);
}

#[test]
fn test_create_branch_from_end_does_nothing() {
    let mut graph = NodeGraph::new();
    let end = graph.add_node(StoryNode::End, pos(0.0, 0.0));
    graph.create_branch(end);
    assert_eq!(graph.len(), 1);
    assert_eq!(graph.connection_count(), 0);
}

#[test]
fn test_connecting_choice_port_auto_creates_option() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Select".to_string(),
            options: vec!["A".to_string()],
        },
        pos(0.0, 100.0),
    );
    let target = graph.add_node(
        StoryNode::Dialogue {
            speaker: "N".to_string(),
            text: "B".to_string(),
        },
        pos(200.0, 100.0),
    );

    graph.connect(start, choice);
    graph.connect_port(choice, 1, target);

    let Some(StoryNode::Choice { options, .. }) = graph.get_node(choice) else {
        panic!("choice node should exist");
    };
    assert_eq!(options.len(), 2);
    assert_eq!(graph.connection_count(), 2);
}

#[test]
fn test_disconnect_port_removes_only_selected_output_port() {
    let mut graph = NodeGraph::new();
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Select".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
        },
        pos(0.0, 0.0),
    );
    let a = graph.add_node(StoryNode::End, pos(-100.0, 100.0));
    let b = graph.add_node(StoryNode::End, pos(100.0, 100.0));

    graph.connect_port(choice, 0, a);
    graph.connect_port(choice, 1, b);
    assert_eq!(graph.connection_count(), 2);

    graph.disconnect_port(choice, 1);
    assert_eq!(graph.connection_count(), 1);
    assert!(graph
        .connections()
        .any(|c| c.from == choice && c.from_port == 0));
}

#[test]
fn test_bookmark_roundtrip_and_cleanup_on_node_remove() {
    let mut graph = NodeGraph::new();
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Hola".to_string(),
        },
        pos(0.0, 0.0),
    );

    assert!(graph.set_bookmark("intro", dialogue));
    assert_eq!(graph.bookmarked_node("intro"), Some(dialogue));

    graph.remove_node(dialogue);
    assert_eq!(graph.bookmarked_node("intro"), None);
}

#[test]
fn test_global_search_finds_dialogue_and_choice_content() {
    let mut graph = NodeGraph::new();
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrador".to_string(),
            text: "Bienvenido al castillo".to_string(),
        },
        pos(0.0, 0.0),
    );
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Ruta".to_string(),
            options: vec!["Bosque".to_string(), "Castillo".to_string()],
        },
        pos(0.0, 100.0),
    );

    let hits = graph.search_nodes("castillo");
    assert!(hits.contains(&dialogue));
    assert!(hits.contains(&choice));
}

#[test]
fn global_search_correctness() {
    let mut graph = NodeGraph::new();
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Puerta secreta".to_string(),
        },
        pos(0.0, 0.0),
    );
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/secret_room.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        pos(0.0, 100.0),
    );

    let hits = graph.search_nodes("secret");
    assert!(hits.contains(&dialogue));
    assert!(hits.contains(&scene));
}

#[test]
fn bookmark_navigation() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: "N".to_string(),
            text: "Hola".to_string(),
        },
        pos(0.0, 100.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 200.0));
    graph.connect(start, dialogue);
    graph.connect(dialogue, end);

    assert!(graph.set_bookmark("intro", dialogue));
    let bookmarked = graph
        .bookmarked_node("intro")
        .expect("bookmark should resolve");
    assert_eq!(bookmarked, dialogue);
    assert_eq!(graph.incoming_nodes(dialogue), vec![start]);
    assert_eq!(graph.outgoing_nodes(dialogue), vec![end]);
}

#[test]
fn test_node_for_event_ip_and_asset_reference_navigation() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: Some("music/theme.ogg".to_string()),
            characters: vec![visual_novel_engine::CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("sprites/ava_smile.png".to_string()),
                position: Some("left".to_string()),
                x: None,
                y: None,
                scale: None,
            }],
        },
        pos(0.0, 100.0),
    );
    let audio = graph.add_node(
        StoryNode::AudioAction {
            channel: "bgm".to_string(),
            action: "play".to_string(),
            asset: Some("music/theme.ogg".to_string()),
            volume: Some(1.0),
            fade_duration_ms: Some(150),
            loop_playback: Some(true),
        },
        pos(0.0, 200.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 300.0));
    graph.connect(start, scene);
    graph.connect(scene, audio);
    graph.connect(audio, end);

    assert_eq!(graph.node_for_event_ip(0), Some(scene));
    assert_eq!(graph.node_for_event_ip(1), Some(audio));
    assert_eq!(graph.node_for_event_ip(2), None);
    assert_eq!(graph.event_ip_for_node(scene), Some(0));
    assert_eq!(graph.event_ip_for_node(audio), Some(1));
    assert_eq!(graph.event_ip_for_node(start), None);

    let refs = graph.nodes_referencing_asset("music/theme.ogg");
    assert!(refs.contains(&scene));
    assert!(refs.contains(&audio));
    assert_eq!(
        graph.first_node_referencing_asset("sprites/ava_smile.png"),
        Some(scene)
    );
}

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

    let start_pos = graph
        .nodes()
        .find(|(id, _, _)| *id == start)
        .map(|(_, _, pos)| *pos)
        .expect("start pos");
    let choice_pos = graph
        .nodes()
        .find(|(id, _, _)| *id == choice)
        .map(|(_, _, pos)| *pos)
        .expect("choice pos");
    let branch_a_pos = graph
        .nodes()
        .find(|(id, _, _)| *id == branch_a)
        .map(|(_, _, pos)| *pos)
        .expect("branch a pos");
    let branch_b_pos = graph
        .nodes()
        .find(|(id, _, _)| *id == branch_b)
        .map(|(_, _, pos)| *pos)
        .expect("branch b pos");

    assert!(
        choice_pos.y > start_pos.y,
        "choice should be on a deeper Y layer"
    );
    assert!(
        branch_a_pos.y > choice_pos.y && branch_b_pos.y > choice_pos.y,
        "branches should be below choice in vertical flow"
    );
    assert_ne!(
        branch_a_pos.x, branch_b_pos.x,
        "parallel branch nodes must not overlap horizontally"
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
        unique_x.len() > 2,
        "long linear scripts should be wrapped into multiple columns/offsets"
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
                *node_pos,
                egui::vec2(
                    NODE_WIDTH,
                    crate::editor::node_types::node_visual_height(node),
                ),
            );
            (*id, rect)
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
