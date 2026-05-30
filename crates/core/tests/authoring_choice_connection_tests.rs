use visual_novel_engine::authoring::{AuthoringPosition, NodeGraph, StoryNode};

fn pos(x: f32, y: f32) -> AuthoringPosition {
    AuthoringPosition::new(x, y)
}

#[test]
fn connect_or_branch_adds_route_to_existing_choice_target_without_nested_hub() {
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
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Where next?".to_string(),
            options: vec!["A".to_string()],
        },
        pos(200.0, 0.0),
    );
    let first = graph.add_node(dialogue("First"), pos(400.0, -100.0));
    let second = graph.add_node(dialogue("Second"), pos(400.0, 100.0));

    graph.connect(source, choice);
    graph.connect_port(choice, 0, first);

    assert!(
        graph.connect_or_branch(source, 0, second, pos(100.0, 100.0)),
        "second route should be added to the already-connected choice hub"
    );

    let choice_nodes = graph
        .nodes()
        .filter(|(_, node, _)| matches!(node, StoryNode::Choice { .. }))
        .count();
    assert_eq!(choice_nodes, 1, "must not create a nested Choice node");
    assert!(graph
        .connections()
        .any(|edge| edge.from == source && edge.from_port == 0 && edge.to == choice));
    assert!(graph
        .connections()
        .any(|edge| edge.from == choice && edge.from_port == 0 && edge.to == first));
    assert!(graph
        .connections()
        .any(|edge| edge.from == choice && edge.from_port == 1 && edge.to == second));
}

#[test]
fn connect_or_branch_rejects_choice_ports_beyond_next_option() {
    let mut graph = NodeGraph::new();
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Where next?".to_string(),
            options: vec!["A".to_string()],
        },
        pos(0.0, 0.0),
    );
    let target = graph.add_node(dialogue("Target"), pos(200.0, 0.0));

    assert!(
        !graph.connect_or_branch(choice, 5, target, pos(100.0, 100.0)),
        "a far out-of-range choice port must not be silently remapped to the next option"
    );
    assert!(!graph
        .connections()
        .any(|edge| edge.from == choice && edge.to == target));
    let Some(StoryNode::Choice { options, .. }) = graph.get_node(choice) else {
        panic!("choice node should still exist");
    };
    assert_eq!(options, &vec!["A".to_string()]);
}

fn dialogue(text: &str) -> StoryNode {
    StoryNode::Dialogue {
        speaker: "Narrator".to_string(),
        text: text.to_string(),
    }
}
