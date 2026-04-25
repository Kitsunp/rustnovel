use super::*;
use crate::resource::ResourceLimiter;
use crate::script::ScriptRaw;
use crate::security::SecurityPolicy;
use crate::{AssetId, AudioCommand};

/// Engineer Manifesto: Criterion A (Air Gapped) & B (Paranoiac Integrity).
///
/// This test verifies deterministic audio command generation:
/// 1. Engine::new queues initial PlayBgm if start Scene has music
/// 2. step() returns queued audio + any delta from current event
/// 3. Music delta is only emitted when music actually changes
#[test]
fn test_audio_determinism() {
    // Script: Scene(music_a) -> Dialogue -> Scene(music_b)
    let json = r#"{
            "script_schema_version": "1.0",
            "events": [
                { "type": "scene", "music": "bgm_intro.ogg", "characters": [] },
                { "type": "dialogue", "speaker": "Me", "text": "Hello" },
                { "type": "scene", "music": "bgm_battle.ogg", "characters": [] }
            ],
            "labels": { "start": 0 }
        }"#;

    let script = ScriptRaw::from_json(json).unwrap();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();

    // === STEP 0 (Scene at position 0) ===
    // Engine::new already applied scene[0] and queued initial_audio_commands.
    // step() processes event[0] again: before_music == after_music (both bgm_intro)
    // So append_music_delta adds nothing. We only get the initial queue.
    let (audio_step0, _) = engine.step().unwrap();
    assert_eq!(audio_step0.len(), 1, "Init queued PlayBgm for bgm_intro");
    assert!(matches!(
        &audio_step0[0],
        AudioCommand::PlayBgm { resource, .. } if resource.as_u64() == AssetId::from_path("bgm_intro.ogg").as_u64()
    ));

    // === STEP 1 (Dialogue) ===
    // No music change in dialogue -> no audio command
    let (audio_step1, _) = engine.step().unwrap();
    assert!(audio_step1.is_empty(), "Dialogue doesn't change music");

    // === STEP 2 (Scene with different music) ===
    // before_music = bgm_intro, after_music = bgm_battle -> delta emits PlayBgm
    let (audio_step2, _) = engine.step().unwrap();
    assert_eq!(audio_step2.len(), 1, "Music change -> PlayBgm");
    assert!(matches!(
        &audio_step2[0],
        AudioCommand::PlayBgm { resource, .. } if resource.as_u64() == AssetId::from_path("bgm_battle.ogg").as_u64()
    ));

    // === Determinism: Run same script again, must get identical results ===
    let script2 = ScriptRaw::from_json(json).unwrap();
    let mut engine2 = Engine::new(
        script2,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();

    let (audio2_0, _) = engine2.step().unwrap();
    let (audio2_1, _) = engine2.step().unwrap();
    let (audio2_2, _) = engine2.step().unwrap();

    assert_eq!(audio_step0, audio2_0, "Run 1 step 0 == Run 2 step 0");
    assert_eq!(audio_step1, audio2_1, "Run 1 step 1 == Run 2 step 1");
    assert_eq!(audio_step2, audio2_2, "Run 1 step 2 == Run 2 step 2");
}

#[test]
fn audio_action_fade_out_emits_stop_bgm() {
    let json = r#"{
            "script_schema_version": "1.0",
            "events": [
                {
                    "type": "audio_action",
                    "channel": "bgm",
                    "action": "fade_out",
                    "fade_duration_ms": 900
                }
            ],
            "labels": { "start": 0 }
        }"#;

    let script = ScriptRaw::from_json(json).unwrap();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();

    let (audio, _) = engine.step().unwrap();
    assert_eq!(audio.len(), 1);
    assert!(matches!(
        audio[0],
        AudioCommand::StopBgm { fade_out } if fade_out.as_millis() == 900
    ));
}

#[test]
fn choose_into_scene_queues_audio_once_from_core() {
    let json = r#"{
            "script_schema_version": "1.0",
            "events": [
                { "type": "scene", "music": "bgm_intro.ogg", "characters": [] },
                {
                    "type": "choice",
                    "prompt": "Go?",
                    "options": [
                        { "text": "Battle", "target": "battle" }
                    ]
                },
                { "type": "scene", "music": "bgm_battle.ogg", "characters": [] }
            ],
            "labels": { "start": 0, "battle": 2 }
        }"#;

    let script = ScriptRaw::from_json(json).unwrap();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();

    let _ = engine.step().unwrap();
    let _choice = engine.choose(0).unwrap();
    let audio = engine.take_audio_commands();

    assert_eq!(
        engine.visual_state().music.as_deref(),
        Some("bgm_battle.ogg")
    );
    assert_eq!(audio.len(), 1);
    assert!(matches!(
        &audio[0],
        AudioCommand::PlayBgm { path, .. } if path.as_ref() == "bgm_battle.ogg"
    ));

    let (scene_audio, _) = engine.step().unwrap();
    assert!(
        scene_audio.is_empty(),
        "stepping the target scene must not replay audio already emitted by choose()"
    );
}
