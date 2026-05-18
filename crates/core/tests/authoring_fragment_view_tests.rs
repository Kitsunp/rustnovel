use visual_novel_engine::authoring::{AuthoringPosition, NodeGraph, StoryNode};
use visual_novel_engine::EventRaw;

fn dialogue(label: &str) -> StoryNode {
    StoryNode::Dialogue {
        speaker: "Narrator".to_string(),
        text: label.to_string(),
    }
}

#[test]
fn active_fragment_filters_visible_nodes_and_connections() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let inner_a = graph.add_node(dialogue("inside a"), AuthoringPosition::new(100.0, 0.0));
    let inner_b = graph.add_node(dialogue("inside b"), AuthoringPosition::new(200.0, 0.0));
    let end = graph.add_node(StoryNode::End, AuthoringPosition::new(300.0, 0.0));

    graph.connect(start, inner_a);
    graph.connect(inner_a, inner_b);
    graph.connect(inner_b, end);
    assert!(graph.create_fragment("chapter_intro", "Chapter intro", vec![inner_b, inner_a]));

    assert_eq!(graph.visible_node_ids(), vec![start, end]);
    assert!(graph.visible_connections().is_empty());

    assert!(graph.enter_fragment("chapter_intro"));
    assert_eq!(graph.visible_node_ids(), vec![inner_a, inner_b]);
    let visible_edges = graph.visible_connections();
    assert_eq!(visible_edges.len(), 1);
    assert_eq!(visible_edges[0].from, inner_a);
    assert_eq!(visible_edges[0].to, inner_b);

    assert!(graph.leave_fragment());
    assert_eq!(graph.visible_node_ids(), vec![start, end]);
}

#[test]
fn subgraph_call_two_outputs_export_to_distinct_targets() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let branch = graph.add_node(
        StoryNode::Choice {
            prompt: "Where?".to_string(),
            options: vec!["Left".to_string(), "Right".to_string()],
        },
        AuthoringPosition::new(100.0, 0.0),
    );
    let left = graph.add_node(dialogue("Left path"), AuthoringPosition::new(300.0, -80.0));
    let right = graph.add_node(dialogue("Right path"), AuthoringPosition::new(300.0, 80.0));

    graph.connect_port(branch, 0, left);
    graph.connect_port(branch, 1, right);
    assert!(graph.create_fragment("route_split", "Route split", vec![branch]));

    let call = graph.add_node(
        StoryNode::SubgraphCall {
            fragment_id: "route_split".to_string(),
            entry_port: None,
            exit_port: None,
        },
        AuthoringPosition::new(0.0, 100.0),
    );
    graph.connect(start, call);
    graph.connect_port(call, 0, left);
    graph.connect_port(call, 1, right);

    let script = graph
        .to_script_strict()
        .expect("two-output subgraph call should export");
    let choice = script
        .events
        .iter()
        .find_map(|event| match event {
            EventRaw::Choice(choice) => Some(choice),
            _ => None,
        })
        .expect("fragment branch should become a runtime choice");

    assert_eq!(choice.options.len(), 2);
    assert_eq!(choice.options[0].target, format!("node_{left}"));
    assert_eq!(choice.options[1].target, format!("node_{right}"));
    assert_ne!(choice.options[0].target, choice.options[1].target);
}

#[test]
fn strict_export_rejects_unreachable_draft_nodes_inside_fragment() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let entry = graph.add_node(dialogue("Entry"), AuthoringPosition::new(100.0, 0.0));
    let draft = graph.add_node(
        dialogue("Disconnected draft"),
        AuthoringPosition::new(100.0, 120.0),
    );
    let call = graph.add_node(
        StoryNode::SubgraphCall {
            fragment_id: "frag".to_string(),
            entry_port: None,
            exit_port: None,
        },
        AuthoringPosition::new(200.0, 0.0),
    );
    graph.connect(start, call);
    assert!(graph.create_fragment("frag", "Fragment", vec![entry, draft]));

    let err = graph
        .to_script_strict()
        .expect_err("strict export must not leak disconnected fragment drafts");
    assert!(format!("{err}").contains("unreachable/draft node"));
}
