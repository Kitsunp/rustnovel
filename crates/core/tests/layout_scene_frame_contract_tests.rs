use visual_novel_engine::{
    resolve_layout,
    runtime::{
        CharacterPlacementRaw, ChoiceOptionRaw, ChoiceRaw, DialogueRaw, Engine, EventRaw,
        SceneUpdateRaw, ScriptRaw,
    },
    validate_ui_theme, ComponentRegistry, DisplayOrientation, DisplayProfile, LayoutPolicy,
    RenderCommand, SafeAreaInsets, SecurityPolicy, StageProfile, UiTheme, WindowMode,
};

use std::collections::BTreeMap;

#[test]
fn layout_resolves_required_viewport_matrix_without_invalid_geometry() {
    let cases = [
        (320.0, 240.0, 1.0),
        (800.0, 600.0, 1.0),
        (1280.0, 720.0, 1.0),
        (1920.0, 1080.0, 2.0),
        (3440.0, 1440.0, 1.5),
        (720.0, 1280.0, 3.0),
    ];
    for (width, height, scale_factor) in cases {
        for user_scale in [0.75, 1.0, 1.25, 1.5, 2.0, 3.0] {
            let display = DisplayProfile {
                logical_size: [width, height],
                physical_size: [
                    (width * scale_factor) as u32,
                    (height * scale_factor) as u32,
                ],
                dpi: None,
                ppi: None,
                tpi: None,
                scale_factor,
                user_scale,
                safe_area: SafeAreaInsets::default(),
                window_mode: WindowMode::Windowed,
                orientation: if width >= height {
                    DisplayOrientation::Landscape
                } else {
                    DisplayOrientation::Portrait
                },
            };
            let resolved =
                resolve_layout(display, StageProfile::default(), LayoutPolicy::default());
            assert!(resolved.stage_rect.width.is_finite());
            assert!(resolved.stage_rect.height.is_finite());
            assert!(resolved.stage_rect.width > 0.0);
            assert!(resolved.stage_rect.height > 0.0);
        }
    }
}

#[test]
fn default_theme_validates_as_tokenized_theme() {
    let report = validate_ui_theme(&UiTheme::default());
    assert!(report.valid, "theme errors: {:?}", report.errors);
    for id in ComponentRegistry::REQUIRED_GAME_COMPONENTS {
        assert!(
            UiTheme::default().components.components.contains_key(*id),
            "default component registry missing {id}"
        );
    }
}

#[test]
fn theme_validation_reports_missing_required_components_without_blocking_custom_skin() {
    let mut theme = UiTheme::default();
    theme.components.components.remove("route_tree");
    let report = validate_ui_theme(&theme);
    assert!(report.valid, "warnings should not invalidate a usable skin");
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("route_tree")));
}

#[test]
fn scene_frame_emits_real_visual_and_dialogue_commands() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: Some("bg/room.png".to_string()),
                music: None,
                characters: vec![CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    expression: Some("sprites/ava.png".to_string()),
                    position: Some("center".to_string()),
                    x: None,
                    y: None,
                    scale: None,
                }],
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Ava".to_string(),
                text: "Ready".to_string(),
            }),
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        visual_novel_engine::ResourceLimiter::default(),
    )
    .expect("engine");

    let scene_frame = engine.scene_frame();
    assert!(scene_frame.commands.iter().any(|command| matches!(
        command,
        RenderCommand::Image { asset, .. } if asset == "bg/room.png"
    )));
    assert!(scene_frame.commands.iter().any(|command| matches!(
        command,
        RenderCommand::Image { asset, .. } if asset == "sprites/ava.png"
    )));

    engine.step().expect("step scene");
    let dialogue_frame = engine.scene_frame();
    assert!(dialogue_frame.commands.iter().any(|command| matches!(
        command,
        RenderCommand::Panel { style, .. } if style == "dialogue_box"
    )));
    assert!(dialogue_frame.commands.iter().any(|command| matches!(
        command,
        RenderCommand::Text { text, style, .. } if text == "Ready" && style == "dialogue.text"
    )));
    assert!(dialogue_frame
        .interactions
        .iter()
        .any(|interaction| interaction.action == "advance"));
}

#[test]
fn scene_frame_emits_choice_buttons_and_actions() {
    let script = ScriptRaw::new(
        vec![EventRaw::Choice(ChoiceRaw {
            prompt: "Where?".to_string(),
            options: vec![
                ChoiceOptionRaw {
                    text: "Left".to_string(),
                    target: "left".to_string(),
                },
                ChoiceOptionRaw {
                    text: "Right".to_string(),
                    target: "right".to_string(),
                },
            ],
        })],
        BTreeMap::from([
            ("start".to_string(), 0),
            ("left".to_string(), 1),
            ("right".to_string(), 1),
        ]),
    );
    let engine = Engine::new(
        script,
        SecurityPolicy::default(),
        visual_novel_engine::ResourceLimiter::default(),
    )
    .expect("engine");

    let frame = engine.scene_frame();
    assert_eq!(
        frame
            .commands
            .iter()
            .filter(|command| matches!(command, RenderCommand::Button { .. }))
            .count(),
        2
    );
    assert_eq!(
        frame
            .interactions
            .iter()
            .map(|interaction| interaction.action.as_str())
            .collect::<Vec<_>>(),
        vec!["choose:0", "choose:1"]
    );
}
