use super::super::*;
use eframe::egui;

fn add_scene_with_character(workbench: &mut EditorWorkbench) -> (u32, String) {
    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("assets/bg/room.png".to_string()),
            music: None,
            characters: vec![visual_novel_engine::runtime::CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("assets/characters/ava.png".to_string()),
                x: Some(10),
                y: Some(20),
                scale: Some(1.0),
                ..Default::default()
            }],
        },
        egui::pos2(0.0, 0.0),
    );
    let object_id = visual_novel_engine::authoring::composer::list_layered_objects(
        workbench.node_graph.authoring_graph(),
        Some(scene),
    )
    .into_iter()
    .find(|object| object.character_name.as_deref() == Some("Ava"))
    .expect("character layer")
    .object_id;
    (scene, object_id)
}

#[test]
fn layout_reset_preserves_document_history_and_is_reversible() {
    use visual_novel_engine::authoring::AuthoringDocumentCommand;
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let (scene, object_id) = add_scene_with_character(&mut workbench);
    workbench.ensure_authoring_session_matches_fields();
    let baseline = workbench.authoring_session.document().clone();
    for command in [
        AuthoringDocumentCommand::SetLayerVisible { object_id: object_id.clone(), visible: false },
        AuthoringDocumentCommand::SetLayerLocked { object_id: object_id.clone(), locked: true },
        AuthoringDocumentCommand::SetBackgroundFitOverride { node_id: scene, fit: crate::editor::BackgroundFit::Contain },
    ] {
        workbench.apply_authoring_document_command(command).unwrap();
    }
    let before_layers = workbench.composer_layer_overrides.clone();
    let before_fits = workbench.composer_background_fit_overrides.clone();
    let ctx = egui::Context::default();
    workbench.reset_layout_state(&ctx);
    assert!(workbench.composer_layer_overrides.is_empty());
    assert!(workbench.composer_background_fit_overrides.is_empty());
    assert_eq!(workbench.authoring_session.undo_delta_count(), 4);
    let commands = workbench.authoring_session.recorded_commands().len();
    workbench.reset_layout_state(&ctx);
    assert_eq!(workbench.authoring_session.recorded_commands().len(), commands);
    assert!(workbench.apply_graph_undo());
    assert_eq!(workbench.composer_layer_overrides, before_layers);
    assert_eq!(workbench.composer_background_fit_overrides, before_fits);
    assert!(workbench.apply_graph_undo());
    assert!(workbench.composer_background_fit_overrides.is_empty());
    assert!(workbench.apply_graph_redo());
    assert_eq!(workbench.composer_background_fit_overrides, before_fits);
    assert!(workbench.apply_graph_redo());
    assert!(workbench.composer_layer_overrides.is_empty());
    let mut replay = visual_novel_engine::authoring::AuthoringDocumentSession::new(baseline);
    for command in workbench.authoring_session.recorded_commands() {
        replay.apply(command.clone()).unwrap();
    }
    assert_eq!(replay.document().composer_layer_overrides, workbench.authoring_session.document().composer_layer_overrides);
    assert_eq!(replay.document().composer_background_fit_overrides, workbench.authoring_session.document().composer_background_fit_overrides);
}

#[test]
fn gui_layer_visible_uses_document_command_bus() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let (scene, object_id) = add_scene_with_character(&mut workbench);

    workbench.handle_composer_actions(
        vec![
            crate::editor::visual_composer::VisualComposerAction::LayerVisibilityChanged {
                object_id: object_id.clone(),
                visible: false,
            },
        ],
        Some(scene),
    );

    assert_eq!(
        workbench
            .composer_layer_overrides
            .get(&object_id)
            .map(|override_| override_.visible),
        Some(false)
    );
    let entry = workbench
        .operation_log
        .last()
        .expect("visibility change should be logged by core");
    assert_eq!(entry.operation_kind, "layer_visibility_changed");
    assert!(matches!(
        entry.operation_kind_v2,
        Some(visual_novel_engine::authoring::OperationKind::LayerVisibilityChanged)
    ));
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some(format!("composer.layers[{object_id}].visible").as_str())
    );
    assert_eq!(
        workbench.verification_runs.len(),
        workbench.operation_log.len()
    );
    assert_eq!(workbench.authoring_session.undo_delta_count(), 1);
    assert!(!workbench
        .authoring_session
        .read_model()
        .composer_layer(&object_id)
        .expect("session read model should index layer")
        .visible);

    let details_ptr = workbench.operation_log[0].details.as_ptr();
    let verification_ptr = workbench.verification_runs[0].operation_id.as_ptr();
    workbench.sync_authoring_fields_from_session();
    assert_eq!(workbench.operation_log[0].details.as_ptr(), details_ptr);
    assert_eq!(workbench.verification_runs[0].operation_id.as_ptr(), verification_ptr);
    workbench.operation_log.push(workbench.operation_log[0].clone());
    workbench.verification_runs.push(workbench.verification_runs[0].clone());
    workbench.sync_authoring_fields_from_session();
    assert_eq!(workbench.operation_log.len(), 1);
    assert_eq!(workbench.verification_runs.len(), 1);
    assert_eq!(workbench.operation_log[0].details.as_ptr(), details_ptr);
    workbench.operation_log[0].details.push_str(" stale");
    workbench.verification_runs[0].diagnostic_ids.push("stale".into());
    workbench.sync_authoring_fields_from_session();
    assert_eq!(workbench.operation_log, workbench.authoring_session.document().operation_log);
    assert_eq!(workbench.verification_runs, workbench.authoring_session.document().verification_runs);
}

#[test]
fn gui_layer_locked_uses_document_command_bus() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let (scene, object_id) = add_scene_with_character(&mut workbench);

    workbench.handle_composer_actions(
        vec![
            crate::editor::visual_composer::VisualComposerAction::LayerLockChanged {
                object_id: object_id.clone(),
                locked: true,
            },
        ],
        Some(scene),
    );

    assert_eq!(
        workbench
            .composer_layer_overrides
            .get(&object_id)
            .map(|override_| override_.locked),
        Some(true)
    );
    let entry = workbench
        .operation_log
        .last()
        .expect("lock change should be logged by core");
    assert_eq!(entry.operation_kind, "layer_lock_changed");
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some(format!("composer.layers[{object_id}].locked").as_str())
    );
}

#[test]
fn gui_document_commands_reuse_live_authoring_session() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let (scene, object_id) = add_scene_with_character(&mut workbench);
    let before = workbench.node_graph.clone();

    workbench.handle_composer_actions(
        vec![
            crate::editor::visual_composer::VisualComposerAction::LayerVisibilityChanged {
                object_id: object_id.clone(),
                visible: false,
            },
        ],
        Some(scene),
    );
    workbench.commit_modified_graph(before);
    assert_eq!(workbench.authoring_session.undo_delta_count(), 1);
    assert_eq!(workbench.operation_log.len(), 1);
    let details_ptr = workbench.operation_log[0].details.as_ptr();
    let verification_ptr = workbench.verification_runs[0].operation_id.as_ptr();
    workbench.handle_composer_actions(
        vec![crate::editor::visual_composer::VisualComposerAction::LayerLockChanged {
            object_id: object_id.clone(),
            locked: true,
        }],
        Some(scene),
    );

    assert_eq!(workbench.authoring_session.undo_delta_count(), 2);
    assert_eq!(workbench.authoring_session.recorded_commands().len(), 2);
    assert_eq!(workbench.operation_log[0].details.as_ptr(), details_ptr);
    assert_eq!(workbench.verification_runs[0].operation_id.as_ptr(), verification_ptr);
    let layer = workbench
        .authoring_session
        .read_model()
        .composer_layer(&object_id)
        .expect("session read model should keep composer layer indexed");
    assert!(!layer.visible);
    assert!(layer.locked);
    assert_eq!(
        workbench.current_authoring_document().operation_log.len(),
        workbench.operation_log.len()
    );

    // A legacy graph edit must join the document history without discarding
    // the preceding visibility and lock commands.
    let before_edit = workbench.node_graph.clone();
    let added = workbench.node_graph.add_node(StoryNode::End, egui::pos2(80.0, 120.0));
    workbench.commit_modified_graph(before_edit);
    assert_eq!(workbench.authoring_session.undo_delta_count(), 3);
    let edited = workbench.node_graph.authoring_graph().clone();
    use visual_novel_engine::authoring::AuthoringDocumentCommand;
    workbench.node_graph.set_single_selection(Some(added));
    workbench.node_graph.dragging_node = Some(added);
    assert!(workbench.apply_graph_undo(), "undo legacy graph edit");
    assert!(workbench.node_graph.get_node(added).is_none());
    assert!(workbench.node_graph.selected.is_none());
    assert!(workbench.node_graph.selected_nodes.is_empty());
    assert!(workbench.node_graph.dragging_node.is_none());
    assert!(workbench.composer_layer_overrides[&object_id].locked);
    assert!(workbench.apply_graph_undo(), "undo lock");
    assert!(!workbench.composer_layer_overrides[&object_id].locked);
    assert!(!workbench.composer_layer_overrides[&object_id].visible);
    assert!(workbench.apply_graph_undo(), "undo visibility");
    assert!(!workbench.composer_layer_overrides.contains_key(&object_id));
    for _ in 0..3 {
        assert!(workbench.apply_graph_redo(), "redo mixed document and graph edits");
    }
    assert!(workbench.node_graph.authoring_graph().has_same_content(&edited));
    assert!(workbench.composer_layer_overrides[&object_id].locked);
    assert!(!workbench.composer_layer_overrides[&object_id].visible);

    workbench
        .apply_authoring_document_command(AuthoringDocumentCommand::RevertLast)
        .unwrap();
    // Deferred actions may apply a typed command before the frame commits its
    // earlier graph edit. Reconcile that edit first, preserving chronological order.
    let uncommitted = workbench.node_graph.add_node(StoryNode::End, egui::pos2(300.0, 120.0));
    workbench
        .apply_authoring_document_command(AuthoringDocumentCommand::SetLayerLocked {
            object_id: object_id.clone(),
            locked: false,
        })
        .unwrap();
    assert_eq!(workbench.authoring_session.undo_delta_count(), 4);
    assert_eq!(workbench.authoring_session.redo_delta_count(), 0);
    workbench
        .apply_authoring_document_command(AuthoringDocumentCommand::RevertLast)
        .unwrap();
    assert!(workbench.node_graph.get_node(uncommitted).is_some());
    assert!(workbench.composer_layer_overrides[&object_id].locked);
    workbench
        .apply_authoring_document_command(AuthoringDocumentCommand::RevertLast)
        .unwrap();
    assert!(workbench.node_graph.get_node(uncommitted).is_none());

    let before_external = workbench.current_authoring_fingerprint();
    workbench.record_editor_operation_now(
        "asset_imported",
        "Recorded an external manifest change",
        Some("manifest.assets".to_string()),
        None,
        None,
        before_external,
    );
    assert_eq!(workbench.authoring_session.undo_delta_count(), 2);
    assert_eq!(workbench.authoring_session.redo_delta_count(), 2);
    workbench
        .apply_authoring_document_command(AuthoringDocumentCommand::RedoLast)
        .unwrap();
    assert!(workbench.node_graph.get_node(uncommitted).is_some());
    assert_eq!(workbench.operation_log.len(), workbench.verification_runs.len());
}

#[test]
fn gui_background_fit_uses_document_command_bus() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let (scene, _) = add_scene_with_character(&mut workbench);
    let before_script = serde_json::to_value(workbench.node_graph.to_script())
        .expect("before script should serialize");

    workbench.handle_composer_actions(
        vec![
            crate::editor::visual_composer::VisualComposerAction::BackgroundFitChanged {
                node_id: Some(scene),
                fit: crate::editor::BackgroundFit::Contain,
            },
        ],
        Some(scene),
    );

    assert_eq!(
        workbench
            .composer_background_fit_overrides
            .get(&scene.to_string()),
        Some(&crate::editor::BackgroundFit::Contain)
    );
    assert_eq!(
        serde_json::to_value(workbench.node_graph.to_script())
            .expect("after script should serialize"),
        before_script
    );
    let entry = workbench
        .operation_log
        .last()
        .expect("background fit should be logged by core");
    assert_eq!(entry.operation_kind, "field_edited");
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some(format!("composer.background_fit[{scene}]").as_str())
    );
}

#[test]
fn gui_document_history_limits_undo_and_preserves_redo_order() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    for index in 0..55 {
        let before = workbench.node_graph.clone();
        workbench.node_graph.add_node(StoryNode::End, egui::pos2(index as f32, 0.0));
        workbench.commit_modified_graph(before);
    }
    assert_eq!(workbench.authoring_session.undo_delta_count(), crate::editor::undo::MAX_UNDO_STATES);
    for _ in 0..50 {
        assert!(workbench.apply_graph_undo());
    }
    assert_eq!(workbench.node_graph.len(), 5);
    assert!(!workbench.apply_graph_undo());
    for _ in 0..50 {
        assert!(workbench.apply_graph_redo());
    }
    assert_eq!(workbench.node_graph.len(), 55);
    assert!(!workbench.apply_graph_redo());
    assert!(workbench.apply_graph_undo());
    let before = workbench.node_graph.clone();
    workbench.node_graph.add_node(StoryNode::End, egui::pos2(900.0, 0.0));
    workbench.commit_modified_graph(before);
    assert!(!workbench.apply_graph_redo());
}

#[test]
fn gui_document_command_preserves_preview_behavior() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let (scene, object_id) = add_scene_with_character(&mut workbench);

    workbench.handle_composer_actions(
        vec![
            crate::editor::visual_composer::VisualComposerAction::LayerVisibilityChanged {
                object_id: object_id.clone(),
                visible: false,
            },
        ],
        Some(scene),
    );

    let document = workbench.current_authoring_document();
    let mut objects = visual_novel_engine::authoring::composer::list_layered_objects(
        &document.graph,
        Some(scene),
    );
    visual_novel_engine::authoring::composer::apply_layer_overrides(
        &mut objects,
        &document.composer_layer_overrides,
    );
    let character = objects
        .into_iter()
        .find(|object| object.object_id == object_id)
        .expect("character layer");
    assert!(!character.visible);
}
