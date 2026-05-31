use std::collections::BTreeMap;

use visual_novel_engine::{
    resolve_layout,
    runtime::{
        CharacterPlacementRaw, DialogueRaw, Engine, EventRaw, SceneFramePresenter, SceneUpdateRaw,
        ScriptRaw, UiView,
    },
    DisplayProfile, LayoutPolicy, ResourceLimiter, SecurityPolicy, StageProfile, UiTheme,
};
use vnengine_runtime::{
    AssetStore, Audio, Input, InputAction, RuntimeApp, RuntimeSceneFramePresenter,
};

#[derive(Default)]
struct NullInput;

impl Input for NullInput {
    fn handle_window_event(&mut self, _event: &winit::event::WindowEvent) -> InputAction {
        InputAction::None
    }
}

#[derive(Default)]
struct MemoryAssets;

impl AssetStore for MemoryAssets {
    fn load_bytes(&self, id: &str) -> Result<Vec<u8>, String> {
        Ok(id.as_bytes().to_vec())
    }
}

#[derive(Default)]
struct SilentAudio;

impl Audio for SilentAudio {
    fn play_music(&mut self, _id: &str) -> Result<(), String> {
        Ok(())
    }

    fn stop_music(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn play_sfx(&mut self, _id: &str) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn runtime_app_exposes_scene_frame_as_primary_render_contract() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: Some("bg/runtime.png".to_string()),
                music: None,
                characters: vec![CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    expression: Some("sprites/ava.png".to_string()),
                    position: None,
                    x: None,
                    y: None,
                    scale: None,
                }],
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Ava".to_string(),
                text: "Runtime frame".to_string(),
            }),
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");
    let mut app =
        RuntimeApp::new(engine, NullInput, SilentAudio, MemoryAssets).expect("runtime app");

    assert!(app.scene_frame().commands.iter().any(|command| matches!(
        command,
        visual_novel_engine::RenderCommand::Image { asset, .. } if asset == "bg/runtime.png"
    )));

    app.handle_action(InputAction::Advance)
        .expect("advance scene");
    assert!(app
        .scene_frame()
        .interactions
        .iter()
        .any(|interaction| interaction.action == "advance"));
}

#[test]
fn runtime_app_reports_invalid_scene_frame_actions() {
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Ava".to_string(),
            text: "Ready".to_string(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");
    let mut app =
        RuntimeApp::new(engine, NullInput, SilentAudio, MemoryAssets).expect("runtime app");

    let err = app
        .handle_action(InputAction::InvalidSceneAction)
        .expect_err("invalid scene-frame action should fail");
    assert!(
        err.to_string().contains("unsupported or malformed"),
        "unexpected error: {err}"
    );
}

#[test]
fn runtime_app_reports_unimplemented_navigation_actions() {
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Ava".to_string(),
            text: "Ready".to_string(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");
    let mut app =
        RuntimeApp::new(engine, NullInput, SilentAudio, MemoryAssets).expect("runtime app");

    for (action, label) in [(InputAction::Back, "back"), (InputAction::Menu, "menu")] {
        let err = app
            .handle_action(action)
            .expect_err("unimplemented action should fail visibly");
        assert!(
            err.to_string().contains(label) && err.to_string().contains("not implemented"),
            "unexpected error for {label}: {err}"
        );
    }
}

#[test]
fn runtime_app_advance_past_final_event_reports_end_state_without_error() {
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Ava".to_string(),
            text: "Done".to_string(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");
    let mut app =
        RuntimeApp::new(engine, NullInput, SilentAudio, MemoryAssets).expect("runtime app");

    assert!(app
        .handle_action(InputAction::Advance)
        .expect("advance past final event should not error"));
    assert!(matches!(
        &app.ui().view,
        UiView::System { message } if message == "End of script"
    ));
}

#[test]
fn runtime_scene_frame_presenter_counts_render_commands() {
    let script = ScriptRaw::new(
        vec![EventRaw::Scene(SceneUpdateRaw {
            background: Some("bg/runtime.png".to_string()),
            music: None,
            characters: Vec::new(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");
    let display = DisplayProfile::new(800.0, 600.0);
    let mut frame = engine.scene_frame();
    frame.layout = Some(resolve_layout(
        display.clone(),
        StageProfile::default(),
        LayoutPolicy::default(),
    ));
    let mut presenter = RuntimeSceneFramePresenter::default();
    let response = presenter.present(&frame, &display, &UiTheme::default());

    assert!(response.diagnostics.is_empty());
    assert_eq!(presenter.last_command_count, frame.commands.len());
    assert!(presenter.last_image_count >= 1);
}
