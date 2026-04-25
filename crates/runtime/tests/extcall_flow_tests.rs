use std::collections::BTreeMap;

use visual_novel_engine::{
    DialogueRaw, Engine, EventRaw, ResourceLimiter, ScriptRaw, SecurityPolicy,
};
use vnengine_runtime::{AssetStore, Audio, Input, InputAction, RuntimeApp};

#[derive(Default)]
struct NullInput;

impl Input for NullInput {
    fn handle_window_event(&mut self, _event: &winit::event::WindowEvent) -> InputAction {
        InputAction::None
    }
}

#[derive(Default)]
struct NullAssets;

impl AssetStore for NullAssets {
    fn load_bytes(&self, _id: &str) -> Result<Vec<u8>, String> {
        Err("NullAssets".to_string())
    }
}

#[derive(Default)]
struct SilentAudio;

impl Audio for SilentAudio {
    fn play_music(&mut self, _id: &str) {}
    fn stop_music(&mut self) {}
    fn play_sfx(&mut self, _id: &str) {}
}

#[test]
fn runtime_advance_resumes_ext_call_without_stalling() {
    let events = vec![
        EventRaw::ExtCall {
            command: "minigame.open".to_string(),
            args: vec!["cards".to_string()],
        },
        EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "Back from minigame".to_string(),
        }),
    ];
    let labels = BTreeMap::from([("start".to_string(), 0)]);
    let script = ScriptRaw::new(events, labels);
    let engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");

    let mut app = RuntimeApp::new(engine, NullInput, SilentAudio, NullAssets).expect("runtime");
    app.handle_action(InputAction::Advance).expect("advance");

    let current = app.engine().current_event().expect("current event");
    assert!(
        matches!(current, visual_novel_engine::EventCompiled::Dialogue(_)),
        "advance should resume ext_call and move to next event"
    );
}
