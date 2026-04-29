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
