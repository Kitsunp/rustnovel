use super::super::*;
use crate::editor::StoryNode;

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
fn composer_runtime_preview_can_start_from_selected_node_and_advance() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let first = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "First".to_string(),
        },
        egui::pos2(0.0, 100.0),
    );
    let second = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Second".to_string(),
        },
        egui::pos2(0.0, 200.0),
    );
    workbench.node_graph.connect(start, first);
    workbench.node_graph.connect(first, second);
    workbench.selected_node = Some(second);

    workbench.start_composer_runtime_preview_from_selection();
    let engine = workbench.engine.as_ref().expect("engine should start");
    let event = engine
        .current_event()
        .expect("selected event should be current");
    assert!(matches!(
        event,
        visual_novel_engine::EventCompiled::Dialogue(dialogue)
            if dialogue.text.as_ref() == "Second"
    ));

    workbench.advance_composer_runtime_preview(None);
    assert!(workbench
        .engine
        .as_ref()
        .and_then(|engine| engine.current_event().ok())
        .is_none());
}

#[test]
fn composer_runtime_preview_uses_live_graph_selection_when_workbench_selection_is_stale() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let first = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "First".to_string(),
        },
        egui::pos2(0.0, 100.0),
    );
    let second = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Second".to_string(),
        },
        egui::pos2(0.0, 200.0),
    );
    workbench.node_graph.connect(start, first);
    workbench.node_graph.connect(first, second);
    workbench.node_graph.set_single_selection(Some(second));
    workbench.selected_node = Some(first);

    workbench.start_composer_runtime_preview_from_node(workbench.node_graph.selected);

    let event = workbench
        .engine
        .as_ref()
        .and_then(|engine| engine.current_event().ok())
        .expect("live graph selection should be current");
    assert!(matches!(
        event,
        visual_novel_engine::EventCompiled::Dialogue(dialogue)
            if dialogue.text.as_ref() == "Second"
    ));
    assert_eq!(workbench.selected_node, Some(second));
}

#[test]
fn composer_runtime_preview_can_start_from_selected_choice_and_choose_route() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let choice = workbench.node_graph.add_node(
        StoryNode::Choice {
            prompt: "Where now?".to_string(),
            options: vec!["Library".to_string(), "Garden".to_string()],
        },
        egui::pos2(0.0, 100.0),
    );
    let library = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Library route".to_string(),
        },
        egui::pos2(-120.0, 220.0),
    );
    let garden = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Garden route".to_string(),
        },
        egui::pos2(120.0, 220.0),
    );
    workbench.node_graph.connect(start, choice);
    workbench.node_graph.connect_port(choice, 0, library);
    workbench.node_graph.connect_port(choice, 1, garden);
    workbench.selected_node = Some(choice);

    workbench.start_composer_runtime_preview_from_selection();
    let event = workbench
        .engine
        .as_ref()
        .and_then(|engine| engine.current_event().ok())
        .expect("selected choice should be current");
    assert!(matches!(
        event,
        visual_novel_engine::EventCompiled::Choice(choice)
            if choice.prompt.as_ref() == "Where now?" && choice.options.len() == 2
    ));

    workbench.advance_composer_runtime_preview(Some(1));
    let event = workbench
        .engine
        .as_ref()
        .and_then(|engine| engine.current_event().ok())
        .expect("chosen route should become current");
    assert!(matches!(
        event,
        visual_novel_engine::EventCompiled::Dialogue(dialogue)
            if dialogue.text.as_ref() == "Garden route"
    ));
}

#[test]
fn composer_owner_map_does_not_treat_dialogue_speaker_as_visual_character() {
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
                x: Some(300),
                y: Some(200),
                scale: Some(1.0),
            }],
        },
        egui::pos2(0.0, 0.0),
    );
    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "This speaker is not a visual placement".to_string(),
        },
        egui::pos2(0.0, 100.0),
    );
    workbench.selected_node = Some(scene);
    workbench.refresh_scene_from_selected_node();
    workbench.composer_entity_owners.clear();

    let owners = workbench.build_entity_node_map();
    let character_entity = workbench
        .scene
        .iter()
        .find_map(|entity| match &entity.kind {
            visual_novel_engine::EntityKind::Character(character)
                if character.name.as_ref() == "Ava" =>
            {
                Some(entity.id.raw())
            }
            _ => None,
        })
        .expect("character entity should exist");

    assert_eq!(owners.get(&character_entity), Some(&scene));
    assert_ne!(owners.get(&character_entity), Some(&dialogue));
}

#[test]
fn composer_owner_map_keeps_duplicate_character_instances_separate() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let first = workbench.node_graph.add_node(
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
    let second = workbench.node_graph.add_node(
        StoryNode::ScenePatch(visual_novel_engine::ScenePatchRaw {
            add: vec![visual_novel_engine::CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("ava/angry.png".to_string()),
                position: None,
                x: Some(200),
                y: Some(10),
                scale: Some(1.0),
            }],
            ..Default::default()
        }),
        egui::pos2(0.0, 120.0),
    );
    workbench.scene = visual_novel_engine::SceneState::new();
    let _ = workbench.scene.spawn_with_transform(
        visual_novel_engine::Transform::at(10, 10),
        visual_novel_engine::EntityKind::Character(visual_novel_engine::CharacterData {
            name: visual_novel_engine::SharedStr::from("Ava"),
            expression: Some(visual_novel_engine::SharedStr::from("ava/smile.png")),
        }),
    );
    let _ = workbench.scene.spawn_with_transform(
        visual_novel_engine::Transform::at(200, 10),
        visual_novel_engine::EntityKind::Character(visual_novel_engine::CharacterData {
            name: visual_novel_engine::SharedStr::from("Ava"),
            expression: Some(visual_novel_engine::SharedStr::from("ava/angry.png")),
        }),
    );

    let owners = workbench.build_entity_node_map();
    let mut pairs = workbench
        .scene
        .iter()
        .filter_map(|entity| match &entity.kind {
            visual_novel_engine::EntityKind::Character(character) => Some((
                character.expression.as_deref().map(str::to_string),
                entity.id.raw(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    pairs.sort_by_key(|(expression, _)| expression.clone());

    for (expression, entity_id) in pairs {
        match expression.as_deref() {
            Some("ava/angry.png") => assert_eq!(owners.get(&entity_id), Some(&second)),
            Some("ava/smile.png") => assert_eq!(owners.get(&entity_id), Some(&first)),
            other => panic!("unexpected expression {other:?}"),
        }
    }
}

#[test]
fn engine_preview_owner_hints_keep_duplicate_character_nodes_separate() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let first = workbench.node_graph.add_node(
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
        egui::pos2(0.0, 100.0),
    );
    let second = workbench.node_graph.add_node(
        StoryNode::ScenePatch(visual_novel_engine::ScenePatchRaw {
            add: vec![visual_novel_engine::CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("ava/angry.png".to_string()),
                position: None,
                x: Some(200),
                y: Some(10),
                scale: Some(1.0),
            }],
            ..Default::default()
        }),
        egui::pos2(0.0, 220.0),
    );
    workbench.node_graph.connect(start, first);
    workbench.node_graph.connect(first, second);
    workbench.selected_node = Some(second);

    workbench
        .sync_graph_to_script()
        .expect("preview graph should compile");

    let owners_by_expression = workbench
        .scene
        .iter()
        .filter_map(|entity| match &entity.kind {
            visual_novel_engine::EntityKind::Character(character) => Some((
                character.expression.as_deref().map(str::to_string),
                workbench
                    .composer_entity_owners
                    .get(&entity.id.raw())
                    .copied(),
            )),
            _ => None,
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    assert_eq!(
        owners_by_expression.get(&Some("ava/smile.png".to_string())),
        Some(&Some(first))
    );
    assert_eq!(
        owners_by_expression.get(&Some("ava/angry.png".to_string())),
        Some(&Some(second))
    );
}

#[test]
fn layered_scene_objects_include_runtime_overlays_and_source_paths() {
    let mut scene = visual_novel_engine::SceneState::new();
    let mut bg_transform = visual_novel_engine::Transform::at(0, 0);
    bg_transform.z_order = -100;
    let bg_id = scene
        .spawn_with_transform(
            bg_transform,
            visual_novel_engine::EntityKind::Image(visual_novel_engine::ImageData {
                path: visual_novel_engine::SharedStr::from("bg/room.png"),
                tint: None,
            }),
        )
        .expect("bg entity");
    let script = visual_novel_engine::ScriptRaw::new(
        vec![visual_novel_engine::EventRaw::Dialogue(
            visual_novel_engine::DialogueRaw {
                speaker: "Ava".to_string(),
                text: "Hola".to_string(),
            },
        )],
        std::collections::BTreeMap::from([("start".to_string(), 0usize)]),
    );
    let engine = Some(
        visual_novel_engine::Engine::new(
            script,
            visual_novel_engine::SecurityPolicy::default(),
            visual_novel_engine::ResourceLimiter::default(),
        )
        .expect("engine"),
    );
    let owners = std::collections::HashMap::from([(bg_id.raw(), 42u32)]);

    let objects = crate::editor::visual_composer::layered_scene_objects(&scene, &owners, &engine);

    assert!(objects.iter().any(|object| {
        object.kind == crate::editor::visual_composer::StageLayerKind::Background
            && object.source_field_path.contains("graph.nodes[42]")
    }));
    assert!(objects.iter().any(|object| {
        object.kind == crate::editor::visual_composer::StageLayerKind::DialogueUi && object.locked
    }));
}

#[test]
fn layered_scene_objects_include_selected_authoring_choice_overlay_without_runtime() {
    let scene = visual_novel_engine::SceneState::new();
    let owners = std::collections::HashMap::new();
    let selected = StoryNode::Choice {
        prompt: "Where?".to_string(),
        options: vec!["A".to_string(), "B".to_string()],
    };

    let objects = crate::editor::visual_composer::layered_scene_objects_with_authoring_overlay(
        &scene,
        &owners,
        &None,
        Some(7),
        Some(&selected),
    );

    let overlay = objects
        .iter()
        .find(|object| object.object_id == "overlay:choice")
        .expect("selected choice should be exposed as a layer");
    assert_eq!(
        overlay.kind,
        crate::editor::visual_composer::StageLayerKind::InteractionUi
    );
    assert_eq!(overlay.source_node_id, Some(7));
    assert_eq!(overlay.source_field_path, "graph.nodes[7].choice");
    assert!(overlay.locked);
}

#[test]
fn selected_authoring_overlay_replaces_same_runtime_overlay_layer() {
    let scene = visual_novel_engine::SceneState::new();
    let script = visual_novel_engine::ScriptRaw::new(
        vec![
            visual_novel_engine::EventRaw::Choice(visual_novel_engine::ChoiceRaw {
                prompt: "Runtime choice".to_string(),
                options: vec![visual_novel_engine::ChoiceOptionRaw {
                    text: "Continue".to_string(),
                    target: "end".to_string(),
                }],
            }),
            visual_novel_engine::EventRaw::Dialogue(visual_novel_engine::DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "Done".to_string(),
            }),
        ],
        std::collections::BTreeMap::from([
            ("start".to_string(), 0usize),
            ("end".to_string(), 1usize),
        ]),
    );
    let engine = Some(
        visual_novel_engine::Engine::new(
            script,
            visual_novel_engine::SecurityPolicy::default(),
            visual_novel_engine::ResourceLimiter::default(),
        )
        .expect("engine"),
    );
    let selected = StoryNode::Choice {
        prompt: "Selected choice".to_string(),
        options: vec!["A".to_string()],
    };

    let objects = crate::editor::visual_composer::layered_scene_objects_with_authoring_overlay(
        &scene,
        &std::collections::HashMap::new(),
        &engine,
        Some(11),
        Some(&selected),
    );
    let overlays = objects
        .iter()
        .filter(|object| object.object_id == "overlay:choice")
        .collect::<Vec<_>>();

    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].source_node_id, Some(11));
    assert_eq!(overlays[0].source_field_path, "graph.nodes[11].choice");
}

#[test]
fn composer_layer_object_ids_match_stage_override_ids() {
    let mut scene = visual_novel_engine::SceneState::new();
    let entity_id = scene
        .spawn_with_transform(
            visual_novel_engine::Transform::at(15, 25),
            visual_novel_engine::EntityKind::Character(visual_novel_engine::CharacterData {
                name: visual_novel_engine::SharedStr::from("Ava"),
                expression: Some(visual_novel_engine::SharedStr::from("ava/smile.png")),
            }),
        )
        .expect("character entity");
    let owners = std::collections::HashMap::from([(entity_id.raw(), 7u32)]);

    let objects = crate::editor::visual_composer::layered_scene_objects(&scene, &owners, &None);
    let entity = scene.get(entity_id).expect("entity should exist");
    let expected_id = crate::editor::visual_composer::scene_entity_object_id(entity, Some(7), 0);

    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].object_id, expected_id);
}
