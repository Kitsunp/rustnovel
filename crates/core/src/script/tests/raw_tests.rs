use super::*;
use crate::event::{AudioActionRaw, SceneTransitionRaw};

#[test]
fn compile_rejects_invalid_audio_channel_and_action() {
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    let script = ScriptRaw::new(
        vec![EventRaw::AudioAction(AudioActionRaw {
            channel: "music".to_string(),
            action: "explode".to_string(),
            asset: Some("assets/bgm.ogg".to_string()),
            volume: Some(0.5),
            fade_duration_ms: Some(250),
            loop_playback: Some(true),
        })],
        labels,
    );

    let err = script
        .compile()
        .expect_err("invalid audio mapping must fail");
    assert!(
        err.to_string().contains("invalid audio channel")
            || err.to_string().contains("invalid audio action")
    );
}

#[test]
fn compile_rejects_invalid_transition_kind() {
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    let script = ScriptRaw::new(
        vec![EventRaw::Transition(SceneTransitionRaw {
            kind: "warp".to_string(),
            duration_ms: 100,
            color: None,
        })],
        labels,
    );

    let err = script
        .compile()
        .expect_err("invalid transition kind must fail");
    assert!(err.to_string().contains("invalid transition kind"));
}
