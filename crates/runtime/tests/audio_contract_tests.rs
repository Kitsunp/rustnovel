use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use visual_novel_engine::{
    runtime::{
        AudioActionRaw, ChoiceOptionRaw, ChoiceRaw, DialogueRaw, Engine, EventRaw, SceneUpdateRaw,
        ScriptRaw,
    },
    ResourceLimiter, SecurityPolicy,
};
use vnengine_runtime::{AssetStore, Audio, Input, InputAction, RuntimeApp, SilentAudio};

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

#[derive(Default, Debug)]
struct AudioProbeState {
    bgm_calls: Vec<(String, bool, Option<f32>)>,
    voice_calls: Vec<(String, Option<f32>)>,
    voice_stop_calls: usize,
}

#[derive(Clone, Default)]
struct AudioProbe {
    state: Rc<RefCell<AudioProbeState>>,
}

impl Audio for AudioProbe {
    fn play_music(&mut self, id: &str) -> Result<(), String> {
        self.play_music_with_options(id, true, None)
    }

    fn play_music_with_options(
        &mut self,
        id: &str,
        loop_playback: bool,
        volume: Option<f32>,
    ) -> Result<(), String> {
        self.state
            .borrow_mut()
            .bgm_calls
            .push((id.to_string(), loop_playback, volume));
        Ok(())
    }

    fn stop_music(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn play_sfx(&mut self, _id: &str) -> Result<(), String> {
        Ok(())
    }

    fn play_voice_with_volume(&mut self, id: &str, volume: Option<f32>) -> Result<(), String> {
        self.state
            .borrow_mut()
            .voice_calls
            .push((id.to_string(), volume));
        Ok(())
    }

    fn stop_voice(&mut self) -> Result<(), String> {
        self.state.borrow_mut().voice_stop_calls += 1;
        Ok(())
    }
}

#[test]
fn audio_transition_default_preserves_loop_and_volume_options() {
    let probe_state = Rc::new(RefCell::new(AudioProbeState::default()));
    let mut probe = AudioProbe {
        state: probe_state.clone(),
    };

    probe
        .play_music_with_transition(
            "music/theme.ogg",
            false,
            Some(0.25),
            Some(Duration::from_millis(100)),
        )
        .expect("transition should delegate");

    assert_eq!(
        probe_state.borrow().bgm_calls,
        vec![("music/theme.ogg".to_string(), false, Some(0.25))]
    );
}

#[test]
fn silent_audio_declares_noop_capabilities() {
    let caps = SilentAudio.capabilities();

    assert!(caps.no_op);
    assert!(!caps.bgm_fade);
    assert!(!caps.stop_sfx);
    assert!(!caps.stop_voice);
}

#[test]
fn silent_audio_reports_playback_as_unavailable() {
    let mut audio = SilentAudio;

    let err = audio
        .play_music("music/theme.ogg")
        .expect_err("silent backend must not pretend playback happened");

    assert!(err.contains("audio backend unavailable"));
    assert!(err.contains("music/theme.ogg"));
    assert!(audio.stop_music().is_ok());
}

#[derive(Default)]
struct FailingAudio;

impl Audio for FailingAudio {
    fn play_music(&mut self, id: &str) -> Result<(), String> {
        Err(format!("cannot play {id}"))
    }

    fn stop_music(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn play_sfx(&mut self, _id: &str) -> Result<(), String> {
        Ok(())
    }
}

fn build_engine(events: Vec<EventRaw>) -> Engine {
    let script = ScriptRaw::new(events, BTreeMap::from([("start".to_string(), 0)]));
    Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine")
}

#[test]
fn runtime_propagates_audio_command_failures() {
    let events = vec![
        EventRaw::AudioAction(AudioActionRaw {
            channel: "bgm".to_string(),
            action: "play".to_string(),
            asset: Some("music/theme.ogg".to_string()),
            volume: Some(0.42),
            fade_duration_ms: Some(250),
            loop_playback: Some(false),
        }),
        EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "done".to_string(),
        }),
    ];
    let engine = build_engine(events);
    let mut app = RuntimeApp::new(engine, NullInput, FailingAudio, NullAssets).expect("runtime");

    let err = app
        .handle_action(InputAction::Advance)
        .expect_err("audio backend failure must propagate");
    let err = err.to_string();

    assert!(err.contains("audio command"));
    assert!(err.contains("play_bgm"));
    assert!(err.contains("music/theme.ogg"));
    assert!(err.contains("cannot play music/theme.ogg"));
}

fn build_engine_with_labels(events: Vec<EventRaw>, labels: BTreeMap<String, usize>) -> Engine {
    let script = ScriptRaw::new(events, labels);
    Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine")
}

#[test]
fn runtime_applies_bgm_loop_and_volume_from_audio_command() {
    let events = vec![
        EventRaw::AudioAction(AudioActionRaw {
            channel: "bgm".to_string(),
            action: "play".to_string(),
            asset: Some("music/theme.ogg".to_string()),
            volume: Some(0.42),
            fade_duration_ms: Some(250),
            loop_playback: Some(false),
        }),
        EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "done".to_string(),
        }),
    ];
    let engine = build_engine(events);
    let probe_state = Rc::new(RefCell::new(AudioProbeState::default()));
    let mut app = RuntimeApp::new(
        engine,
        NullInput,
        AudioProbe {
            state: probe_state.clone(),
        },
        NullAssets,
    )
    .expect("runtime");

    app.handle_action(InputAction::Advance).expect("advance");

    let state = probe_state.borrow();
    assert_eq!(state.bgm_calls.len(), 1);
    let (path, loop_playback, volume) = &state.bgm_calls[0];
    assert_eq!(path, "music/theme.ogg");
    assert!(!loop_playback);
    assert_eq!(*volume, Some(0.42));
}

#[test]
fn runtime_routes_voice_channel_play_and_stop() {
    let events = vec![
        EventRaw::AudioAction(AudioActionRaw {
            channel: "voice".to_string(),
            action: "play".to_string(),
            asset: Some("voice/line1.ogg".to_string()),
            volume: Some(0.8),
            fade_duration_ms: None,
            loop_playback: None,
        }),
        EventRaw::AudioAction(AudioActionRaw {
            channel: "voice".to_string(),
            action: "stop".to_string(),
            asset: None,
            volume: None,
            fade_duration_ms: None,
            loop_playback: None,
        }),
        EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "done".to_string(),
        }),
    ];
    let engine = build_engine(events);
    let probe_state = Rc::new(RefCell::new(AudioProbeState::default()));
    let mut app = RuntimeApp::new(
        engine,
        NullInput,
        AudioProbe {
            state: probe_state.clone(),
        },
        NullAssets,
    )
    .expect("runtime");

    app.handle_action(InputAction::Advance)
        .expect("advance play");
    app.handle_action(InputAction::Advance)
        .expect("advance stop");

    let state = probe_state.borrow();
    assert_eq!(state.voice_calls.len(), 1);
    assert_eq!(state.voice_calls[0].0, "voice/line1.ogg");
    assert_eq!(state.voice_calls[0].1, Some(0.8));
    assert_eq!(state.voice_stop_calls, 1);
}

#[test]
fn runtime_consumes_core_audio_when_choice_enters_scene() {
    let events = vec![
        EventRaw::Scene(SceneUpdateRaw {
            background: None,
            music: Some("music/intro.ogg".to_string()),
            characters: vec![],
        }),
        EventRaw::Choice(ChoiceRaw {
            prompt: "Route?".to_string(),
            options: vec![ChoiceOptionRaw {
                text: "Battle".to_string(),
                target: "battle".to_string(),
            }],
        }),
        EventRaw::Scene(SceneUpdateRaw {
            background: None,
            music: Some("music/battle.ogg".to_string()),
            characters: vec![],
        }),
    ];
    let engine = build_engine_with_labels(
        events,
        BTreeMap::from([("start".to_string(), 0), ("battle".to_string(), 2)]),
    );
    let probe_state = Rc::new(RefCell::new(AudioProbeState::default()));
    let mut app = RuntimeApp::new(
        engine,
        NullInput,
        AudioProbe {
            state: probe_state.clone(),
        },
        NullAssets,
    )
    .expect("runtime");

    app.handle_action(InputAction::Advance)
        .expect("advance to choice");
    app.handle_action(InputAction::Choose(0))
        .expect("choose route");

    let bgm_paths = probe_state
        .borrow()
        .bgm_calls
        .iter()
        .map(|(path, _, _)| path.clone())
        .collect::<Vec<_>>();
    assert_eq!(bgm_paths, vec!["music/intro.ogg", "music/battle.ogg"]);
}
