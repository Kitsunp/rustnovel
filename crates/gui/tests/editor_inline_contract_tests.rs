use std::collections::BTreeMap;

use eframe::egui;
use visual_novel_engine::authoring::AuthoringPosition;
use visual_novel_engine::runtime::{
    CharacterPlacementRaw, ChoiceOptionRaw, ChoiceRaw, DialogueRaw, EventRaw, SceneUpdateRaw,
    ScriptRaw,
};
use visual_novel_gui::editor::authoring_adapter::replace_gui_semantics_from_authoring;
use visual_novel_gui::editor::execution_contract::{
    contract_for_event_raw, contract_for_node, contract_matrix, is_preview_only_node, FidelityClass,
};
use visual_novel_gui::editor::image_asset_cache::{
    scene_stage_cache_key, should_retry_missing_image_failure,
};
use visual_novel_gui::editor::inspector_panel::graph_summary_lines;
use visual_novel_gui::editor::inspector_panel::node_editor::parse_generic_event_json;
use visual_novel_gui::editor::lint_panel::diagnostic_docs_url;
use visual_novel_gui::editor::node_types::{ContextMenu, StoryNode, StoryNodeVisualExt};
use visual_novel_gui::editor::script_sync::{from_script, to_script};
use visual_novel_gui::editor::{NodeGraph, PreviewQuality};
use visual_novel_gui::{AssetManager, AssetStore, SecurityMode};

fn write_png(path: &std::path::Path) {
    let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([12, 34, 56, 255]));
    image.save(path).expect("write png");
}

#[test]
fn texture_for_asset_reuses_normalized_cache_key() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();
    std::fs::create_dir_all(root.join("assets/bg")).expect("asset dir");
    write_png(&root.join("assets/bg/portrait.png"));

    let store =
        AssetStore::new(root.to_path_buf(), SecurityMode::Trusted, None, false).expect("store");
    let mut manager = AssetManager::new(store, 8 * 1024 * 1024);
    let ctx = egui::Context::default();

    let first = manager
        .texture_for_asset(&ctx, "bg/portrait")
        .expect("first lookup")
        .expect("texture should load");
    let stats_after_first = manager.stats();
    assert_eq!(stats_after_first.misses, 1);
    assert_eq!(stats_after_first.hits, 0);

    let second = manager
        .texture_for_asset(&ctx, "assets/bg/portrait.png")
        .expect("second lookup")
        .expect("texture should load");
    let stats_after_second = manager.stats();
    assert_eq!(stats_after_second.misses, 1);
    assert_eq!(stats_after_second.hits, 1);
    assert_eq!(first.id(), second.id());
}

#[test]
fn candidate_paths_include_normalized_and_assets_prefix() {
    let candidates =
        visual_novel_gui::editor::asset_candidates::candidate_asset_paths(" bg\\theme ", &["ogg"]);
    assert_eq!(
        candidates,
        vec![
            "bg/theme".to_string(),
            "assets/bg/theme".to_string(),
            "bg/theme.ogg".to_string(),
            "assets/bg/theme.ogg".to_string(),
        ]
    );
}

#[test]
fn candidate_paths_append_extensions_without_duplicates() {
    let candidates = visual_novel_gui::editor::asset_candidates::candidate_asset_paths(
        "audio/theme",
        &["ogg", "wav"],
    );
    assert_eq!(
        candidates,
        vec![
            "audio/theme".to_string(),
            "assets/audio/theme".to_string(),
            "audio/theme.ogg".to_string(),
            "audio/theme.wav".to_string(),
            "assets/audio/theme.ogg".to_string(),
            "assets/audio/theme.wav".to_string(),
        ]
    );
}

#[test]
fn adapter_preserves_view_state_while_replacing_semantics() {
    let mut graph = NodeGraph::new();
    let old = graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    graph.selected = Some(old);
    graph.selected_nodes.insert(old);
    graph.pan = egui::vec2(8.0, 9.0);
    graph.zoom = 1.7;

    let mut next = visual_novel_engine::authoring::NodeGraph::new();
    let scene = next.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: None,
            characters: vec![CharacterPlacementRaw {
                name: "Ava".to_string(),
                ..Default::default()
            }],
        },
        AuthoringPosition::new(4.0, 5.0),
    );

    replace_gui_semantics_from_authoring(&mut graph, &next);

    assert_eq!(graph.selected, None);
    assert!(graph.selected_nodes.is_empty());
    assert_eq!(graph.pan, egui::vec2(8.0, 9.0));
    assert_eq!(graph.zoom, 1.7);
    assert!(matches!(
        graph.get_node(scene),
        Some(StoryNode::Scene { .. })
    ));
}

#[test]
fn execution_contract_matrix_contains_all_fidelity_classes() {
    assert!(contract_matrix()
        .iter()
        .any(|entry| entry.fidelity == FidelityClass::PreviewOnly));
    assert!(contract_matrix()
        .iter()
        .any(|entry| entry.fidelity == FidelityClass::RuntimeReal));
    assert!(contract_matrix()
        .iter()
        .any(|entry| entry.fidelity == FidelityClass::FallbackDegraded));
}

#[test]
fn execution_contract_classifies_story_markers_and_extcall() {
    assert!(is_preview_only_node(&StoryNode::Start));
    assert!(is_preview_only_node(&StoryNode::End));

    let dialogue = contract_for_event_raw(&EventRaw::Dialogue(DialogueRaw {
        speaker: "A".to_string(),
        text: "B".to_string(),
    }));
    assert_eq!(dialogue.event_name, "Dialogue");
    assert_eq!(dialogue.fidelity, FidelityClass::RuntimeReal);
    assert!(dialogue.export_supported);

    let extcall = contract_for_node(&StoryNode::Generic(EventRaw::ExtCall {
        command: "hook".to_string(),
        args: vec!["x".to_string()],
    }));
    assert!(extcall.export_supported);
    assert_eq!(extcall.fidelity, FidelityClass::RuntimeReal);
}

#[test]
fn image_retry_policy_waits_for_real_candidates() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let failure = "missing image: image asset not found";

    assert!(!should_retry_missing_image_failure(
        failure,
        root,
        "assets/backgrounds/room.png"
    ));

    std::fs::create_dir_all(root.join("assets/backgrounds")).expect("mkdir assets");
    std::fs::write(root.join("assets/backgrounds/room.png"), b"not-a-real-png")
        .expect("write file");

    assert!(should_retry_missing_image_failure(
        failure,
        root,
        "assets/backgrounds/room.png"
    ));
    assert!(!should_retry_missing_image_failure(
        "image 'assets/backgrounds/room.png' load failed: decode",
        root,
        "assets/backgrounds/room.png"
    ));
}

#[test]
fn image_cache_key_tracks_resolved_candidate_contents() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("assets/backgrounds")).expect("mkdir assets");
    let asset = temp.path().join("assets/backgrounds/room.png");
    std::fs::write(&asset, b"first").expect("write first asset");
    let before = scene_stage_cache_key(temp.path(), PreviewQuality::Draft, "backgrounds/room");

    std::fs::write(&asset, b"second-version").expect("write second asset");
    let after = scene_stage_cache_key(temp.path(), PreviewQuality::Draft, "backgrounds/room");

    assert_ne!(before, after);
    assert!(!before.ends_with("::missing"));
}

#[test]
fn image_cache_key_dedupes_after_asset_store_resolution() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("assets/backgrounds")).expect("mkdir assets");
    std::fs::write(
        temp.path().join("assets/backgrounds/room.png"),
        b"placeholder",
    )
    .expect("write asset");
    let store = vnengine_assets::AssetStore::new(
        temp.path().to_path_buf(),
        vnengine_assets::SecurityMode::Trusted,
        None,
        false,
    )
    .expect("asset store");

    let resolved_short = store
        .resolve_image_path("backgrounds/room")
        .expect("short path should resolve");
    let resolved_full = store
        .resolve_image_path("assets/backgrounds/room.png")
        .expect("full path should resolve");

    assert_eq!(resolved_short, resolved_full);
    assert_eq!(
        scene_stage_cache_key(temp.path(), PreviewQuality::Draft, &resolved_short),
        scene_stage_cache_key(temp.path(), PreviewQuality::Draft, &resolved_full)
    );
}

#[test]
fn inspector_and_lint_contract_helpers_are_environment_independent() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let end = graph.add_node(StoryNode::End, egui::pos2(0.0, 120.0));
    graph.connect(start, end);
    graph.toggle_multi_selection(start);
    graph.toggle_multi_selection(end);
    assert!(graph.create_fragment_from_selection("intro", "Intro"));
    assert!(graph.enter_fragment("intro"));

    let lines = graph_summary_lines(&graph);
    assert!(lines.contains(&"Nodes: 2".to_string()));
    assert!(lines.contains(&"Connections: 1".to_string()));
    assert!(lines.contains(&"Selected: 2 nodes (0, 1)".to_string()));
    assert!(lines.contains(&"Active fragment: intro".to_string()));
    assert!(lines.contains(&"Fragments: 1".to_string()));

    let url = diagnostic_docs_url("docs/diagnostics/authoring.md#val-asset-not-found");
    assert!(url.starts_with("file:///"), "{url}");
    assert!(url.contains("/docs/diagnostics/authoring.md#val-asset-not-found"));
    assert!(!url.contains('\\'));
}

#[test]
fn generic_event_json_parser_accepts_extcall_and_rejects_unknown_payload() {
    let event = parse_generic_event_json(
        r#"{"type":"ext_call","command":"show_overlay","args":["inventory"]}"#,
    )
    .expect("valid ext call");

    match event {
        EventRaw::ExtCall { command, args } => {
            assert_eq!(command, "show_overlay");
            assert_eq!(args, vec!["inventory".to_string()]);
        }
        _ => panic!("expected ext call"),
    }
    assert!(parse_generic_event_json(r#"{"type":"unknown"}"#).is_err());
}

#[test]
fn node_type_contracts_come_from_core_with_gui_visual_extension() {
    assert_eq!(StoryNode::Start.type_name(), "Start");
    assert_eq!(StoryNode::End.type_name(), "End");
    assert!(StoryNode::Start.can_connect_from());
    assert!(!StoryNode::Start.can_connect_to());
    assert!(!StoryNode::End.can_connect_from());
    assert!(StoryNode::End.can_connect_to());
    assert_eq!(StoryNode::Start.icon(), ">");
    assert_eq!(
        StoryNode::Dialogue {
            speaker: "A".to_string(),
            text: "B".to_string(),
        }
        .color(),
        egui::Color32::from_rgb(60, 80, 100)
    );
}

#[test]
fn context_menu_distinguishes_node_and_canvas_targets() {
    let node_menu = ContextMenu::for_node(7, egui::pos2(10.0, 20.0));
    assert_eq!(node_menu.node_id, Some(7));
    assert_eq!(node_menu.graph_position, None);

    let canvas_menu = ContextMenu::for_canvas(egui::pos2(10.0, 20.0), egui::pos2(30.0, 40.0));
    assert_eq!(canvas_menu.node_id, None);
    assert_eq!(canvas_menu.graph_position, Some(egui::pos2(30.0, 40.0)));
}

#[test]
fn script_sync_roundtrips_empty_dialogue_and_scene_scripts() {
    let empty = ScriptRaw::new(vec![], BTreeMap::new());
    assert!(to_script(&from_script(&empty)).events.is_empty());

    let dialogue = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Alice".to_string(),
            text: "Hello, world!".to_string(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let dialogue_roundtrip = to_script(&from_script(&dialogue));
    assert!(!dialogue_roundtrip.events.is_empty());
    assert!(dialogue_roundtrip.labels.contains_key("start"));

    let scene_script = ScriptRaw::new(
        vec![EventRaw::Scene(SceneUpdateRaw {
            background: Some("bg/room.png".to_string()),
            music: Some("bgm/theme.ogg".to_string()),
            characters: vec![CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("smile".to_string()),
                position: Some("left".to_string()),
                x: Some(10),
                y: Some(20),
                scale: Some(1.2),
            }],
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let roundtrip = to_script(&from_script(&scene_script));
    let Some(EventRaw::Scene(scene)) = roundtrip.events.first() else {
        panic!("Expected first event to be scene");
    };
    assert_eq!(scene.background.as_deref(), Some("bg/room.png"));
    assert_eq!(scene.music.as_deref(), Some("bgm/theme.ogg"));
    assert_eq!(scene.characters[0].name, "Ava");
}

#[test]
fn script_sync_preserves_choice_exit_contracts() {
    let mut graph = NodeGraph::new();
    let start_id = graph.add_node(StoryNode::Start, egui::pos2(50.0, 30.0));
    let choice_id = graph.add_node(
        StoryNode::Choice {
            prompt: "Elige".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
        },
        egui::pos2(100.0, 120.0),
    );
    graph.connect(start_id, choice_id);

    let script = to_script(&graph);
    let Some(EventRaw::Choice(choice)) = script.events.first() else {
        panic!("Expected first event to be choice");
    };
    assert_eq!(choice.options.len(), 2);
    assert!(choice.options[0].target.starts_with("__unlinked_node_"));
    assert!(choice.options[1].target.starts_with("__unlinked_node_"));

    let end_choice = ScriptRaw::new(
        vec![EventRaw::Choice(ChoiceRaw {
            prompt: "Salir?".to_string(),
            options: vec![ChoiceOptionRaw {
                text: "Fin".to_string(),
                target: "__end".to_string(),
            }],
        })],
        BTreeMap::from([("start".to_string(), 0), ("__end".to_string(), 1)]),
    );
    let roundtrip = to_script(&from_script(&end_choice));
    let Some(EventRaw::Choice(choice)) = roundtrip.events.first() else {
        panic!("Expected first event to be choice");
    };
    assert_eq!(choice.options[0].target, "__end");
    assert!(roundtrip.compile().is_ok());
}
