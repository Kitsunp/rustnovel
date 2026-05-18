use super::*;

#[test]
fn scene_preview_audio_stop_clears_music_entity() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/one.png".to_string()),
            music: Some("audio/theme.ogg".to_string()),
            characters: Vec::new(),
        },
        egui::pos2(0.0, 100.0),
    );
    let stop_bgm = workbench.node_graph.add_node(
        StoryNode::AudioAction {
            channel: "bgm".to_string(),
            action: "stop".to_string(),
            asset: None,
            volume: None,
            fade_duration_ms: None,
            loop_playback: None,
        },
        egui::pos2(0.0, 200.0),
    );
    workbench.node_graph.connect(start, scene);
    workbench.node_graph.connect(scene, stop_bgm);

    workbench
        .sync_graph_to_script()
        .expect("graph should compile for preview");
    workbench.selected_node = Some(stop_bgm);
    workbench.refresh_scene_from_engine_preview();

    assert!(
        !workbench
            .scene
            .iter()
            .any(|entity| matches!(entity.kind, visual_novel_engine::EntityKind::Audio(_))),
        "audio stop node should clear preview audio entity"
    );
}

#[test]
fn scene_preview_builds_owner_map_for_core_entities() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/owner.png".to_string()),
            music: Some("audio/owner.ogg".to_string()),
            characters: vec![visual_novel_engine::CharacterPlacementRaw {
                name: "sylvie".to_string(),
                expression: Some("pose/default.png".to_string()),
                position: Some("center".to_string()),
                x: Some(500),
                y: Some(350),
                scale: Some(1.0),
            }],
        },
        egui::pos2(0.0, 100.0),
    );
    workbench.node_graph.connect(start, scene);
    workbench
        .sync_graph_to_script()
        .expect("scene graph should compile");

    let tracked = workbench
        .scene
        .iter()
        .filter(|entity| {
            matches!(
                entity.kind,
                visual_novel_engine::EntityKind::Image(_)
                    | visual_novel_engine::EntityKind::Character(_)
                    | visual_novel_engine::EntityKind::Audio(_)
            )
        })
        .count();
    assert!(
        tracked >= 3,
        "preview should include image, character and audio entities"
    );
    for entity in workbench.scene.iter() {
        match entity.kind {
            visual_novel_engine::EntityKind::Image(_)
            | visual_novel_engine::EntityKind::Character(_)
            | visual_novel_engine::EntityKind::Audio(_) => {
                assert_eq!(
                    workbench.composer_entity_owners.get(&entity.id.raw()),
                    Some(&scene),
                    "entity {} should map back to source scene node",
                    entity.id.raw()
                );
            }
            _ => {}
        }
    }
}

#[test]
fn composer_mutation_updates_node_character_position() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: vec![visual_novel_engine::CharacterPlacementRaw {
                name: "hero".to_string(),
                expression: None,
                position: Some("center".to_string()),
                x: Some(100),
                y: Some(120),
                scale: Some(1.0),
            }],
        },
        egui::pos2(0.0, 0.0),
    );

    let changed = workbench.apply_composer_node_mutation(
        scene,
        crate::editor::visual_composer::ComposerNodeMutation::CharacterPosition {
            name: "hero".to_string(),
            expression: None,
            source_instance_index: 0,
            x: 640,
            y: 360,
            scale: Some(1.25),
        },
    );
    assert!(changed, "mutation should modify source node");

    let Some(StoryNode::Scene { characters, .. }) = workbench.node_graph.get_node(scene) else {
        panic!("expected scene node");
    };
    let character = characters
        .iter()
        .find(|entry| entry.name == "hero")
        .expect("character should exist");
    assert_eq!(character.x, Some(640));
    assert_eq!(character.y, Some(360));
    assert_eq!(character.scale, Some(1.25));
}

#[test]
fn fallback_entity_owner_map_prefers_scene_music_owner_over_audio_action_override() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: Some("audio/theme.ogg".to_string()),
            characters: Vec::new(),
        },
        egui::pos2(0.0, 100.0),
    );
    let audio = workbench.node_graph.add_node(
        StoryNode::AudioAction {
            channel: "bgm".to_string(),
            action: "play".to_string(),
            asset: Some("audio/theme.ogg".to_string()),
            volume: None,
            fade_duration_ms: None,
            loop_playback: Some(true),
        },
        egui::pos2(0.0, 200.0),
    );
    workbench.node_graph.connect(start, scene);
    workbench.node_graph.connect(scene, audio);

    workbench
        .sync_graph_to_script()
        .expect("graph should compile for preview");
    workbench.composer_entity_owners.clear();

    let owners = workbench.build_entity_node_map();
    let audio_entity = workbench
        .scene
        .iter()
        .find_map(|entity| match &entity.kind {
            visual_novel_engine::EntityKind::Audio(audio_data)
                if audio_data.path.as_ref() == "audio/theme.ogg" =>
            {
                Some(entity.id.raw())
            }
            _ => None,
        })
        .expect("audio entity should exist");

    assert_eq!(
        owners.get(&audio_entity),
        Some(&scene),
        "scene node should remain canonical owner for shared scene music entity"
    );
}
