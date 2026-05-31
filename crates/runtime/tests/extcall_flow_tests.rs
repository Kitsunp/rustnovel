use std::collections::BTreeMap;

use visual_novel_engine::{
    runtime::{DialogueRaw, Engine, EventCompiled, EventRaw, ExternalCallOutcome, ScriptRaw},
    ResourceLimiter, SecurityPolicy, VnError,
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
fn runtime_advance_reports_pending_ext_call_until_host_completes_it() {
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
    let err = app
        .handle_action(InputAction::Advance)
        .expect_err("advance should wait for host completion");
    assert!(
        matches!(
            err,
            VnError::ExternalCallPending {
                event_ip: 0,
                ref command,
            } if command == "minigame.open"
        ),
        "advance should report the pending external call with traceable context"
    );

    let current = app.engine().current_event().expect("current event");
    assert!(
        matches!(
            current,
            EventCompiled::ExtCall {
                command,
                args,
            } if command == "minigame.open" && args == vec!["cards".to_string()]
        ),
        "runtime should not move past an external call before the host reports an outcome"
    );

    app.complete_external_call(ExternalCallOutcome::succeeded(0, "minigame.open"))
        .expect("complete extcall");

    let current = app.engine().current_event().expect("current event");
    assert!(
        matches!(current, EventCompiled::Dialogue(_)),
        "host completion should move to the next event"
    );
}
