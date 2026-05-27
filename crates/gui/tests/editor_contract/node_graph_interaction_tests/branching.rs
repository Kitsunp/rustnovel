use super::*;

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
    let mut engine = visual_novel_engine::runtime::Engine::new(
        script,
        visual_novel_engine::SecurityPolicy::default(),
        visual_novel_engine::ResourceLimiter::default(),
    )
    .expect("engine should initialize");
    let event = engine.current_event().expect("choice should be current");
    assert!(matches!(
        event,
        visual_novel_engine::runtime::EventCompiled::Choice(choice) if choice.options.len() == 3
    ));

    engine.choose(2).expect("new route should be selectable");
    let event = engine.current_event().expect("scene should be current");
    assert!(matches!(
        event,
        visual_novel_engine::runtime::EventCompiled::Scene(scene)
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
