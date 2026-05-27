use super::*;

#[test]
fn strict_export_blocks_unreachable_drafts_and_generic_payloads() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let live = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Live".to_string(),
        },
        pos(0.0, 90.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 180.0));
    graph.connect(start, live);
    graph.connect(live, end);
    graph.add_node(
        StoryNode::Dialogue {
            speaker: "Draft".to_string(),
            text: "Not connected".to_string(),
        },
        pos(300.0, 90.0),
    );
    let err = graph
        .to_script_strict()
        .expect_err("unreachable draft must block strict export");
    assert!(err.to_string().contains("unreachable"));

    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let generic = graph.add_node(
        StoryNode::Generic(EventRaw::Jump {
            target: "start".to_string(),
        }),
        pos(0.0, 90.0),
    );
    graph.connect(start, generic);
    let err = graph
        .to_script_strict()
        .expect_err("unsupported generic must block strict export");
    assert!(err.to_string().contains("not export-supported"));
}

#[test]
fn graph_fragments_are_stable_authoring_metadata() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Loop?".to_string(),
            options: vec!["Again".to_string(), "End".to_string()],
        },
        pos(0.0, 90.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 180.0));
    graph.connect(start, choice);
    graph.connect_port(choice, 0, choice);
    graph.connect_port(choice, 1, end);

    assert!(graph.create_fragment("intro_loop", "Intro Loop", vec![choice, start, choice]));
    let fragment = graph.fragment("intro_loop").unwrap();
    assert_eq!(fragment.node_ids, vec![start, choice]);
    assert!(fragment.inputs.is_empty());
    assert_eq!(fragment.outputs.len(), 1);
    assert_eq!(fragment.outputs[0].node_id, Some(choice));

    let script = graph.to_script();
    assert!(
        !script
            .labels
            .keys()
            .any(|label| label.contains("fragment_intro_loop")),
        "fragment labels are metadata and should not become public runtime contract without a call"
    );
    let flow = graph.flow_analysis(&[start]);
    assert!(flow.reachable.contains(&choice));
    assert!(flow.reachable_cycle_nodes.contains(&choice));
}

#[test]
fn subgraph_call_strict_export_flattens_fragment_with_call_namespace() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let fragment_a = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Inside fragment A".to_string(),
        },
        pos(300.0, 0.0),
    );
    let fragment_b = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Inside fragment B".to_string(),
        },
        pos(300.0, 90.0),
    );
    graph.connect(fragment_a, fragment_b);
    assert!(graph.create_fragment(
        "intro_fragment",
        "Intro Fragment",
        vec![fragment_a, fragment_b]
    ));

    let call = graph.add_node(
        StoryNode::SubgraphCall {
            fragment_id: "intro_fragment".to_string(),
            entry_port: None,
            exit_port: None,
        },
        pos(0.0, 90.0),
    );
    let after = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "After fragment".to_string(),
        },
        pos(0.0, 180.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 270.0));
    graph.connect(start, call);
    graph.connect(call, after);
    graph.connect(after, end);

    let script = graph
        .to_script_strict()
        .expect("strict export should flatten subgraph call");
    assert!(script.labels.contains_key(&format!("node_{call}")));
    assert!(script
        .labels
        .contains_key(&format!("__call_{call}_node_{fragment_a}")));
    assert!(matches!(
        script.events.first(),
        Some(EventRaw::Dialogue(dialogue)) if dialogue.text == "Inside fragment A"
    ));
    assert!(
        script
            .compile()
            .expect("flattened subgraph script compiles")
            .events
            .len()
            >= 3
    );
}

#[test]
fn fragment_nested_namespace_stress() {
    let mut graph = NodeGraph::new();
    let inner_a = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Inner A".to_string(),
        },
        pos(300.0, 0.0),
    );
    let inner_b = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Inner B".to_string(),
        },
        pos(300.0, 90.0),
    );
    graph.connect(inner_a, inner_b);
    assert!(graph.create_fragment("inner", "Inner", vec![inner_a, inner_b]));
    let inner_hash = graph.fragment("inner").expect("inner").interface_hash();

    let outer_call = graph.add_node(
        StoryNode::SubgraphCall {
            fragment_id: "inner".to_string(),
            entry_port: None,
            exit_port: None,
        },
        pos(600.0, 0.0),
    );
    assert!(graph.create_fragment("outer", "Outer", vec![outer_call]));
    let outer_hash = graph.fragment("outer").expect("outer").interface_hash();
    assert_ne!(inner_hash, outer_hash);

    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let call_outer = graph.add_node(
        StoryNode::SubgraphCall {
            fragment_id: "outer".to_string(),
            entry_port: None,
            exit_port: None,
        },
        pos(0.0, 90.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 180.0));
    graph.connect(start, call_outer);
    graph.connect(call_outer, end);

    let script = graph.to_script_strict().expect("nested strict export");
    assert!(script
        .labels
        .keys()
        .any(|label| label.contains(&format!("__call_{call_outer}_"))));
    script.compile().expect("nested fragment script compiles");
}

#[test]
fn repeated_fragment_calls_do_not_collide() {
    let mut graph = NodeGraph::new();
    let fragment_line = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Shared fragment".to_string(),
        },
        pos(300.0, 0.0),
    );
    assert!(graph.create_fragment("shared", "Shared", vec![fragment_line]));

    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let call_a = graph.add_node(
        StoryNode::SubgraphCall {
            fragment_id: "shared".to_string(),
            entry_port: None,
            exit_port: None,
        },
        pos(0.0, 90.0),
    );
    let call_b = graph.add_node(
        StoryNode::SubgraphCall {
            fragment_id: "shared".to_string(),
            entry_port: None,
            exit_port: None,
        },
        pos(0.0, 180.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 270.0));
    graph.connect(start, call_a);
    graph.connect(call_a, call_b);
    graph.connect(call_b, end);

    let script = graph.to_script_strict().expect("strict export");
    assert!(script
        .labels
        .contains_key(&format!("__call_{call_a}_node_{fragment_line}")));
    assert!(script
        .labels
        .contains_key(&format!("__call_{call_b}_node_{fragment_line}")));
}

#[test]
fn deleting_internal_node_invalidates_ports_and_blocks_strict_export() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let inside = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Inside".to_string(),
        },
        pos(300.0, 0.0),
    );
    let after = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "After".to_string(),
        },
        pos(0.0, 180.0),
    );
    graph.connect(start, inside);
    graph.connect(inside, after);
    assert!(graph.create_fragment("frag", "Frag", vec![inside]));
    let call = graph.add_node(
        StoryNode::SubgraphCall {
            fragment_id: "frag".to_string(),
            entry_port: None,
            exit_port: None,
        },
        pos(0.0, 90.0),
    );
    graph.connect(start, call);
    graph.connect(call, after);

    graph.connect_port(inside, 1, after);

    let issues = graph.validate_fragments();
    assert!(issues.iter().any(|issue| {
        issue.code == LintCode::FragmentPortStale || issue.message.contains("stale")
    }));
    let err = graph
        .to_script_strict()
        .expect_err("stale fragment ports must block strict export");
    assert!(err.to_string().contains("stale") || err.to_string().contains("fragment"));
}

#[test]
fn fragment_ownership_conflict_is_blocked_at_creation() {
    let mut graph = NodeGraph::new();
    let node = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Shared".to_string(),
        },
        pos(0.0, 0.0),
    );

    assert!(graph.create_fragment("one", "One", vec![node]));
    assert!(
        !graph.create_fragment("two", "Two", vec![node]),
        "a node must not belong to two fragments"
    );
}

#[test]
fn fragment_validation_detects_indirect_subgraph_recursion() {
    let mut graph = NodeGraph::new();
    let call_b = graph.add_node(
        StoryNode::SubgraphCall {
            fragment_id: "b".to_string(),
            entry_port: None,
            exit_port: None,
        },
        pos(0.0, 0.0),
    );
    let call_a = graph.add_node(
        StoryNode::SubgraphCall {
            fragment_id: "a".to_string(),
            entry_port: None,
            exit_port: None,
        },
        pos(300.0, 0.0),
    );
    assert!(graph.create_fragment("a", "A", vec![call_b]));
    assert!(graph.create_fragment("b", "B", vec![call_a]));

    let recursion = graph
        .validate_fragments()
        .into_iter()
        .filter(|issue| issue.code == LintCode::FragmentRecursion)
        .collect::<Vec<_>>();

    assert_eq!(recursion.len(), 2);
    assert!(recursion
        .iter()
        .any(|issue| issue.message.contains("a -> b -> a")));
    assert!(recursion.iter().all(|issue| issue.evidence_trace.is_some()));
}
