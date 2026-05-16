use std::fs;

use super::super::*;
use crate::editor::StoryNode;
use tempfile::tempdir;

#[test]
fn load_project_with_status_reports_error_in_silent_mode() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let missing_manifest = std::path::PathBuf::from("__missing__/project.vnm");

    let result = workbench.load_project_with_status(missing_manifest, false);
    assert!(result.is_err());
    assert!(
        workbench.toast.is_none(),
        "silent mode should not overwrite UI toast state"
    );
}

#[test]
fn load_project_with_status_reports_error_with_toast() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let missing_manifest = std::path::PathBuf::from("__missing__/project.vnm");

    let result = workbench.load_project_with_status(missing_manifest, true);
    assert!(result.is_err());
    let message = workbench
        .toast
        .as_ref()
        .map(|toast| toast.message.clone())
        .unwrap_or_default();
    assert!(
        message.contains("Failed to load project"),
        "error toast should expose load failure"
    );
}

#[test]
fn load_project_with_status_can_open_recently_imported_renpy_project() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let dir = tempfile::tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    std::fs::create_dir_all(&game_dir).expect("mkdir game");
    std::fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    "Hello from import"
"#,
    )
    .expect("write script");

    let output_root = dir.path().join("imported_out");
    visual_novel_engine::import_renpy_project(visual_novel_engine::ImportRenpyOptions {
        project_root,
        output_root: output_root.clone(),
        entry_label: "start".to_string(),
        report_path: None,
        profile: visual_novel_engine::ImportProfile::StoryFirst,
        include_tl: None,
        include_ui: None,
        include_patterns: Vec::new(),
        exclude_patterns: Vec::new(),
        strict_mode: false,
        fallback_policy: visual_novel_engine::ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import renpy");

    let manifest_path = output_root.join("project.vnm");
    workbench
        .load_project_with_status(manifest_path, false)
        .expect("workbench should load imported project");
    assert!(workbench.current_script.is_some(), "script must be loaded");
    assert!(
        workbench
            .node_graph
            .nodes()
            .any(|(_, node, _)| matches!(node, StoryNode::Dialogue { .. })),
        "graph should contain imported dialogue node"
    );
    assert!(
        workbench.engine.is_some(),
        "player engine should be initialized"
    );
    assert!(
        workbench
            .engine
            .as_ref()
            .and_then(|engine| engine.current_event().ok())
            .is_some(),
        "engine should expose a current event after load"
    );
}

#[test]
fn load_project_with_status_ignores_locales_outside_locale_root() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("project");
    let locale_root = project_root.join("locales");
    fs::create_dir_all(&locale_root).expect("mkdir locales");

    fs::write(
        project_root.join("main.json"),
        r#"{
  "script_schema_version": "1.0",
  "events": [
    { "type": "dialogue", "speaker": "Narrator", "text": "Hola" }
  ],
  "labels": { "start": 0 }
}"#,
    )
    .expect("write script");
    fs::write(
        locale_root.join("en.json"),
        r#"{"hello":"hola","start":"inicio"}"#,
    )
    .expect("write safe locale");
    fs::write(
        project_root.join("escape.json"),
        r#"{"hello":"pwned","start":"escape"}"#,
    )
    .expect("write escaping locale");

    fs::write(
        project_root.join("project.vnm"),
        r#"
schema_version = "1.0"

[metadata]
name = "Locale Safety"
author = "QA"
version = "0.1.0"

[settings]
resolution = [1280, 720]
default_language = "en"
supported_languages = ["en", "../escape"]
entry_point = "main.json"

[assets]
"#,
    )
    .expect("write manifest");

    workbench
        .load_project_with_status(project_root.join("project.vnm"), false)
        .expect("project should load");

    assert_eq!(
        workbench.localization_catalog.locale_codes(),
        vec!["en".to_string()],
        "only locale files contained in locales/ should be loaded"
    );
    assert_eq!(workbench.localization_catalog.default_locale, "en");
    assert!(
        workbench.current_script.is_some(),
        "entry script should load"
    );
}

#[test]
fn load_standalone_script_replaces_previous_project_root_and_manifest_state() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let dir = tempdir().expect("tempdir");
    let first_root = dir.path().join("first_project");
    let second_root = dir.path().join("second_project");
    fs::create_dir_all(first_root.join("locales")).expect("mkdir first locales");
    fs::create_dir_all(second_root.join("locales")).expect("mkdir second locales");

    fs::write(
        first_root.join("main.json"),
        r#"{
  "script_schema_version": "1.0",
  "events": [
    { "type": "scene", "background": "backgrounds/old.png" }
  ],
  "labels": { "start": 0 }
}"#,
    )
    .expect("write first script");
    fs::write(
        first_root.join("project.vnm"),
        r#"
schema_version = "1.0"

[metadata]
name = "First"
author = "QA"
version = "0.1.0"

[settings]
resolution = [1280, 720]
default_language = "es"
supported_languages = ["es"]
entry_point = "main.json"

[assets]
"#,
    )
    .expect("write first manifest");
    fs::write(first_root.join("locales/es.json"), r#"{"old":"viejo"}"#)
        .expect("write first locale");

    fs::write(
        second_root.join("main.json"),
        r#"{
  "script_schema_version": "1.0",
  "events": [
    { "type": "scene", "background": "backgrounds/new.png" }
  ],
  "labels": { "start": 0 }
}"#,
    )
    .expect("write second script");
    fs::write(second_root.join("locales/fr.json"), r#"{"new":"nouveau"}"#)
        .expect("write second locale");

    workbench
        .load_project_with_status(first_root.join("project.vnm"), false)
        .expect("first project should load");
    assert_eq!(
        workbench.project_root.as_deref(),
        Some(first_root.as_path())
    );
    assert!(workbench.manifest.is_some());

    workbench.load_script(second_root.join("main.json"));

    assert_eq!(
        workbench.project_root.as_deref(),
        Some(second_root.as_path()),
        "standalone script load must not keep resolving assets against the old project"
    );
    assert!(
        workbench.manifest.is_none(),
        "standalone scripts should not keep stale manifest assets/settings from a previous project"
    );
    assert!(
        workbench.manifest_path.is_none(),
        "standalone script load should clear manifest path"
    );
    assert_eq!(
        workbench.localization_catalog.locale_codes(),
        vec!["fr".to_string()],
        "locale discovery should use the new script directory"
    );
    assert_eq!(workbench.player_locale, "fr");
    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image)
                if image.path.as_ref() == "backgrounds/new.png"
        )),
        "preview scene should come from the newly loaded script"
    );
}

#[test]
fn sync_graph_to_script_builds_non_empty_scene_preview_from_visual_events() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let prelude = workbench.node_graph.add_node(
        StoryNode::SetVariable {
            key: "intro_ready".to_string(),
            value: 1,
        },
        egui::pos2(0.0, 80.0),
    );
    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/lecturehall".to_string()),
            music: Some("assets/illurock.opus".to_string()),
            characters: vec![visual_novel_engine::CharacterPlacementRaw {
                name: "sylvie".to_string(),
                expression: Some("green".to_string()),
                position: Some("center".to_string()),
                x: Some(640),
                y: Some(480),
                scale: Some(1.0),
            }],
        },
        egui::pos2(0.0, 160.0),
    );
    workbench.node_graph.connect(start, prelude);
    workbench.node_graph.connect(prelude, scene);

    workbench
        .sync_graph_to_script()
        .expect("graph should compile for preview");

    assert!(
        !workbench.scene.is_empty(),
        "scene preview should not be empty"
    );
    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/lecturehall"
        )),
        "preview scene should include imported background image entity"
    );
    let background = workbench
        .scene
        .iter()
        .find(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/lecturehall"
        ))
        .expect("background entity must exist");
    assert_eq!(background.transform.x, 0);
    assert_eq!(background.transform.y, 0);
    assert!(background.transform.z_order <= -50);
}

#[test]
fn scene_preview_tracks_selected_node_context() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let scene_a = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/one.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 100.0),
    );
    let scene_b = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/two.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 200.0),
    );
    workbench.node_graph.connect(start, scene_a);
    workbench.node_graph.connect(scene_a, scene_b);
    workbench
        .sync_graph_to_script()
        .expect("scene chain should compile");

    workbench.selected_node = Some(scene_b);
    workbench.refresh_scene_from_engine_preview();

    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/two.png"
        )),
        "composer preview should follow selected node context"
    );
}

#[test]
fn scene_preview_reconstructs_selected_context_from_start_after_runtime_advances() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let scene_a = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/one.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 100.0),
    );
    let scene_b = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/two.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 200.0),
    );
    workbench.node_graph.connect(start, scene_a);
    workbench.node_graph.connect(scene_a, scene_b);
    workbench
        .sync_graph_to_script()
        .expect("scene chain should compile");
    workbench
        .engine
        .as_mut()
        .expect("engine should exist")
        .jump_to_label(&format!("node_{scene_b}"))
        .expect("runtime should advance to the later scene");

    workbench.selected_node = Some(scene_a);
    workbench.refresh_scene_from_engine_preview();

    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/one.png"
        )),
        "composer preview should rebuild from script start when selecting an earlier node"
    );
    assert!(
        !workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/two.png"
        )),
        "later runtime visual state must not leak into earlier selected-node preview"
    );
}

#[test]
fn scene_preview_uses_selected_branch_scene_when_default_choice_route_misses_target() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let choice = workbench.node_graph.add_node(
        StoryNode::Choice {
            prompt: "Where?".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
        },
        egui::pos2(0.0, 100.0),
    );
    let scene_a = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/a.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(-160.0, 220.0),
    );
    let scene_b = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/b.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(160.0, 220.0),
    );
    let end = workbench
        .node_graph
        .add_node(StoryNode::End, egui::pos2(0.0, 360.0));
    workbench.node_graph.connect(start, choice);
    workbench.node_graph.connect_port(choice, 0, scene_a);
    workbench.node_graph.connect_port(choice, 1, scene_b);
    workbench.node_graph.connect(scene_a, end);
    workbench.node_graph.connect(scene_b, end);
    workbench
        .sync_graph_to_script()
        .expect("branching graph should compile");

    workbench.selected_node = Some(scene_b);
    workbench.refresh_scene_from_engine_preview();

    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/b.png"
        )),
        "selected branch scene should preview bg/b.png"
    );
    assert!(
        !workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/a.png"
        )),
        "composer must not silently show the default choice branch"
    );
}

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
