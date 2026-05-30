use visual_novel_engine::authoring::*;
use visual_novel_engine::runtime::CharacterPlacementRaw;

fn pos(x: f32, y: f32) -> AuthoringPosition {
    AuthoringPosition::new(x, y)
}

fn scene_document() -> (AuthoringDocument, u32, String) {
    let mut graph = NodeGraph::new();
    let scene_id = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("assets/bg/room.png".to_string()),
            music: None,
            characters: vec![CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("assets/characters/ava.png".to_string()),
                x: Some(10),
                y: Some(20),
                scale: Some(1.0),
                ..Default::default()
            }],
        },
        pos(0.0, 0.0),
    );
    let object_id = composer::list_layered_objects(&graph, Some(scene_id))
        .into_iter()
        .find(|object| object.character_name.as_deref() == Some("Ava"))
        .expect("character layer")
        .object_id;
    (AuthoringDocument::new(graph), scene_id, object_id)
}

#[test]
fn document_command_bus_layer_visible_trace() {
    let (document, _, object_id) = scene_document();
    let mut bus = AuthoringDocumentCommandBus::new(document);
    let before_script = bus
        .document()
        .graph
        .to_script_lossy_for_diagnostics()
        .to_json()
        .expect("before script json");

    let outcome = bus
        .apply(AuthoringDocumentCommand::SetLayerVisible {
            object_id: object_id.clone(),
            visible: false,
        })
        .expect("set layer visible");

    assert!(matches!(
        outcome.delta,
        AuthoringDocumentDelta::LayerVisibleChanged { .. }
    ));
    assert_eq!(
        bus.document()
            .composer_layer_overrides
            .get(&object_id)
            .map(|override_| override_.visible),
        Some(false)
    );
    assert_eq!(
        outcome.operation.operation_kind_v2,
        Some(OperationKind::LayerVisibilityChanged)
    );
    assert_eq!(
        outcome
            .operation
            .field_paths
            .first()
            .map(|path| path.value.as_str()),
        Some(format!("composer.layers[{object_id}].visible").as_str())
    );
    assert_eq!(
        outcome.operation.operation_id,
        outcome.verification.operation_id
    );
    assert_eq!(
        outcome.before_fingerprint.story_semantic_sha256,
        outcome.after_fingerprint.story_semantic_sha256
    );
    assert_ne!(
        outcome.before_fingerprint.full_document_sha256,
        outcome.after_fingerprint.full_document_sha256
    );
    assert_eq!(
        before_script,
        bus.document()
            .graph
            .to_script_lossy_for_diagnostics()
            .to_json()
            .expect("after script json")
    );
}

#[test]
fn document_command_bus_layer_locked_trace() {
    let (document, _, object_id) = scene_document();
    let mut bus = AuthoringDocumentCommandBus::new(document);
    let outcome = bus
        .apply(AuthoringDocumentCommand::SetLayerLocked {
            object_id: object_id.clone(),
            locked: true,
        })
        .expect("set layer locked");

    assert!(matches!(
        outcome.delta,
        AuthoringDocumentDelta::LayerLockedChanged { .. }
    ));
    assert_eq!(
        bus.document()
            .composer_layer_overrides
            .get(&object_id)
            .map(|override_| override_.locked),
        Some(true)
    );
    assert_eq!(
        outcome.operation.operation_kind_v2,
        Some(OperationKind::LayerLockChanged)
    );
    assert_eq!(bus.document().operation_log.len(), 1);
    assert_eq!(bus.document().verification_runs.len(), 1);
}

#[test]
fn document_command_bus_rejects_unknown_layer_object_ids_without_stale_overrides() {
    let (document, _, _) = scene_document();
    let mut bus = AuthoringDocumentCommandBus::new(document);

    let err = bus
        .apply(AuthoringDocumentCommand::SetLayerVisible {
            object_id: "missing-layer-object".to_string(),
            visible: false,
        })
        .expect_err("unknown layer object must not create a stale override");

    assert!(err.contains("unknown composer layer object"));
    assert!(bus.document().composer_layer_overrides.is_empty());
    assert!(bus.document().operation_log.is_empty());
    assert!(bus.document().verification_runs.is_empty());
    assert!(bus
        .read_model()
        .composer_layer("missing-layer-object")
        .is_none());
}

#[test]
fn document_command_bus_background_fit_trace() {
    let (document, scene_id, _) = scene_document();
    let mut bus = AuthoringDocumentCommandBus::new(document);
    let outcome = bus
        .apply(AuthoringDocumentCommand::SetBackgroundFitOverride {
            node_id: scene_id,
            fit: composer::BackgroundFit::Contain,
        })
        .expect("set background fit");

    assert!(matches!(
        outcome.delta,
        AuthoringDocumentDelta::BackgroundFitChanged { .. }
    ));
    assert_eq!(
        bus.document()
            .composer_background_fit_overrides
            .get(&scene_id.to_string()),
        Some(&composer::BackgroundFit::Contain)
    );
    assert_eq!(
        outcome.operation.field_paths[0].value,
        format!("composer.background_fit[{scene_id}]")
    );
    assert_eq!(
        outcome.operation.operation_kind_v2,
        Some(OperationKind::FieldEdited)
    );
}

#[test]
fn document_command_bus_clear_background_fit_trace() {
    let (document, scene_id, _) = scene_document();
    let mut bus = AuthoringDocumentCommandBus::new(document);
    bus.apply(AuthoringDocumentCommand::SetBackgroundFitOverride {
        node_id: scene_id,
        fit: composer::BackgroundFit::Contain,
    })
    .expect("set background fit");

    let outcome = bus
        .apply(AuthoringDocumentCommand::ClearBackgroundFitOverride { node_id: scene_id })
        .expect("clear background fit");

    assert!(matches!(
        outcome.delta,
        AuthoringDocumentDelta::BackgroundFitCleared { .. }
    ));
    assert!(!bus
        .document()
        .composer_background_fit_overrides
        .contains_key(&scene_id.to_string()));
    assert_eq!(bus.document().operation_log.len(), 2);
}

#[test]
fn document_command_bus_graph_command_delegates_to_authoring_bus() {
    let mut graph = NodeGraph::new();
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Before".to_string(),
        },
        pos(0.0, 0.0),
    );
    let mut bus = AuthoringDocumentCommandBus::new(AuthoringDocument::new(graph));
    let before_script = bus
        .document()
        .graph
        .to_script_lossy_for_diagnostics()
        .to_json()
        .expect("before script json");

    let outcome = bus
        .apply(AuthoringDocumentCommand::Graph(
            AuthoringCommand::EditDialogue {
                node_id: dialogue,
                speaker: "Ava".to_string(),
                text: "After".to_string(),
            },
        ))
        .expect("edit dialogue through document bus");

    let AuthoringDocumentDelta::Graph(delta) = outcome.delta else {
        panic!("expected graph delta");
    };
    assert!(matches!(*delta, AuthoringDelta::NodeEdited { .. }));
    assert_eq!(
        outcome.operation.operation_kind_v2,
        Some(OperationKind::FieldEdited)
    );
    assert_ne!(
        outcome.before_fingerprint.story_semantic_sha256,
        outcome.after_fingerprint.story_semantic_sha256
    );
    let Some(StoryNode::Dialogue { text, .. }) = bus.document().graph.get_node(dialogue) else {
        panic!("dialogue node should remain");
    };
    assert_eq!(text, "After");
    assert_ne!(
        before_script,
        bus.document()
            .graph
            .to_script_lossy_for_diagnostics()
            .to_json()
            .expect("after script json")
    );
}

#[test]
fn document_command_bus_replay_headless() {
    let commands = vec![
        AuthoringDocumentCommand::Graph(AuthoringCommand::CreateNode {
            node_id: 0,
            node: StoryNode::Start,
            position: pos(0.0, 0.0),
        }),
        AuthoringDocumentCommand::SetBackgroundFitOverride {
            node_id: 0,
            fit: composer::BackgroundFit::Tile,
        },
    ];

    let bus = AuthoringDocumentCommandBus::replay(&commands).expect("replay document commands");
    assert_eq!(bus.document().graph.len(), 1);
    assert_eq!(
        bus.document().composer_background_fit_overrides.get("0"),
        Some(&composer::BackgroundFit::Tile)
    );
    assert_eq!(bus.document().operation_log.len(), commands.len());
    assert_eq!(bus.document().verification_runs.len(), commands.len());
}

#[test]
fn document_command_bus_rejects_noop_without_fake_log() {
    let (document, scene_id, object_id) = scene_document();
    let mut bus = AuthoringDocumentCommandBus::new(document);

    bus.apply(AuthoringDocumentCommand::SetLayerVisible {
        object_id,
        visible: true,
    })
    .expect_err("default visible layer should be a no-op");
    bus.apply(AuthoringDocumentCommand::ClearBackgroundFitOverride { node_id: scene_id })
        .expect_err("clearing a missing background fit should be a no-op");

    assert!(bus.document().operation_log.is_empty());
    assert!(bus.document().verification_runs.is_empty());
    assert_eq!(bus.undo_delta_count(), 0);
}

#[test]
fn document_command_bus_fingerprint_semantic_vs_document_split() {
    let (document, _, object_id) = scene_document();
    let mut bus = AuthoringDocumentCommandBus::new(document);

    let document_outcome = bus
        .apply(AuthoringDocumentCommand::SetLayerVisible {
            object_id,
            visible: false,
        })
        .expect("set layer visible");

    assert_eq!(
        document_outcome.before_fingerprint.story_semantic_sha256,
        document_outcome.after_fingerprint.story_semantic_sha256
    );
    assert_ne!(
        document_outcome.before_fingerprint.full_document_sha256,
        document_outcome.after_fingerprint.full_document_sha256
    );
}

#[test]
fn document_command_bus_undo_redo_delta_contract() {
    let (document, scene_id, _) = scene_document();
    let mut bus = AuthoringDocumentCommandBus::new(document);
    bus.apply(AuthoringDocumentCommand::SetBackgroundFitOverride {
        node_id: scene_id,
        fit: composer::BackgroundFit::Contain,
    })
    .expect("set background fit");

    let outcome = bus
        .apply(AuthoringDocumentCommand::RevertLast)
        .expect("revert last delta");

    assert!(matches!(
        outcome.delta,
        AuthoringDocumentDelta::Reverted { .. }
    ));
    assert_eq!(bus.undo_delta_count(), 0);
    assert_eq!(bus.redo_delta_count(), 1);
    assert!(!bus
        .document()
        .composer_background_fit_overrides
        .contains_key(&scene_id.to_string()));
    assert_eq!(bus.document().operation_log.len(), 2);
}

#[test]
fn document_session_layout_delta_updates_read_model_and_dirty_flags() {
    let (document, _, object_id) = scene_document();
    let mut session = AuthoringDocumentSession::new(document);
    assert!(session.dirty_flags().is_clean());
    assert!(
        session
            .read_model()
            .composer_layer(&object_id)
            .expect("layer indexed before mutation")
            .visible
    );

    let outcome = session
        .apply(AuthoringDocumentCommand::SetLayerVisible {
            object_id: object_id.clone(),
            visible: false,
        })
        .expect("set layer visible");

    let flags = session.dirty_flags();
    assert!(!flags.graph_dirty);
    assert!(flags.layout_dirty);
    assert!(!flags.assets_dirty);
    assert!(flags.document_dirty);
    assert!(!flags.validation_dirty);
    assert!(!flags.runtime_export_dirty);
    assert!(
        !session
            .read_model()
            .composer_layer(&object_id)
            .expect("layer stays indexed after delta")
            .visible
    );
    assert!(outcome.verification.resolved_diagnostic_ids.is_empty());
    assert!(outcome.verification.introduced_diagnostic_ids.is_empty());
    assert_eq!(
        outcome.before_fingerprint.story_semantic_sha256,
        outcome.after_fingerprint.story_semantic_sha256
    );

    session.clear_dirty_flags();
    assert!(session.dirty_flags().is_clean());
}

#[test]
fn document_session_graph_delta_rebuilds_read_model_asset_refs_and_marks_runtime_dirty() {
    let (document, scene_id, _) = scene_document();
    let mut session = AuthoringDocumentSession::new(document);
    assert!(session
        .read_model()
        .asset_refs()
        .contains(&"assets/bg/room.png".to_string()));

    let outcome = session
        .apply(AuthoringDocumentCommand::Graph(
            AuthoringCommand::EditNode {
                node_id: scene_id,
                replacement: StoryNode::Scene {
                    profile: None,
                    background: Some("assets/bg/lab.png".to_string()),
                    music: None,
                    characters: vec![],
                },
            },
        ))
        .expect("edit scene through document session");

    let flags = session.dirty_flags();
    assert!(flags.graph_dirty);
    assert!(flags.layout_dirty);
    assert!(flags.assets_dirty);
    assert!(flags.document_dirty);
    assert!(flags.validation_dirty);
    assert!(flags.runtime_export_dirty);
    assert!(session
        .read_model()
        .asset_refs()
        .contains(&"assets/bg/lab.png".to_string()));
    assert!(!session
        .read_model()
        .asset_refs()
        .contains(&"assets/bg/room.png".to_string()));
    assert_ne!(
        outcome.before_fingerprint.story_semantic_sha256,
        outcome.after_fingerprint.story_semantic_sha256
    );
}

#[test]
fn document_session_read_model_indexes_nodes_routes_diagnostics_and_preview_data() {
    let mut graph = NodeGraph::new();
    let start_id = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let dialogue_id = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "old laboratory note".to_string(),
        },
        pos(100.0, 0.0),
    );
    let end_id = graph.add_node(StoryNode::End, pos(200.0, 0.0));
    let orphan_id = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Nox".to_string(),
            text: "orphan route".to_string(),
        },
        pos(100.0, 120.0),
    );
    graph.connect(start_id, dialogue_id);
    graph.connect(dialogue_id, end_id);

    let mut session = AuthoringDocumentSession::new(AuthoringDocument::new(graph));
    let read_model = session.read_model();
    assert!(read_model.node_ids().contains(&dialogue_id));
    assert_eq!(
        read_model.nodes_by_text("old laboratory"),
        vec![dialogue_id]
    );
    assert!(read_model.reachable_node_ids().contains(&dialogue_id));
    assert!(read_model.unreachable_node_ids().contains(&orphan_id));
    assert!(read_model.preview_data_for_node(dialogue_id).is_some());
    assert!(read_model
        .diagnostics_by_code(LintCode::UnreachableNode.label())
        .iter()
        .any(|issue| issue.node_id == Some(orphan_id)));
    assert!(!read_model.diagnostics_for_node(orphan_id).is_empty());

    session
        .apply(AuthoringDocumentCommand::Graph(
            AuthoringCommand::EditDialogue {
                node_id: dialogue_id,
                speaker: "Ava".to_string(),
                text: "new laboratory note".to_string(),
            },
        ))
        .expect("edit dialogue through document session");

    assert!(session
        .read_model()
        .nodes_by_text("old laboratory")
        .is_empty());
    assert_eq!(
        session.read_model().nodes_by_text("new laboratory"),
        vec![dialogue_id]
    );
    assert!(session
        .read_model()
        .diagnostics_for_node(orphan_id)
        .iter()
        .any(|issue| issue.code == LintCode::UnreachableNode));
}

#[test]
fn document_session_read_model_reports_stale_domains_from_fingerprints() {
    let (document, _, object_id) = scene_document();
    let mut session = AuthoringDocumentSession::new(document);

    let outcome = session
        .apply(AuthoringDocumentCommand::SetLayerVisible {
            object_id,
            visible: false,
        })
        .expect("set layer visible");

    let stale = session
        .read_model()
        .report_stale_state(&outcome.before_fingerprint, &outcome.after_fingerprint);
    assert!(stale.is_stale());
    assert!(!stale.semantic_stale);
    assert!(stale.layout_stale);
    assert!(!stale.assets_stale);
    assert!(stale.full_document_stale);
}

#[test]
fn document_session_revert_rebuilds_read_model_without_snapshot_undo() {
    let (document, _, object_id) = scene_document();
    let mut session = AuthoringDocumentSession::new(document);

    session
        .apply(AuthoringDocumentCommand::SetLayerLocked {
            object_id: object_id.clone(),
            locked: true,
        })
        .expect("lock layer");
    assert!(
        session
            .read_model()
            .composer_layer(&object_id)
            .expect("layer indexed after lock")
            .locked
    );
    session.clear_dirty_flags();

    let outcome = session
        .apply(AuthoringDocumentCommand::RevertLast)
        .expect("revert lock delta");

    assert!(matches!(
        outcome.delta,
        AuthoringDocumentDelta::Reverted { .. }
    ));
    assert!(
        !session
            .read_model()
            .composer_layer(&object_id)
            .expect("layer indexed after revert")
            .locked
    );
    assert_eq!(session.undo_delta_count(), 0);
    assert_eq!(session.redo_delta_count(), 1);
    let flags = session.dirty_flags();
    assert!(flags.layout_dirty);
    assert!(flags.document_dirty);
    assert!(!flags.validation_dirty);
}
