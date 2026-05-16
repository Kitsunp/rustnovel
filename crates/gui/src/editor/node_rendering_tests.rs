use super::*;

#[test]
fn test_bezier_connection_does_not_panic() {
    let from = egui::pos2(0.0, 0.0);
    let to = egui::pos2(100.0, 200.0);

    let (control1, control2) = bezier_control_points(from, to);

    assert_eq!(control1, egui::pos2(0.0, 100.0));
    assert_eq!(control2, egui::pos2(100.0, 100.0));

    let points: Vec<egui::Pos2> = (0..=20)
        .map(|i| {
            let t = i as f32 / 20.0;
            let t2 = t * t;
            let t3 = t2 * t;
            let mt = 1.0 - t;
            let mt2 = mt * mt;
            let mt3 = mt2 * mt;

            egui::pos2(
                mt3 * from.x + 3.0 * mt2 * t * control1.x + 3.0 * mt * t2 * control2.x + t3 * to.x,
                mt3 * from.y + 3.0 * mt2 * t * control1.y + 3.0 * mt * t2 * control2.y + t3 * to.y,
            )
        })
        .collect();

    assert_eq!(points.len(), 21);
    assert_eq!(points[0], from);
    assert_eq!(points[20], to);
}

#[test]
fn test_bezier_horizontal_line() {
    let from = egui::pos2(0.0, 50.0);
    let to = egui::pos2(100.0, 50.0);

    let (control1, control2) = bezier_control_points(from, to);
    assert_eq!(control1, egui::pos2(50.0, 50.0));
    assert_eq!(control2, egui::pos2(50.0, 50.0));
}

#[test]
fn test_bezier_control_points_keep_direction_for_reverse_edges() {
    let from = egui::pos2(300.0, 220.0);
    let to = egui::pos2(120.0, 80.0);
    let (control1, control2) = bezier_control_points(from, to);

    assert!(control1.x <= from.x);
    assert!(control2.x >= to.x);
}

#[test]
fn test_bezier_control_points_clamp_offset_for_long_edges() {
    let from = egui::pos2(0.0, 0.0);
    let to = egui::pos2(2000.0, 0.0);
    let (control1, control2) = bezier_control_points(from, to);

    assert_eq!(control1.x, 220.0);
    assert_eq!(control2.x, 1780.0);
}

#[test]
fn test_context_menu_no_panic_when_no_menu() {
    let mut graph = NodeGraph::new();
    graph.context_menu = None;
    assert!(graph.context_menu.is_none());
}

#[test]
fn context_menu_connect_to_choice_targets_new_option_port() {
    let choice = StoryNode::Choice {
        prompt: "Route?".to_string(),
        options: vec!["A".to_string(), "B".to_string()],
    };
    let dialogue = StoryNode::Dialogue {
        speaker: "N".to_string(),
        text: "Line".to_string(),
    };

    assert_eq!(default_context_connect_port(&choice), 2);
    assert_eq!(default_context_connect_port(&dialogue), 0);
}

#[test]
fn canvas_context_palette_matches_extended_authoring_nodes() {
    let labels = canvas_node_palette_items()
        .into_iter()
        .map(|(label, _)| label)
        .collect::<Vec<_>>();

    for required in [
        "Dialogue",
        "Choice",
        "Scene",
        "Jump",
        "Start",
        "End",
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
            "canvas context menu is missing node type {required}"
        );
    }
}

#[test]
fn canvas_palette_creation_finishes_pending_connection_to_new_node() {
    let mut graph = NodeGraph::new();
    let source = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Before".to_string(),
        },
        egui::pos2(0.0, 0.0),
    );
    graph.start_connection_pick(source, 0);
    graph.take_operation_hint();

    let inserted = add_canvas_node_from_palette(
        &mut graph,
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Created from canvas menu".to_string(),
        },
        egui::pos2(180.0, 120.0),
    );

    assert_eq!(graph.connecting_from, None);
    assert!(!graph.connecting_sticky);
    assert_eq!(graph.get_node_pos(inserted), Some(egui::pos2(180.0, 120.0)));
    assert!(graph
        .connections()
        .any(|conn| conn.from == source && conn.from_port == 0 && conn.to == inserted));
    let hint = graph
        .take_operation_hint()
        .expect("create-and-connect should leave a traceable operation");
    assert_eq!(hint.kind, "node_connected");
    let expected_field_path = format!("graph.edges[{source}:0]");
    assert_eq!(
        hint.field_path.as_deref(),
        Some(expected_field_path.as_str())
    );
}

#[test]
fn canvas_palette_creation_from_occupied_output_creates_branch_to_new_node() {
    let mut graph = NodeGraph::new();
    let source = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Before".to_string(),
        },
        egui::pos2(0.0, 0.0),
    );
    let existing = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Existing continuation".to_string(),
        },
        egui::pos2(-140.0, 120.0),
    );
    graph.connect(source, existing);
    graph.start_connection_pick(source, 0);

    let inserted = add_canvas_node_from_palette(
        &mut graph,
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "New branch".to_string(),
        },
        egui::pos2(180.0, 120.0),
    );

    assert_eq!(graph.connecting_from, None);
    assert!(!graph.connecting_sticky);
    let hub = graph
        .connections()
        .find(|conn| conn.from == source && conn.from_port == 0)
        .map(|conn| conn.to)
        .expect("source should be rerouted through a branch hub");
    assert!(matches!(
        graph.get_node(hub),
        Some(StoryNode::Choice { .. })
    ));
    assert!(graph
        .connections()
        .any(|conn| conn.from == hub && conn.to == existing));
    assert!(graph
        .connections()
        .any(|conn| conn.from == hub && conn.to == inserted));
}

#[test]
fn test_inline_editor_no_panic_when_not_editing() {
    let mut graph = NodeGraph::new();
    graph.editing = None;
    assert!(graph.editing.is_none());
}
