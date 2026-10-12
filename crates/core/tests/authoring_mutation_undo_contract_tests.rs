use visual_novel_engine::authoring::*;

fn pos() -> AuthoringPosition {
    AuthoringPosition::new(40.0, 80.0)
}

fn choice() -> StoryNode {
    StoryNode::Choice {
        prompt: "Route".into(),
        options: vec!["Continue".into()],
    }
}

#[test]
fn graph_snapshots_share_unchanged_nodes_and_isolate_mutations() {
    let mut graph = NodeGraph::new();
    let first = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".into(),
            text: "x".repeat(64 * 1024),
        },
        pos(),
    );
    let second = graph.add_node(choice(), pos());
    let serialized = serde_json::to_value(&graph).unwrap();
    let snapshots = (0..50).map(|_| graph.clone()).collect::<Vec<_>>();
    assert!(snapshots.iter().all(|saved| std::ptr::eq(
        saved.get_node(first).unwrap(),
        graph.get_node(first).unwrap()
    )));
    assert_eq!(snapshots[0].nodes().count(), 2);
    *graph.get_node_mut(second).unwrap() = StoryNode::End;
    assert!(std::ptr::eq(
        snapshots[0].get_node(first).unwrap(),
        graph.get_node(first).unwrap()
    ));
    assert_ne!(snapshots[0].get_node(second), graph.get_node(second));
    graph.set_node_pos(first, AuthoringPosition::new(90.0, 100.0));
    assert_eq!(snapshots[0].get_node_pos(first), Some(pos()));
    assert_eq!(serde_json::to_value(&snapshots[0]).unwrap(), serialized);
    let decoded: NodeGraph = serde_json::from_value(serialized).unwrap();
    assert!(decoded.has_same_content(&snapshots[0]));
    graph.remove_node(first);
    assert!(snapshots[0].get_node(first).is_some());
}

fn assert_undo_parity(graph: NodeGraph, command: AuthoringCommand) {
    let original = serde_json::to_value(&graph).unwrap();
    let mut bus = AuthoringCommandBus::new(graph.clone());
    let outcome = bus.apply(command.clone()).expect("apply graph command");
    let delta = serde_json::to_value(&outcome.delta).unwrap();
    let mutation = delta.as_object().unwrap().values().next().unwrap();
    if let Some(undo) = mutation.get("undo") {
        assert!(undo["nodes"].as_array().unwrap().len() <= 1);
        assert!(undo["connections"].as_array().unwrap().len() <= 1);
    }
    let changed = serde_json::to_value(bus.graph()).unwrap();
    assert_ne!(changed, original);
    let mut session = AuthoringDocumentSession::new(AuthoringDocument::new(graph));
    let doc_outcome = session
        .apply(AuthoringDocumentCommand::Graph(command.clone()))
        .expect("apply document command");
    let metadata = |operation: &OperationLogEntry| {
        serde_json::json!({
            "kind": operation.operation_kind_v2,
            "paths": operation.field_paths,
            "targets": operation.affected_targets,
            "diagnostic_id": operation.diagnostic_id,
            "diagnostic_target": operation.diagnostic_target,
            "before": operation.before_value,
            "after": operation.after_value,
        })
    };
    assert_eq!(
        metadata(&outcome.operation),
        metadata(&doc_outcome.operation)
    );
    assert_eq!(session.document().operation_log.len(), 1);
    assert_eq!(session.document().verification_runs.len(), 1);
    let saved = session.document().graph.clone();
    let commands = session.recorded_commands().len();
    assert!(session
        .apply(AuthoringDocumentCommand::Graph(
            AuthoringCommand::RemoveNode { node_id: u32::MAX },
        ))
        .is_err());
    assert!(session.document().graph.has_same_content(&saved));
    assert_eq!(session.recorded_commands().len(), commands);
    assert_eq!(session.document().operation_log.len(), 1);
    assert_eq!(session.document().verification_runs.len(), 1);
    assert_eq!(
        serde_json::to_value(&session.document().graph).unwrap(),
        changed
    );

    let undo = bus
        .apply(AuthoringCommand::RevertLast)
        .expect("undo graph command");
    let doc_undo = session
        .apply(AuthoringDocumentCommand::RevertLast)
        .expect("undo document command");
    assert_eq!(serde_json::to_value(bus.graph()).unwrap(), original);
    assert_eq!(
        serde_json::to_value(&session.document().graph).unwrap(),
        original
    );
    assert_eq!(
        undo.operation.after_fingerprint_sha256,
        outcome.operation.before_fingerprint_sha256
    );
    assert_eq!(
        undo.operation.operation_kind_v2,
        Some(OperationKind::Revert)
    );
    assert_eq!(
        doc_undo.operation.operation_kind_v2,
        Some(OperationKind::Revert)
    );
    assert_eq!(session.read_model().node_ids().len(), bus.graph().len());
    assert_eq!(bus.undo_delta_count(), 0);
    assert_eq!(session.undo_delta_count(), 0);
    assert_eq!(session.redo_delta_count(), 1);
    let redo = session
        .apply(AuthoringDocumentCommand::RedoLast)
        .expect("redo document command");
    assert_eq!(
        serde_json::to_value(&session.document().graph).unwrap(),
        changed
    );
    assert_eq!(redo.operation.operation_kind_v2, Some(OperationKind::Redo));
    assert_eq!(session.redo_delta_count(), 0);
    assert_eq!(session.undo_delta_count(), 1);
    // Reapplying after undo must allocate the same IDs and produce the same graph.
    bus.apply(command).expect("reapply");
    assert_eq!(serde_json::to_value(bus.graph()).unwrap(), changed);
}

#[test]
fn branching_undo_restores_new_hubs_existing_choices_and_replaced_routes() {
    for scenario in 0..5 {
        let mut graph = NodeGraph::new();
        let from = graph.add_node(
            if scenario == 3 {
                choice()
            } else {
                StoryNode::Start
            },
            pos(),
        );
        let first = graph.add_node(
            if scenario == 1 {
                choice()
            } else {
                StoryNode::End
            },
            pos(),
        );
        let to = graph.add_node(
            if scenario == 2 {
                choice()
            } else {
                StoryNode::End
            },
            pos(),
        );
        if scenario != 4 {
            graph.connect(from, first);
        }
        let unrelated = graph.add_node(StoryNode::Start, pos());
        graph.connect(unrelated, first);
        if scenario == 0 {
            for _ in 0..64 {
                let id = graph.add_node(
                    StoryNode::Dialogue {
                        speaker: "Other".into(),
                        text: "Unchanged".repeat(128),
                    },
                    pos(),
                );
                graph.connect(id, first);
            }
        }
        assert_undo_parity(
            graph,
            AuthoringCommand::ConnectOrBranch {
                from,
                from_port: if scenario == 3 { 1 } else { 0 },
                to,
                branch_position: pos(),
            },
        );
    }
}

#[test]
fn node_removal_undo_restores_bookmarks_fragment_ports_and_entry_order() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos());
    let middle = graph.add_node(
        StoryNode::Dialogue {
            speaker: "N".into(),
            text: "Middle".into(),
        },
        pos(),
    );
    let end = graph.add_node(StoryNode::End, pos());
    graph.connect(start, middle);
    graph.connect(middle, end);
    let other = graph.add_node(StoryNode::Start, pos());
    graph.connect(other, end);
    let external = graph.add_node(StoryNode::Start, pos());
    graph.connect(external, middle);
    assert!(graph.set_bookmark("middle", middle));
    assert!(graph.set_bookmark("alias", middle));
    assert!(graph.set_bookmark("end", end));
    assert!(graph.create_fragment("chapter", "Chapter", vec![start, middle]));
    assert_undo_parity(graph, AuthoringCommand::RemoveNode { node_id: middle });
}

#[test]
fn document_redo_preserves_branch_order_and_replays_history() {
    let mut session = AuthoringDocumentSession::new(AuthoringDocument::new(NodeGraph::new()));
    for (node_id, node) in [(0, StoryNode::Start), (1, StoryNode::End)] {
        session
            .apply(AuthoringDocumentCommand::Graph(
                AuthoringCommand::CreateNode {
                    node_id,
                    node,
                    position: pos(),
                },
            ))
            .unwrap();
    }
    let expected = serde_json::to_value(&session.document().graph).unwrap();
    for _ in 0..2 {
        session.apply(AuthoringDocumentCommand::RevertLast).unwrap();
    }
    assert_eq!(session.redo_delta_count(), 2);
    // A rejected new edit must not discard the redo branch.
    let count = session.recorded_commands().len();
    assert!(session
        .apply(AuthoringDocumentCommand::Graph(
            AuthoringCommand::RemoveNode { node_id: 999 }
        ))
        .is_err());
    assert_eq!(session.redo_delta_count(), 2);
    assert_eq!(session.recorded_commands().len(), count);
    for _ in 0..2 {
        session.apply(AuthoringDocumentCommand::RedoLast).unwrap();
    }
    assert_eq!(
        serde_json::to_value(&session.document().graph).unwrap(),
        expected
    );
    assert!(session.apply(AuthoringDocumentCommand::RedoLast).is_err());
    let replay = AuthoringDocumentSession::replay(session.recorded_commands()).unwrap();
    assert_eq!(
        serde_json::to_value(&replay.document().graph).unwrap(),
        expected
    );
    assert_eq!(
        replay
            .document()
            .operation_log
            .iter()
            .map(|entry| &entry.operation_kind)
            .collect::<Vec<_>>(),
        session
            .document()
            .operation_log
            .iter()
            .map(|entry| &entry.operation_kind)
            .collect::<Vec<_>>()
    );
    session.apply(AuthoringDocumentCommand::RevertLast).unwrap();
    session
        .apply(AuthoringDocumentCommand::Graph(
            AuthoringCommand::CreateNode {
                node_id: 2,
                node: StoryNode::End,
                position: pos(),
            },
        ))
        .unwrap();
    assert_eq!(session.redo_delta_count(), 0);
    assert!(session.apply(AuthoringDocumentCommand::RedoLast).is_err());
}

#[test]
fn quick_fix_undo_restores_node_edits_and_structural_changes() {
    for fix_id in [
        "graph_add_start",
        "node_connect_dead_end_to_end",
        "choice_add_default_option_to_end",
        "dialogue_fill_speaker",
    ] {
        let mut graph = NodeGraph::new();
        if fix_id != "graph_add_start" {
            graph.add_node(StoryNode::Start, pos());
        }
        let node = if fix_id == "choice_add_default_option_to_end" {
            StoryNode::Choice {
                prompt: "Route".into(),
                options: Vec::new(),
            }
        } else {
            StoryNode::Dialogue {
                speaker: String::new(),
                text: "Hello".into(),
            }
        };
        graph.add_node(node, pos());
        let issue = validate_authoring_graph_no_io(&graph)
            .into_iter()
            .find(|issue| {
                quick_fix::suggest_fixes(issue, &graph)
                    .iter()
                    .any(|fix| fix.fix_id == fix_id)
            })
            .expect("matching diagnostic");
        assert_undo_parity(
            graph,
            AuthoringCommand::ApplyQuickFix {
                issue: Box::new(issue),
                fix_id: fix_id.into(),
            },
        );
    }
}
