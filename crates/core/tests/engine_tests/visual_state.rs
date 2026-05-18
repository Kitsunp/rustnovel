use super::*;

#[test]
fn scene_updates_visual_state_and_renderer_output() {
    let script = sample_script();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    let scene = engine.step_event().unwrap();
    assert!(matches!(scene, EventCompiled::Scene(_)));
    let visual = engine.visual_state();
    assert_eq!(visual.background.as_deref(), Some("bg/room.png"));
    assert_eq!(visual.music.as_deref(), Some("music/theme.ogg"));
    assert_eq!(visual.characters.len(), 1);

    let renderer = TextRenderer;
    let output = renderer.render(&scene, visual);
    assert!(output.text.contains("Background: bg/room.png"));
    assert!(output.text.contains("Characters: Ava (smile) @ center"));
}

#[test]
fn scene_patch_add_preserves_distinct_character_instances_by_expression() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: None,
                music: None,
                characters: vec![CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    expression: Some("smile".to_string()),
                    position: None,
                    x: Some(10),
                    y: Some(20),
                    scale: Some(1.0),
                }],
            }),
            EventRaw::Patch(ScenePatchRaw {
                add: vec![CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    expression: Some("angry".to_string()),
                    position: None,
                    x: Some(300),
                    y: Some(20),
                    scale: Some(1.0),
                }],
                ..Default::default()
            }),
            EventRaw::Patch(ScenePatchRaw {
                add: vec![CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    expression: Some("angry".to_string()),
                    position: None,
                    x: Some(360),
                    y: Some(40),
                    scale: Some(1.2),
                }],
                ..Default::default()
            }),
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");

    let _ = engine.step().expect("scene");
    let _ = engine.step().expect("patch add");
    let visual = engine.visual_state();
    assert_eq!(visual.characters.len(), 2);
    assert!(visual.characters.iter().any(|character| {
        character.name.as_ref() == "Ava"
            && character.expression.as_deref() == Some("smile")
            && character.x == Some(10)
    }));
    assert!(visual.characters.iter().any(|character| {
        character.name.as_ref() == "Ava"
            && character.expression.as_deref() == Some("angry")
            && character.x == Some(300)
    }));

    let _ = engine.step().expect("patch update same instance");
    let visual = engine.visual_state();
    assert_eq!(visual.characters.len(), 2);
    assert!(visual.characters.iter().any(|character| {
        character.name.as_ref() == "Ava"
            && character.expression.as_deref() == Some("angry")
            && character.x == Some(360)
            && character.y == Some(40)
            && character.scale == Some(1.2)
    }));
}

#[test]
fn engine_emits_audio_command_on_scene_start() {
    let script = sample_script();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    let (audio_commands, change) = engine.step().unwrap();
    assert!(matches!(change.event, EventCompiled::Scene(_)));
    assert!(audio_commands
        .iter()
        .any(|command| matches!(command, visual_novel_engine::AudioCommand::PlayBgm { .. })));
}

#[test]
fn ambiguous_character_update_and_position_do_not_cross_update_duplicates() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: None,
                music: None,
                characters: vec![
                    CharacterPlacementRaw {
                        name: "Ava".to_string(),
                        expression: Some("smile".to_string()),
                        position: Some("left".to_string()),
                        x: Some(10),
                        y: Some(20),
                        scale: Some(1.0),
                    },
                    CharacterPlacementRaw {
                        name: "Ava".to_string(),
                        expression: Some("angry".to_string()),
                        position: Some("right".to_string()),
                        x: Some(300),
                        y: Some(20),
                        scale: Some(1.0),
                    },
                ],
            }),
            EventRaw::Patch(ScenePatchRaw {
                update: vec![CharacterPatchRaw {
                    name: "Ava".to_string(),
                    expression: Some("sad".to_string()),
                    position: Some("center".to_string()),
                }],
                ..Default::default()
            }),
            EventRaw::SetCharacterPosition(SetCharacterPositionRaw {
                name: "Ava".to_string(),
                x: 640,
                y: 360,
                scale: Some(2.0),
            }),
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");

    let _ = engine.step().expect("scene");
    let _ = engine.step().expect("ambiguous patch update");
    let _ = engine.step().expect("ambiguous position update");

    let visual = engine.visual_state();
    assert_eq!(visual.characters.len(), 2);
    assert!(visual.characters.iter().any(|character| {
        character.name.as_ref() == "Ava"
            && character.expression.as_deref() == Some("smile")
            && character.position.as_deref() == Some("left")
            && character.x == Some(10)
            && character.y == Some(20)
            && character.scale == Some(1.0)
    }));
    assert!(visual.characters.iter().any(|character| {
        character.name.as_ref() == "Ava"
            && character.expression.as_deref() == Some("angry")
            && character.position.as_deref() == Some("right")
            && character.x == Some(300)
            && character.y == Some(20)
            && character.scale == Some(1.0)
    }));
    assert!(!visual.characters.iter().any(|character| {
        character.expression.as_deref() == Some("sad")
            || character.x == Some(640)
            || character.y == Some(360)
    }));
}

#[test]
fn single_character_update_and_position_still_apply() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: None,
                music: None,
                characters: vec![CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    expression: Some("smile".to_string()),
                    position: Some("left".to_string()),
                    x: Some(10),
                    y: Some(20),
                    scale: Some(1.0),
                }],
            }),
            EventRaw::Patch(ScenePatchRaw {
                update: vec![CharacterPatchRaw {
                    name: "Ava".to_string(),
                    expression: Some("sad".to_string()),
                    position: Some("center".to_string()),
                }],
                ..Default::default()
            }),
            EventRaw::SetCharacterPosition(SetCharacterPositionRaw {
                name: "Ava".to_string(),
                x: 640,
                y: 360,
                scale: Some(2.0),
            }),
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");

    let _ = engine.step().expect("scene");
    let _ = engine.step().expect("patch update");
    let _ = engine.step().expect("position update");

    let character = engine
        .visual_state()
        .characters
        .first()
        .expect("single character");
    assert_eq!(character.expression.as_deref(), Some("sad"));
    assert_eq!(character.position.as_deref(), Some("center"));
    assert_eq!(character.x, Some(640));
    assert_eq!(character.y, Some(360));
    assert_eq!(character.scale, Some(2.0));
}
