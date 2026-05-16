use super::*;

fn pos(x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(x, y)
}

#[test]
fn connect_or_branch_creates_explicit_choice_hub_for_second_output() {
    let mut graph = NodeGraph::new();
    let source = graph.add_node(
        StoryNode::Dialogue {
            speaker: "N".to_string(),
            text: "Go".to_string(),
        },
        pos(0.0, 0.0),
    );
    let existing = graph.add_node(StoryNode::End, pos(-120.0, 200.0));
    let extra = graph.add_node(StoryNode::End, pos(120.0, 200.0));

    assert!(graph.connect_or_branch(source, 0, existing));
    assert!(graph.connect_or_branch(source, 0, extra));

    let hub = graph
        .connections()
        .find(|conn| conn.from == source && conn.from_port == 0)
        .map(|conn| conn.to)
        .expect("source should point to generated choice hub");
    assert!(matches!(
        graph.get_node(hub),
        Some(StoryNode::Choice { .. })
    ));
    assert!(graph
        .connections()
        .any(|conn| conn.from == hub && conn.from_port == 0 && conn.to == existing));
    assert!(graph
        .connections()
        .any(|conn| conn.from == hub && conn.from_port == 1 && conn.to == extra));
}

#[test]
fn connect_or_branch_reuses_existing_choice_hub() {
    let mut graph = NodeGraph::new();
    let source = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Route?".to_string(),
            options: vec!["A".to_string()],
        },
        pos(0.0, 100.0),
    );
    let first = graph.add_node(StoryNode::End, pos(-120.0, 220.0));
    let second = graph.add_node(StoryNode::End, pos(120.0, 220.0));

    graph.connect(source, choice);
    graph.connect_port(choice, 0, first);
    assert!(graph.connect_or_branch(source, 0, second));

    let Some(StoryNode::Choice { options, .. }) = graph.get_node(choice) else {
        panic!("choice should remain present");
    };
    assert_eq!(options.len(), 2);
    assert!(graph
        .connections()
        .any(|conn| conn.from == choice && conn.from_port == 1 && conn.to == second));
}

#[test]
fn connect_or_branch_to_existing_choice_reuses_target_hub_without_nested_choice() {
    let mut graph = NodeGraph::new();
    let source = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/classroom.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        pos(0.0, 0.0),
    );
    let previous = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Existing continuation".to_string(),
        },
        pos(-120.0, 150.0),
    );
    let target_choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Where next?".to_string(),
            options: vec!["Left".to_string()],
        },
        pos(120.0, 150.0),
    );
    let left = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Left branch".to_string(),
        },
        pos(120.0, 280.0),
    );
    graph.connect(source, previous);
    graph.connect_port(target_choice, 0, left);

    assert!(graph.connect_or_branch(source, 0, target_choice));

    let choice_nodes = graph
        .nodes()
        .filter(|(_, node, _)| matches!(node, StoryNode::Choice { .. }))
        .count();
    assert_eq!(
        choice_nodes, 1,
        "connecting to an existing choice must not nest another choice"
    );
    assert!(graph
        .connections()
        .any(|conn| conn.from == source && conn.from_port == 0 && conn.to == target_choice));
    assert!(graph
        .connections()
        .any(|conn| conn.from == target_choice && conn.to == previous));
    let Some(StoryNode::Choice { options, .. }) = graph.get_node(target_choice) else {
        panic!("target choice should remain a choice");
    };
    assert_eq!(options, &vec!["Left".to_string(), "Continue".to_string()]);
}

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

#[test]
fn connect_or_branch_from_choice_new_option_uses_real_route_label() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Route?".to_string(),
            options: vec!["A".to_string()],
        },
        pos(0.0, 90.0),
    );
    let first = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "First".to_string(),
        },
        pos(-120.0, 180.0),
    );
    let second = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Second".to_string(),
        },
        pos(120.0, 180.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 270.0));

    graph.connect(start, choice);
    graph.connect_port(choice, 0, first);
    graph.connect(first, end);
    graph.connect(second, end);
    assert!(graph.connect_or_branch(choice, 1, second));

    let Some(StoryNode::Choice { options, .. }) = graph.get_node(choice) else {
        panic!("choice node should remain");
    };
    assert_eq!(options, &vec!["A".to_string(), "New route".to_string()]);
    graph
        .authoring_graph()
        .to_script_strict()
        .expect("new choice route should be strict-exportable");
}

#[test]
fn connect_or_branch_same_choice_route_is_noop_without_operation_hint() {
    let mut graph = NodeGraph::new();
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Route?".to_string(),
            options: vec!["A".to_string()],
        },
        pos(0.0, 0.0),
    );
    let target = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Same route".to_string(),
        },
        pos(0.0, 140.0),
    );
    graph.connect_port(choice, 0, target);
    graph.take_operation_hint();
    graph.clear_modified();

    assert!(
        !graph.connect_or_branch(choice, 0, target),
        "reconnecting an already-routed choice option should be a no-op"
    );
    assert_eq!(graph.connection_count(), 1);
    assert!(
        graph.take_operation_hint().is_none(),
        "no-op reconnects must not create false operation-log entries"
    );
    assert!(
        !graph.is_modified(),
        "no-op reconnects must not dirty the editor graph"
    );
}

#[test]
fn insert_after_choice_adds_route_without_overwriting_existing_options() {
    let mut graph = NodeGraph::new();
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Route?".to_string(),
            options: vec!["A".to_string()],
        },
        pos(0.0, 0.0),
    );
    let first = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "First".to_string(),
        },
        pos(-120.0, 120.0),
    );
    graph.connect_port(choice, 0, first);

    graph.insert_after(
        choice,
        StoryNode::Scene {
            profile: None,
            background: Some("bg/classroom.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
    );

    let Some(StoryNode::Choice { options, .. }) = graph.get_node(choice) else {
        panic!("choice should still be a choice");
    };
    assert_eq!(
        options,
        &vec!["A".to_string(), "Go to classroom".to_string()]
    );
    assert!(graph
        .connections()
        .any(|conn| conn.from == choice && conn.from_port == 0 && conn.to == first));
    let new_scene = graph
        .nodes()
        .find_map(|(id, node, _)| matches!(node, StoryNode::Scene { .. }).then_some(id))
        .expect("inserted scene should exist");
    assert!(graph
        .connections()
        .any(|conn| conn.from == choice && conn.from_port == 1 && conn.to == new_scene));
}

#[test]
fn insert_after_choice_route_exports_and_runs_to_inserted_scene() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Route?".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
        },
        pos(0.0, 90.0),
    );
    let first = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "First".to_string(),
        },
        pos(-120.0, 180.0),
    );
    let second = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Second".to_string(),
        },
        pos(120.0, 180.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 300.0));
    graph.connect(start, choice);
    graph.connect_port(choice, 0, first);
    graph.connect_port(choice, 1, second);
    graph.connect(first, end);
    graph.connect(second, end);

    graph.insert_after(
        choice,
        StoryNode::Scene {
            profile: None,
            background: Some("assets/backgrounds/classroom.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
    );

    let script = graph
        .authoring_graph()
        .to_script_strict()
        .expect("inserted choice route must be strict-exportable");
    let mut engine = visual_novel_engine::Engine::new(
        script,
        visual_novel_engine::SecurityPolicy::default(),
        visual_novel_engine::ResourceLimiter::default(),
    )
    .expect("engine should initialize");
    let event = engine.current_event().expect("choice should be current");
    assert!(matches!(
        event,
        visual_novel_engine::EventCompiled::Choice(choice) if choice.options.len() == 3
    ));

    engine.choose(2).expect("new route should be selectable");
    let event = engine.current_event().expect("scene should be current");
    assert!(matches!(
        event,
        visual_novel_engine::EventCompiled::Scene(scene)
            if scene.background.as_deref() == Some("assets/backgrounds/classroom.png")
    ));
}

#[test]
fn create_branch_reuses_existing_choice_continuation_without_nesting() {
    let mut graph = NodeGraph::new();
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/classroom.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        pos(0.0, 0.0),
    );
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Where now?".to_string(),
            options: vec!["Stay".to_string(), "Leave".to_string()],
        },
        pos(0.0, 140.0),
    );
    let stay = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "We stay.".to_string(),
        },
        pos(-120.0, 280.0),
    );
    let leave = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "We leave.".to_string(),
        },
        pos(120.0, 280.0),
    );
    graph.connect(scene, choice);
    graph.connect_port(choice, 0, stay);
    graph.connect_port(choice, 1, leave);
    let before_nodes = graph.len();
    let before_edges = graph.connection_count();

    graph.create_branch(scene);

    assert_eq!(
        graph.len(),
        before_nodes,
        "Create Branch should focus the existing choice continuation instead of adding nodes"
    );
    assert_eq!(graph.connection_count(), before_edges);
    assert_eq!(graph.selected, Some(choice));
    assert_eq!(
        graph
            .nodes()
            .filter(|(_, node, _)| matches!(node, StoryNode::Choice { .. }))
            .count(),
        1,
        "scene -> choice must not become scene -> generated choice -> existing choice"
    );
    assert!(graph
        .connections()
        .any(|conn| conn.from == scene && conn.from_port == 0 && conn.to == choice));
    let Some(StoryNode::Choice { options, .. }) = graph.get_node(choice) else {
        panic!("choice should still be editable");
    };
    assert_eq!(options, &vec!["Stay".to_string(), "Leave".to_string()]);
}

#[test]
fn create_branch_on_choice_adds_route_to_same_choice_without_nested_choice() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Where now?".to_string(),
            options: vec!["Stay".to_string(), "Leave".to_string()],
        },
        pos(0.0, 120.0),
    );
    let stay = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "We stay.".to_string(),
        },
        pos(-120.0, 260.0),
    );
    let leave = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "We leave.".to_string(),
        },
        pos(120.0, 260.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 420.0));
    graph.connect(start, choice);
    graph.connect_port(choice, 0, stay);
    graph.connect_port(choice, 1, leave);
    graph.connect(stay, end);
    graph.connect(leave, end);

    graph.create_branch(choice);

    assert_eq!(
        graph
            .nodes()
            .filter(|(_, node, _)| matches!(node, StoryNode::Choice { .. }))
            .count(),
        1
    );
    let Some(StoryNode::Choice { options, .. }) = graph.get_node(choice) else {
        panic!("choice should remain the route hub");
    };
    assert_eq!(options.len(), 3);
    assert_eq!(options[2], "New route");
    let new_route = graph
        .connections()
        .find(|conn| conn.from == choice && conn.from_port == 2)
        .map(|conn| conn.to)
        .expect("new option should point to a real route node");
    assert!(matches!(
        graph.get_node(new_route),
        Some(StoryNode::Dialogue { speaker, .. }) if speaker == "New Route"
    ));
    assert_eq!(graph.selected, Some(choice));
    graph
        .authoring_graph()
        .to_script_strict()
        .expect("expanded choice must stay strict-exportable");
}
