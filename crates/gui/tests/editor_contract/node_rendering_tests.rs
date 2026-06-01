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
fn route_labels_identify_choice_options_by_port() {
    let choice = StoryNode::Choice {
        prompt: "Where?".to_string(),
        options: vec![
            "Visit the court".to_string(),
            "Find the music".to_string(),
        ],
    };
    let dialogue = StoryNode::Dialogue {
        speaker: "Sakura".to_string(),
        text: "Hello".to_string(),
    };

    assert_eq!(
        route_label_for_source(&choice, 0).as_deref(),
        Some("1. Visit the court")
    );
    assert_eq!(
        route_label_for_source(&choice, 1).as_deref(),
        Some("2. Find the music")
    );
    assert_eq!(route_label_for_source(&choice, 2), None);
    assert_eq!(route_label_for_source(&dialogue, 0), None);
}

#[test]
fn route_colors_distinguish_adjacent_option_ports() {
    assert_ne!(route_color(0), route_color(1));
    assert_ne!(route_color(1), route_color(2));
    assert_eq!(route_color(0), route_color(6));
}

#[test]
fn bezier_point_clamps_label_position_to_curve() {
    let from = egui::pos2(0.0, 50.0);
    let to = egui::pos2(100.0, 50.0);

    assert_eq!(bezier_point(from, to, -1.0), from);
    assert_eq!(bezier_point(from, to, 2.0), to);
    assert_eq!(bezier_point(from, to, 0.5), egui::pos2(50.0, 50.0));
}

#[test]
fn route_labels_do_not_render_at_zoom_levels_that_crowd_node_options() {
    let from = egui::pos2(220.0, 180.0);
    let to = egui::pos2(360.0, 210.0);

    assert_eq!(route_label_rect(from, to, "1. Visit the court", 0.48), None);
    assert!(route_label_rect(from, to, "1. Visit the court", 0.55).is_some());
}

#[test]
fn route_label_rect_stays_outside_source_rect_for_vertical_and_horizontal_edges() {
    let source = egui::Rect::from_min_size(egui::pos2(100.0, 120.0), egui::vec2(372.0, 88.0));
    let vertical_label = route_label_rect(
        egui::pos2(162.0, source.bottom()),
        egui::pos2(180.0, 300.0),
        "1. Visit the court",
        0.55,
    )
    .expect("vertical label");
    let horizontal_label = route_label_rect(
        egui::pos2(source.right(), 168.0),
        egui::pos2(640.0, 160.0),
        "2. Find the music room",
        0.55,
    )
    .expect("horizontal label");

    assert!(
        !source.intersects(vertical_label),
        "vertical route labels should not cover the Choice node"
    );
    assert!(
        !source.intersects(horizontal_label),
        "horizontal route labels should not cover the Choice node"
    );
}

#[test]
fn choice_route_labels_do_not_overlap_each_other_in_vertical_or_horizontal_flow() {
    let choice = StoryNode::Choice {
        prompt: "Route?".to_string(),
        options: vec![
            "Visit the courtyard".to_string(),
            "Find the music room".to_string(),
            "Ask about the locked hall".to_string(),
        ],
    };
    let zoom = 0.55;

    let vertical_a = route_label_rect_for_source(
        egui::pos2(162.0, 208.0),
        egui::pos2(165.0, 330.0),
        "1. Visit the courtyard",
        zoom,
        &choice,
        0,
    )
    .expect("vertical route label 1");
    let vertical_b = route_label_rect_for_source(
        egui::pos2(286.0, 208.0),
        egui::pos2(286.0, 330.0),
        "2. Find the music room",
        zoom,
        &choice,
        1,
    )
    .expect("vertical route label 2");

    assert!(
        !vertical_a.intersects(vertical_b),
        "choice option labels should stay in separate vertical lanes"
    );

    let horizontal_a = route_label_rect_for_source(
        egui::pos2(472.0, 154.0),
        egui::pos2(650.0, 152.0),
        "1. Visit the courtyard",
        zoom,
        &choice,
        0,
    )
    .expect("horizontal route label 1");
    let horizontal_b = route_label_rect_for_source(
        egui::pos2(472.0, 170.0),
        egui::pos2(650.0, 172.0),
        "2. Find the music room",
        zoom,
        &choice,
        1,
    )
    .expect("horizontal route label 2");
    let horizontal_c = route_label_rect_for_source(
        egui::pos2(472.0, 186.0),
        egui::pos2(650.0, 190.0),
        "3. Ask about the locked hall",
        zoom,
        &choice,
        2,
    )
    .expect("horizontal route label 3");

    assert!(
        !horizontal_a.intersects(horizontal_b),
        "choice option labels should stack into separate horizontal lanes"
    );
    assert!(
        !horizontal_b.intersects(horizontal_c),
        "choice option labels should stack into separate horizontal lanes"
    );
}

#[test]
fn connection_viewport_culling_tracks_screen_space_after_pan() {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 240.0));

    assert!(connection_intersects_viewport(
        egui::pos2(20.0, 20.0),
        egui::pos2(260.0, 180.0),
        viewport,
        1.0
    ));
    assert!(!connection_intersects_viewport(
        egui::pos2(900.0, 900.0),
        egui::pos2(1100.0, 920.0),
        viewport,
        1.0
    ));

    let shifted_from = egui::pos2(20.0, 20.0) + egui::vec2(40.0, 30.0);
    let shifted_to = egui::pos2(260.0, 180.0) + egui::vec2(40.0, 30.0);
    assert_eq!(
        connection_bounds(egui::pos2(20.0, 20.0), egui::pos2(260.0, 180.0), 1.0).size(),
        connection_bounds(shifted_from, shifted_to, 1.0).size(),
        "panning should translate a connection without changing its shape bounds"
    );
}

#[test]
fn projected_bezier_curve_is_stable_when_camera_pans() {
    let from = egui::pos2(80.0, 90.0);
    let to = egui::pos2(360.0, 220.0);
    let pan_a = egui::vec2(0.0, 0.0);
    let pan_b = egui::vec2(-180.0, 75.0);
    let zoom = 1.35;
    let project_a =
        |point: egui::Pos2| egui::pos2((point.x + pan_a.x) * zoom, (point.y + pan_a.y) * zoom);
    let project_b =
        |point: egui::Pos2| egui::pos2((point.x + pan_b.x) * zoom, (point.y + pan_b.y) * zoom);
    let a = bezier_curve_points(from, to)
        .into_iter()
        .map(project_a)
        .collect::<Vec<_>>();
    let b = bezier_curve_points(from, to)
        .into_iter()
        .map(project_b)
        .collect::<Vec<_>>();
    let expected_delta = (pan_b - pan_a) * zoom;

    assert_eq!(a.len(), b.len());
    for (left, right) in a.iter().zip(b.iter()) {
        let delta = *right - *left;
        assert!(
            (delta - expected_delta).length() < 0.01,
            "camera pan should translate the rendered curve without rerouting it: {delta:?}"
        );
    }
}

#[test]
fn context_menu_absent_renders_real_frame_without_mutating_graph() {
    let mut graph = NodeGraph::new();
    let node_id = graph.add_node(StoryNode::Start, egui::pos2(80.0, 80.0));
    graph.context_menu = None;
    let ctx = egui::Context::default();
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(320.0, 240.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_context_menu(&mut graph, ui);
            });
        },
    );

    assert!(graph.context_menu.is_none());
    assert_eq!(graph.get_node(node_id), Some(&StoryNode::Start));
    assert!(
        output.shapes.len() <= 1,
        "absent context menu should not paint menu chrome, shapes={}",
        output.shapes.len()
    );
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
fn context_menu_position_is_clamped_inside_visible_canvas() {
    let bounds = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(320.0, 240.0));
    let requested = egui::pos2(310.0, 230.0);
    let size = egui::vec2(160.0, 120.0);
    let pos = clamped_context_menu_position(requested, bounds, size);

    assert!(pos.x >= bounds.left());
    assert!(pos.y >= bounds.top());
    assert!(pos.x + size.x <= bounds.right() + 0.1);
    assert!(pos.y + size.y <= bounds.bottom() + 0.1);
}

#[test]
fn context_menu_layout_caps_width_and_height_to_viewport() {
    let bounds = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(240.0, 190.0));
    let layout = context_menu_layout(bounds, canvas_context_menu_size());

    assert!(layout.width <= bounds.width() - 16.0 + 0.1);
    assert!(layout.max_height <= bounds.height() - 16.0 + 0.1);
    assert!(layout.item_width() < layout.width);
    assert!(layout.list_max_height < layout.max_height);
}

#[test]
fn context_menu_palette_groups_are_stable() {
    assert_eq!(canvas_palette_section("Dialogue"), "Basic");
    assert_eq!(canvas_palette_section("Branch If"), "Logic");
    assert_eq!(canvas_palette_section("Audio"), "Media and advanced");
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
fn every_canvas_palette_option_creates_a_distinct_traceable_node() {
    let items = canvas_node_palette_items();
    let mut labels = std::collections::BTreeSet::new();
    let mut graph = NodeGraph::new();

    for (idx, (label, node)) in items.into_iter().enumerate() {
        assert!(labels.insert(label), "duplicate canvas palette label {label}");
        assert_ne!(
            canvas_palette_section(label),
            "",
            "palette option {label} must belong to a visible section"
        );

        let expected = std::mem::discriminant(&node);
        let pos = egui::pos2(idx as f32 * 32.0, idx as f32 * 24.0);
        let inserted = add_canvas_node_from_palette(&mut graph, node, pos);
        let created = graph
            .get_node(inserted)
            .unwrap_or_else(|| panic!("palette option {label} did not create a node"));

        assert_eq!(
            std::mem::discriminant(created),
            expected,
            "palette option {label} created the wrong node type"
        );
        assert_eq!(
            graph.get_node_pos(inserted),
            Some(pos),
            "palette option {label} did not preserve insertion position"
        );
        let hint = graph
            .take_operation_hint()
            .unwrap_or_else(|| panic!("palette option {label} did not leave an operation hint"));
        assert_eq!(hint.kind, "node_created");
        assert!(
            hint.details.contains(label) || hint.details.contains("Created"),
            "palette option {label} left an unhelpful operation hint: {:?}",
            hint.details
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
