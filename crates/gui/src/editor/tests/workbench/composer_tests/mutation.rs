use super::*;

#[test]
fn composer_scene_patch_character_image_position_survives_sync() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let patch = workbench.node_graph.add_node(
        StoryNode::ScenePatch(visual_novel_engine::ScenePatchRaw {
            add: vec![visual_novel_engine::CharacterPlacementRaw {
                name: "furina".to_string(),
                expression: Some("assets/characters/furina.png".to_string()),
                position: None,
                x: Some(240),
                y: Some(120),
                scale: Some(1.0),
            }],
            ..Default::default()
        }),
        egui::pos2(0.0, 120.0),
    );
    workbench.node_graph.connect(start, patch);

    assert!(workbench.apply_composer_node_mutation(
        patch,
        crate::editor::visual_composer::ComposerNodeMutation::CharacterPosition {
            name: "furina".to_string(),
            expression: Some("assets/characters/furina.png".to_string()),
            source_instance_index: 0,
            x: 480,
            y: 260,
            scale: Some(1.2),
        },
    ));
    workbench
        .sync_graph_to_script()
        .expect("patch should compile after composer mutation");

    let script = workbench
        .current_script
        .as_ref()
        .expect("script should be available");
    let visual_novel_engine::EventRaw::Patch(saved_patch) = &script.events[0] else {
        panic!("expected patch event");
    };
    let character = saved_patch.add.first().expect("character should remain");
    assert_eq!(
        character.expression.as_deref(),
        Some("assets/characters/furina.png")
    );
    assert_eq!(character.x, Some(480));
    assert_eq!(character.y, Some(260));
    assert_eq!(character.scale, Some(1.2));
}

#[test]
fn composer_mutation_targets_duplicate_character_by_expression_and_instance() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: vec![
                visual_novel_engine::CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    expression: Some("ava/smile.png".to_string()),
                    position: None,
                    x: Some(10),
                    y: Some(10),
                    scale: Some(1.0),
                },
                visual_novel_engine::CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    expression: Some("ava/angry.png".to_string()),
                    position: None,
                    x: Some(200),
                    y: Some(10),
                    scale: Some(1.0),
                },
                visual_novel_engine::CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    expression: Some("ava/angry.png".to_string()),
                    position: None,
                    x: Some(300),
                    y: Some(10),
                    scale: Some(1.0),
                },
            ],
        },
        egui::pos2(0.0, 0.0),
    );

    assert!(workbench.apply_composer_node_mutation(
        scene,
        crate::editor::visual_composer::ComposerNodeMutation::CharacterPosition {
            name: "Ava".to_string(),
            expression: Some("ava/angry.png".to_string()),
            source_instance_index: 1,
            x: 640,
            y: 360,
            scale: Some(1.25),
        },
    ));

    let Some(StoryNode::Scene { characters, .. }) = workbench.node_graph.get_node(scene) else {
        panic!("expected scene node");
    };
    assert_eq!(characters[0].x, Some(10));
    assert_eq!(characters[1].x, Some(200));
    assert_eq!(characters[2].x, Some(640));
    assert_eq!(characters[2].y, Some(360));
    assert_eq!(characters[2].scale, Some(1.25));
}

#[test]
fn composer_drag_operation_records_node_before_after_values() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: vec![visual_novel_engine::CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("ava/smile.png".to_string()),
                position: None,
                x: Some(10),
                y: Some(10),
                scale: Some(1.0),
            }],
        },
        egui::pos2(0.0, 0.0),
    );
    workbench.node_graph.clear_operation_hint();
    workbench.node_graph.clear_modified();
    workbench.refresh_operation_fingerprint();
    let before = workbench.node_graph.clone();

    assert!(workbench.apply_composer_node_mutation(
        scene,
        crate::editor::visual_composer::ComposerNodeMutation::CharacterPosition {
            name: "Ava".to_string(),
            expression: Some("ava/smile.png".to_string()),
            source_instance_index: 0,
            x: 320,
            y: 240,
            scale: Some(1.2),
        },
    ));
    workbench.node_graph.mark_modified();
    workbench.commit_modified_graph(before);

    let entry = workbench
        .operation_log
        .last()
        .expect("composer drag should be logged");
    assert_eq!(entry.operation_kind, "composer_object_moved");
    assert!(matches!(
        entry.operation_kind_v2,
        Some(visual_novel_engine::authoring::OperationKind::ComposerObjectMoved)
    ));
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some("graph.nodes[0].visual.transform")
    );
    assert!(entry
        .before_value
        .as_deref()
        .is_some_and(|value| value.contains("\"x\":10")));
    assert!(entry
        .after_value
        .as_deref()
        .is_some_and(|value| value.contains("\"x\":320")));
}

#[test]
fn composer_overlay_dialogue_edit_updates_node_and_operation_trace() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Old speaker".to_string(),
            text: "Old text".to_string(),
        },
        egui::pos2(0.0, 0.0),
    );
    workbench.node_graph.clear_operation_hint();
    workbench.node_graph.clear_modified();
    workbench.refresh_operation_fingerprint();
    let before = workbench.node_graph.clone();

    assert!(workbench.apply_composer_node_mutation(
        dialogue,
        crate::editor::visual_composer::ComposerNodeMutation::DialogueText {
            speaker: "Sakura".to_string(),
            text: "A visible edit from the Composer overlay.".to_string(),
        },
    ));
    workbench.node_graph.mark_modified();
    workbench.commit_modified_graph(before);

    let Some(StoryNode::Dialogue { speaker, text }) = workbench.node_graph.get_node(dialogue)
    else {
        panic!("expected dialogue node");
    };
    assert_eq!(speaker, "Sakura");
    assert_eq!(text, "A visible edit from the Composer overlay.");

    let entry = workbench
        .operation_log
        .last()
        .expect("dialogue overlay edit should be logged");
    assert_eq!(entry.operation_kind, "field_edited");
    assert!(matches!(
        entry.operation_kind_v2,
        Some(visual_novel_engine::authoring::OperationKind::FieldEdited)
    ));
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some("graph.nodes[0].dialogue")
    );
    assert!(entry
        .before_value
        .as_deref()
        .is_some_and(|value| value.contains("Old speaker")));
    assert!(entry
        .after_value
        .as_deref()
        .is_some_and(|value| value.contains("Sakura")));
}

#[test]
fn composer_choice_reorder_preserves_option_target_pairs() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let choice = workbench.node_graph.add_node(
        StoryNode::Choice {
            prompt: "Where next?".to_string(),
            options: vec![
                "Left route".to_string(),
                "Center route".to_string(),
                "Right route".to_string(),
            ],
        },
        egui::pos2(0.0, 0.0),
    );
    let left = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Left".to_string(),
            text: "Left target".to_string(),
        },
        egui::pos2(-120.0, 90.0),
    );
    let center = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Center".to_string(),
            text: "Center target".to_string(),
        },
        egui::pos2(0.0, 90.0),
    );
    let right = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Right".to_string(),
            text: "Right target".to_string(),
        },
        egui::pos2(120.0, 90.0),
    );
    workbench.node_graph.connect_port(choice, 0, left);
    workbench.node_graph.connect_port(choice, 1, center);
    workbench.node_graph.connect_port(choice, 2, right);
    workbench.node_graph.clear_operation_hint();
    workbench.node_graph.clear_modified();
    workbench.refresh_operation_fingerprint();
    let before = workbench.node_graph.clone();

    assert!(workbench.apply_composer_node_mutation(
        choice,
        crate::editor::visual_composer::ComposerNodeMutation::ChoiceOptionOrder {
            from_index: 0,
            to_index: 2,
        },
    ));
    workbench.node_graph.mark_modified();
    workbench.commit_modified_graph(before);

    let Some(StoryNode::Choice { options, .. }) = workbench.node_graph.get_node(choice) else {
        panic!("expected choice node");
    };
    assert_eq!(
        options,
        &vec![
            "Center route".to_string(),
            "Right route".to_string(),
            "Left route".to_string()
        ]
    );
    let mut targets = workbench
        .node_graph
        .connections()
        .filter(|conn| conn.from == choice)
        .map(|conn| (conn.from_port, conn.to))
        .collect::<Vec<_>>();
    targets.sort_unstable();
    assert_eq!(targets, vec![(0, center), (1, right), (2, left)]);

    let entry = workbench
        .operation_log
        .last()
        .expect("choice reorder should be logged");
    assert_eq!(entry.operation_kind, "field_edited");
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some("graph.nodes[0].choice.options")
    );
}
