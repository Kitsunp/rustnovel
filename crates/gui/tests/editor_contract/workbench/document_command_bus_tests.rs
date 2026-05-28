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

    workbench.handle_composer_actions(
        vec![
            crate::editor::visual_composer::VisualComposerAction::LayerVisibilityChanged {
                object_id: object_id.clone(),
                visible: false,
            },
        ],
        Some(scene),
    );
    workbench.handle_composer_actions(
        vec![crate::editor::visual_composer::VisualComposerAction::LayerLockChanged {
            object_id: object_id.clone(),
            locked: true,
        }],
        Some(scene),
    );

    assert_eq!(workbench.authoring_session.undo_delta_count(), 2);
    assert_eq!(workbench.authoring_session.recorded_commands().len(), 2);
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
