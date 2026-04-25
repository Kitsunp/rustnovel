use std::collections::BTreeMap;

use visual_novel_engine::{
    AssetId, AudioActionRaw, CharacterPatchRaw, CharacterPlacementRaw, Engine, EventRaw,
    ScenePatchRaw, SceneUpdateRaw, ScriptRaw, SecurityPolicy,
};

#[test]
fn ext_call_requires_resume_to_advance() {
    let events = vec![
        EventRaw::ExtCall {
            command: "minigame_start".to_string(),
            args: vec!["poker".to_string()],
        },
        EventRaw::Dialogue(visual_novel_engine::DialogueRaw {
            speaker: "Ava".to_string(),
            text: "Hola".to_string(),
        }),
    ];
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    let script = ScriptRaw::new(events, labels);
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        visual_novel_engine::ResourceLimiter::default(),
    )
    .unwrap();

    let (_audio, change) = engine.step().unwrap();
    assert!(matches!(
        change.event,
        visual_novel_engine::EventCompiled::ExtCall { .. }
    ));
    let event = engine.current_event().unwrap();
    assert!(matches!(
        event,
        visual_novel_engine::EventCompiled::ExtCall { .. }
    ));

    engine.resume().unwrap();
    let event = engine.current_event().unwrap();
    assert!(matches!(
        event,
        visual_novel_engine::EventCompiled::Dialogue(_)
    ));
}

#[test]
fn peek_next_assets_collects_scene_assets() {
    let events = vec![EventRaw::Scene(SceneUpdateRaw {
        background: Some("bg/room.png".to_string()),
        music: Some("music/theme.ogg".to_string()),
        characters: vec![CharacterPlacementRaw {
            name: "Ava".to_string(),
            expression: Some("smile".to_string()),
            position: Some("center".to_string()),
            x: None,
            y: None,
            scale: None,
        }],
    })];
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    let script = ScriptRaw::new(events, labels);
    let engine = Engine::new(
        script,
        SecurityPolicy::default(),
        visual_novel_engine::ResourceLimiter::default(),
    )
    .unwrap();

    let assets = engine.peek_next_assets(1);
    assert!(assets.contains(&AssetId::from_path("bg/room.png")));
    assert!(assets.contains(&AssetId::from_path("music/theme.ogg")));
    assert!(assets.contains(&AssetId::from_path("Ava")));
    assert!(assets.contains(&AssetId::from_path("smile")));
}

#[test]
fn peek_next_asset_paths_returns_prefetchable_paths_only() {
    let events = vec![
        EventRaw::Scene(SceneUpdateRaw {
            background: Some("bg/room.png".to_string()),
            music: Some("music/theme.ogg".to_string()),
            characters: vec![CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("sprites/ava_smile.png".to_string()),
                position: Some("center".to_string()),
                x: None,
                y: None,
                scale: None,
            }],
        }),
        EventRaw::Patch(ScenePatchRaw {
            background: None,
            music: None,
            add: vec![CharacterPlacementRaw {
                name: "Narrator".to_string(),
                expression: Some("sprites/narrator.png".to_string()),
                position: Some("left".to_string()),
                x: None,
                y: None,
                scale: None,
            }],
            update: vec![CharacterPatchRaw {
                name: "Ava".to_string(),
                expression: Some("sprites/ava_focus.png".to_string()),
                position: None,
            }],
            remove: Vec::new(),
        }),
        EventRaw::AudioAction(AudioActionRaw {
            channel: "bgm".to_string(),
            action: "play".to_string(),
            asset: Some("audio/ambient.ogg".to_string()),
            volume: Some(0.8),
            fade_duration_ms: Some(200),
            loop_playback: Some(true),
        }),
    ];
    let labels = BTreeMap::from([("start".to_string(), 0)]);
    let script = ScriptRaw::new(events, labels);
    let engine = Engine::new(
        script,
        SecurityPolicy::default(),
        visual_novel_engine::ResourceLimiter::default(),
    )
    .unwrap();

    let paths = engine.peek_next_asset_paths(3);
    assert!(paths.contains(&"bg/room.png".to_string()));
    assert!(paths.contains(&"music/theme.ogg".to_string()));
    assert!(paths.contains(&"sprites/ava_smile.png".to_string()));
    assert!(paths.contains(&"sprites/narrator.png".to_string()));
    assert!(paths.contains(&"sprites/ava_focus.png".to_string()));
    assert!(paths.contains(&"audio/ambient.ogg".to_string()));
    assert!(!paths.contains(&"Ava".to_string()));
    assert!(!paths.contains(&"Narrator".to_string()));
}
