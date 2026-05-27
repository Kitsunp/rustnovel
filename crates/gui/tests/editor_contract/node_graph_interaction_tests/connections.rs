use super::*;

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
