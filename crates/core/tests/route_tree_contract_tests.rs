use std::collections::BTreeMap;

use visual_novel_engine::{
    build_route_tree, resolve_visual_at_ip, runtime::*, ResourceLimiter, SecurityPolicy,
    VisualResolveStrategy,
};

fn branching_script() -> ScriptRaw {
    ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: Some("bg/room.png".to_string()),
                ..Default::default()
            }),
            EventRaw::Choice(ChoiceRaw {
                prompt: "Go?".to_string(),
                options: vec![
                    ChoiceOptionRaw {
                        text: "Loop".to_string(),
                        target: "start".to_string(),
                    },
                    ChoiceOptionRaw {
                        text: "End".to_string(),
                        target: "end".to_string(),
                    },
                ],
            }),
            EventRaw::JumpIf {
                cond: CondRaw::Flag {
                    key: "seen".to_string(),
                    is_set: true,
                },
                target: "end".to_string(),
            },
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "Middle".to_string(),
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "End".to_string(),
            }),
        ],
        BTreeMap::from([("start".to_string(), 0), ("end".to_string(), 4)]),
    )
}

#[test]
fn route_tree_contains_choices_cycles_conditionals_and_endings() {
    let compiled = branching_script().compile().expect("compile");
    let tree = build_route_tree(&compiled);

    assert_eq!(tree.root, 0);
    assert!(tree
        .edges
        .iter()
        .any(|edge| edge.kind == RouteEdgeKind::Choice && edge.to == Some(0)));
    assert!(tree
        .edges
        .iter()
        .any(|edge| edge.kind == RouteEdgeKind::Conditional && edge.to == Some(4)));
    assert!(tree
        .edges
        .iter()
        .any(|edge| edge.kind == RouteEdgeKind::Ending && edge.from == 4));
}

#[test]
fn engine_snapshots_preserve_read_model_and_route_progress_on_set_state() {
    let mut engine = Engine::new(
        branching_script(),
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");
    let (_audio, _change) = engine.step().expect("scene");
    engine.choose(1).expect("choose ending");
    let (_audio, _change) = engine.step().expect("ending dialogue");

    let saved = engine.state().clone();
    let mut restored = Engine::new(
        branching_script(),
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");
    restored.set_state(saved).expect("restore");

    assert_eq!(
        restored.read_model_snapshot().visited_dialogue_ips,
        engine.read_model_snapshot().visited_dialogue_ips
    );
    assert_eq!(
        restored.route_progress_snapshot().selected_choices,
        engine.route_progress_snapshot().selected_choices
    );
}

#[test]
fn resolve_visual_at_ip_replays_visual_state_for_preview() {
    let compiled = branching_script().compile().expect("compile");
    let visual = resolve_visual_at_ip(&compiled, 3, VisualResolveStrategy::LinearReplay);
    assert_eq!(visual.background.as_deref(), Some("bg/room.png"));

    let script = ScriptRaw::from_json(r#"{
        "script_schema_version":"1.0",
        "events":[
            {"type":"scene","music":"old.ogg","characters":[]},
            {"type":"audio_action","channel":"bgm","action":"play","asset":"new.ogg","volume":0.3,"loop_playback":false,"fade_duration_ms":750},
            {"type":"audio_action","channel":"sfx","action":"play","asset":"click.ogg"},
            {"type":"audio_action","channel":"voice","action":"play","asset":"line.ogg"},
            {"type":"audio_action","channel":"bgm","action":"fade_out","fade_duration_ms":300},
            {"type":"audio_action","channel":"bgm","action":"play","asset":"last.ogg"},
            {"type":"audio_action","channel":"bgm","action":"stop"},
            {"type":"scene","music":"scene.ogg","characters":[]},
            {"type":"patch","music":null}
        ],
        "labels":{"start":0}
    }"#).unwrap();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    let script_id = visual_novel_engine::compute_script_id(&engine.script().to_binary().unwrap());
    for (ip, expected) in [
        Some("old.ogg"),
        Some("new.ogg"),
        Some("new.ogg"),
        Some("new.ogg"),
        None,
        Some("last.ogg"),
        None,
        Some("scene.ogg"),
        None,
    ]
    .into_iter()
    .enumerate()
    {
        let (audio, _) = engine.step().unwrap();
        assert_eq!(
            engine.visual_state().music.as_deref(),
            expected,
            "runtime ip={ip}"
        );
        let visual = engine.resolve_visual_at_ip(ip as u32);
        assert_eq!(&visual, engine.visual_state(), "preview ip={ip}");
        if ip == 1 {
            assert!(
                matches!(&audio[..], [AudioCommand::PlayBgm { path, r#loop: false, volume: Some(volume), fade_in, .. }] if path.as_ref() == "new.ogg" && *volume == 0.3 && fade_in.as_millis() == 750)
            );
        }
        let save = visual_novel_engine::SaveData::new(script_id, engine.state().clone());
        let bytes = save
            .to_authenticated_binary(visual_novel_engine::AUTH_SAVE_KEY)
            .unwrap();
        let loaded = visual_novel_engine::SaveData::from_any_binary(
            &bytes,
            visual_novel_engine::AUTH_SAVE_KEY,
        )
        .unwrap();
        let mut restored = Engine::from_compiled(
            engine.script().clone(),
            SecurityPolicy::default(),
            ResourceLimiter::default(),
        )
        .unwrap();
        restored.load_save_data(loaded).unwrap();
        assert_eq!(
            restored.visual_state(),
            engine.visual_state(),
            "saved ip={ip}"
        );
        let commands = restored.take_audio_commands();
        match (&commands[2], expected) {
            (AudioCommand::PlayBgm { path, .. }, Some(expected)) => {
                assert_eq!(path.as_ref(), expected)
            }
            (AudioCommand::StopBgm { .. }, None) => {}
            _ => panic!("wrong restored music at ip={ip}: {commands:?}"),
        }
    }
}
