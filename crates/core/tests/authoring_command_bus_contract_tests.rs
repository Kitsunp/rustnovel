use visual_novel_engine::authoring::*;
use visual_novel_engine::runtime::CharacterPlacementRaw;

fn pos(x: f32, y: f32) -> AuthoringPosition {
    AuthoringPosition::new(x, y)
}

fn character(name: &str, image: &str) -> CharacterPlacementRaw {
    CharacterPlacementRaw {
        name: name.to_string(),
        expression: Some(image.to_string()),
        ..Default::default()
    }
}

#[test]
fn command_bus_replay_headless() {
    let commands = vec![
        AuthoringCommand::CreateNode {
            node_id: 0,
            node: StoryNode::Start,
            position: pos(0.0, 0.0),
        },
        AuthoringCommand::CreateNode {
            node_id: 1,
            node: StoryNode::Dialogue {
                speaker: "Narrator".to_string(),
                text: "Replay".to_string(),
            },
            position: pos(0.0, 90.0),
        },
        AuthoringCommand::CreateNode {
            node_id: 2,
            node: StoryNode::End,
            position: pos(0.0, 180.0),
        },
        AuthoringCommand::Connect {
            from: 0,
            from_port: 0,
            to: 1,
        },
        AuthoringCommand::Connect {
            from: 1,
            from_port: 0,
            to: 2,
        },
    ];

    let bus = AuthoringCommandBus::replay(&commands).expect("replay commands");
    let script = bus.graph().to_script_strict().expect("strict script");
    script.compile().expect("compiled replay script");
    assert_eq!(bus.operation_log().len(), commands.len());
    assert_eq!(bus.verification_runs().len(), commands.len());
}

#[test]
fn undo_delta_memory_contract() {
    let mut bus = AuthoringCommandBus::new(NodeGraph::new());
    for node_id in 0..1000 {
        bus.apply(AuthoringCommand::CreateNode {
            node_id,
            node: if node_id == 0 {
                StoryNode::Start
            } else {
                StoryNode::Dialogue {
                    speaker: "Narrator".to_string(),
                    text: format!("Line {node_id}"),
                }
            },
            position: pos(0.0, node_id as f32),
        })
        .expect("create node");
    }
    let created_delta_count = bus.undo_delta_count();

    for idx in 0..50 {
        bus.apply(AuthoringCommand::EditNode {
            node_id: 1,
            replacement: StoryNode::Dialogue {
                speaker: "Narrator".to_string(),
                text: format!("Edited {idx}"),
            },
        })
        .expect("edit node");
    }

    assert_eq!(bus.graph().len(), 1000);
    assert_eq!(bus.undo_delta_count(), created_delta_count + 50);
    assert_eq!(bus.redo_delta_count(), 0);
}

#[test]
fn operation_log_replay_create_connect_edit_import_move_revert() {
    let mut bus = AuthoringCommandBus::new(NodeGraph::new());
    bus.apply(AuthoringCommand::CreateNode {
        node_id: 0,
        node: StoryNode::Start,
        position: pos(0.0, 0.0),
    })
    .expect("create start");
    bus.apply(AuthoringCommand::CreateNode {
        node_id: 1,
        node: StoryNode::Scene {
            profile: None,
            background: Some("assets/bg.png".to_string()),
            music: None,
            characters: vec![character("Ava", "assets/ava.png")],
        },
        position: pos(0.0, 90.0),
    })
    .expect("create scene");
    bus.apply(AuthoringCommand::CreateNode {
        node_id: 2,
        node: StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Before".to_string(),
        },
        position: pos(0.0, 180.0),
    })
    .expect("create dialogue");
    bus.apply(AuthoringCommand::Connect {
        from: 0,
        from_port: 0,
        to: 1,
    })
    .expect("connect start");
    bus.apply(AuthoringCommand::Connect {
        from: 1,
        from_port: 0,
        to: 2,
    })
    .expect("connect scene");
    bus.apply(AuthoringCommand::EditNode {
        node_id: 2,
        replacement: StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "After".to_string(),
        },
    })
    .expect("edit field");
    bus.apply(AuthoringCommand::ImportAsset {
        path: "assets/bg.png".to_string(),
    })
    .expect("import asset");

    let object_id = composer::list_layered_objects(bus.graph(), Some(1))
        .into_iter()
        .find(|object| object.character_name.as_deref() == Some("Ava"))
        .expect("character layer")
        .object_id;
    bus.apply(AuthoringCommand::MoveLayer {
        object_id,
        x: 640,
        y: 360,
        scale: Some(1.2),
    })
    .expect("move layer");
    bus.apply(AuthoringCommand::RevertLast)
        .expect("revert layer move");

    assert!(bus
        .operation_log()
        .iter()
        .any(|entry| entry.operation_kind == "asset_imported"));
    assert_eq!(
        bus.operation_log()
            .last()
            .and_then(|entry| entry.operation_kind_v2.as_ref()),
        Some(&OperationKind::Revert)
    );
    assert_eq!(bus.operation_log().len(), bus.verification_runs().len());

    let replay = AuthoringCommandBus::replay(bus.recorded_commands()).expect("replay log commands");
    assert_eq!(replay.graph().len(), bus.graph().len());
}

#[test]
fn command_bus_covers_fragment_choice_target_and_remove_node_mutations() {
    let mut bus = AuthoringCommandBus::new(NodeGraph::new());
    for (node_id, node) in [
        (0, StoryNode::Start),
        (
            1,
            StoryNode::Choice {
                prompt: "Route?".to_string(),
                options: vec!["A".to_string()],
            },
        ),
        (
            2,
            StoryNode::Dialogue {
                speaker: "A".to_string(),
                text: "Route A".to_string(),
            },
        ),
        (3, StoryNode::End),
    ] {
        bus.apply(AuthoringCommand::CreateNode {
            node_id,
            node,
            position: pos(0.0, node_id as f32 * 90.0),
        })
        .expect("create node");
    }
    bus.apply(AuthoringCommand::Connect {
        from: 0,
        from_port: 0,
        to: 1,
    })
    .expect("connect start");
    let outcome = bus
        .apply(AuthoringCommand::ConnectNewChoiceOption {
            choice_id: 1,
            to: 2,
            text: "B".to_string(),
        })
        .expect("connect new choice option");
    assert!(matches!(
        outcome.delta,
        AuthoringDelta::ChoiceOptionConnected {
            option_index: 1,
            ..
        }
    ));
    bus.apply(AuthoringCommand::SetChoiceOptionTarget {
        node_id: 1,
        option_index: 0,
        target_node_id: Some(3),
    })
    .expect("set choice target");
    bus.apply(AuthoringCommand::CreateFragment {
        fragment_id: "route".to_string(),
        title: "Route".to_string(),
        node_ids: vec![2],
    })
    .expect("create fragment");
    bus.apply(AuthoringCommand::RefreshFragmentPorts {
        fragment_id: "route".to_string(),
    })
    .expect_err("unchanged fragment ports should not create a fake operation");
    bus.apply(AuthoringCommand::RemoveNode { node_id: 2 })
        .expect("remove node through bus");

    assert!(bus.graph().get_node(2).is_none());
    assert!(bus
        .operation_log()
        .iter()
        .any(|entry| entry.operation_kind_v2.as_ref() == Some(&OperationKind::FragmentCreated)));
    assert!(bus
        .operation_log()
        .iter()
        .any(|entry| entry.operation_kind_v2.as_ref() == Some(&OperationKind::NodeRemoved)));
    assert_eq!(bus.operation_log().len(), bus.verification_runs().len());
}

#[test]
fn command_bus_covers_python_semantic_edits_and_fragment_navigation() {
    let mut bus = AuthoringCommandBus::new(NodeGraph::new());
    for (node_id, node) in [
        (
            0,
            StoryNode::Choice {
                prompt: "Old prompt".to_string(),
                options: vec!["Left".to_string(), "Right".to_string()],
            },
        ),
        (
            1,
            StoryNode::Dialogue {
                speaker: "A".to_string(),
                text: "Left route".to_string(),
            },
        ),
        (
            2,
            StoryNode::Dialogue {
                speaker: "B".to_string(),
                text: "Right route".to_string(),
            },
        ),
    ] {
        bus.apply(AuthoringCommand::CreateNode {
            node_id,
            node,
            position: pos(0.0, node_id as f32 * 90.0),
        })
        .expect("create node");
    }
    bus.apply(AuthoringCommand::Connect {
        from: 0,
        from_port: 0,
        to: 1,
    })
    .expect("connect left");
    bus.apply(AuthoringCommand::Connect {
        from: 0,
        from_port: 1,
        to: 2,
    })
    .expect("connect right");

    bus.apply(AuthoringCommand::EditChoicePrompt {
        node_id: 0,
        prompt: "New prompt".to_string(),
    })
    .expect("edit prompt");
    bus.apply(AuthoringCommand::EditChoiceOptionText {
        node_id: 0,
        option_index: 0,
        text: "Stay left".to_string(),
    })
    .expect("edit option text");
    bus.apply(AuthoringCommand::ReorderChoiceOption {
        node_id: 0,
        from_index: 0,
        to_index: 1,
    })
    .expect("reorder option");
    let connections = bus
        .graph()
        .connections()
        .map(|conn| (conn.from, conn.from_port, conn.to))
        .collect::<Vec<_>>();
    assert!(connections.contains(&(0, 0, 2)));
    assert!(connections.contains(&(0, 1, 1)));

    bus.apply(AuthoringCommand::CreateFragment {
        fragment_id: "intro".to_string(),
        title: "Intro".to_string(),
        node_ids: vec![1],
    })
    .expect("create fragment");
    bus.apply(AuthoringCommand::EnterFragment {
        fragment_id: "intro".to_string(),
    })
    .expect("enter fragment");
    bus.apply(AuthoringCommand::LeaveFragment)
        .expect("leave fragment");
    bus.apply(AuthoringCommand::RemoveFragment {
        fragment_id: "intro".to_string(),
    })
    .expect("remove fragment");

    let field_paths = bus
        .operation_log()
        .iter()
        .flat_map(|entry| entry.field_paths.iter().map(|path| path.value.as_str()))
        .collect::<Vec<_>>();
    assert!(field_paths.contains(&"graph.nodes[0].choice.prompt"));
    assert!(field_paths.contains(&"graph.nodes[0].choice.options[0].text"));
    assert!(field_paths.contains(&"graph.nodes[0].choice.options"));
    assert!(field_paths.contains(&"graph.active_fragment"));
    assert!(bus.graph().get_fragment("intro").is_none());
    assert!(bus
        .operation_log()
        .iter()
        .any(|entry| entry.operation_kind_v2.as_ref() == Some(&OperationKind::FragmentRemoved)));
    assert_eq!(bus.operation_log().len(), bus.verification_runs().len());
}
