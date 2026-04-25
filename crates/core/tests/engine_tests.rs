use std::collections::BTreeMap;

use visual_novel_engine::{
    CharacterPlacementRaw, Engine, EventCompiled, EventRaw, RenderBackend, ResourceLimiter,
    SceneUpdateRaw, ScriptRaw, SecurityPolicy, TextRenderer,
};

fn sample_script() -> ScriptRaw {
    let events = vec![
        EventRaw::Scene(SceneUpdateRaw {
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
        }),
        EventRaw::Dialogue(visual_novel_engine::DialogueRaw {
            speaker: "Ava".to_string(),
            text: "Hola".to_string(),
        }),
        EventRaw::Choice(visual_novel_engine::ChoiceRaw {
            prompt: "Ir?".to_string(),
            options: vec![
                visual_novel_engine::ChoiceOptionRaw {
                    text: "Si".to_string(),
                    target: "end".to_string(),
                },
                visual_novel_engine::ChoiceOptionRaw {
                    text: "No".to_string(),
                    target: "start".to_string(),
                },
            ],
        }),
        EventRaw::Dialogue(visual_novel_engine::DialogueRaw {
            speaker: "Ava".to_string(),
            text: "Fin".to_string(),
        }),
    ];
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    labels.insert("end".to_string(), 3);
    ScriptRaw::new(events, labels)
}

fn script_without_start_label() -> ScriptRaw {
    let events = vec![EventRaw::Dialogue(visual_novel_engine::DialogueRaw {
        speaker: "Ava".to_string(),
        text: "Hola".to_string(),
    })];
    let labels = BTreeMap::new();
    ScriptRaw::new(events, labels)
}

fn script_with_invalid_choice_target() -> ScriptRaw {
    let events = vec![EventRaw::Choice(visual_novel_engine::ChoiceRaw {
        prompt: "Ir?".to_string(),
        options: vec![visual_novel_engine::ChoiceOptionRaw {
            text: "Si".to_string(),
            target: "missing".to_string(),
        }],
    })];
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    ScriptRaw::new(events, labels)
}

#[test]
fn engine_steps_through_dialogue() {
    let script = sample_script();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    let scene = engine.step_event().unwrap();
    assert!(matches!(scene, EventCompiled::Scene(_)));
    let dialogue = engine.step_event().unwrap();
    assert!(matches!(dialogue, EventCompiled::Dialogue(_)));
}

#[test]
fn engine_records_dialogue_history() {
    let script = sample_script();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    let _ = engine.step().unwrap();
    let _ = engine.step().unwrap();
    let history = &engine.state().history;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].text.as_ref(), "Hola");
}

#[test]
fn engine_marks_dialogue_as_read_by_ip() {
    let script = sample_script();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();

    let _ = engine.step().unwrap(); // scene (ip 0)
    assert!(!engine.is_current_dialogue_read());

    let _ = engine.step().unwrap(); // dialogue (ip 1) -> marked as read
    assert!(engine.is_dialogue_read(1));
}

#[test]
fn engine_state_round_trip() {
    let script = sample_script();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    let _ = engine.step().unwrap();
    let _ = engine.step().unwrap();
    let serialized = serde_json::to_string(engine.state()).unwrap();
    let parsed = serde_json::from_str::<visual_novel_engine::EngineState>(&serialized).unwrap();
    assert_eq!(parsed.position, engine.state().position);
    assert_eq!(parsed.history.len(), engine.state().history.len());
}

#[test]
fn engine_choice_jumps() {
    let script = sample_script();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    let _ = engine.step().unwrap();
    let _ = engine.step().unwrap();
    let choice = engine.choose(0).unwrap();
    assert!(matches!(choice, EventCompiled::Choice(_)));
    let next = engine.step_event().unwrap();
    if let EventCompiled::Dialogue(dialogue) = next {
        assert_eq!(dialogue.text.as_ref(), "Fin");
    } else {
        panic!("expected dialogue");
    }
}

#[test]
fn engine_records_choice_history() {
    let script = sample_script();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();

    let _ = engine.step().unwrap(); // scene
    let _ = engine.step().unwrap(); // dialogue
    let _ = engine.choose(1).unwrap(); // choice -> start

    let history = engine.choice_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].event_ip, 2);
    assert_eq!(history[0].option_index, 1);
    assert_eq!(history[0].option_text, "No");
    assert_eq!(history[0].target_ip, 0);
}

#[test]
fn json_round_trip() {
    let script = sample_script();
    let serialized = serde_json::json!({
        "script_schema_version": visual_novel_engine::SCRIPT_SCHEMA_VERSION,
        "events": script.events,
        "labels": script.labels,
    })
    .to_string();
    let parsed = ScriptRaw::from_json(&serialized).unwrap();
    assert_eq!(parsed.events.len(), 4);
}

#[test]
fn engine_rejects_missing_start_label() {
    let script = script_without_start_label();
    let error = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect_err("should reject missing start label");
    assert!(matches!(
        error,
        visual_novel_engine::VnError::InvalidScript(_)
    ));
}

#[test]
fn engine_rejects_invalid_choice_target() {
    let script = script_with_invalid_choice_target();
    let error = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect_err("should reject missing choice target");
    assert!(matches!(
        error,
        visual_novel_engine::VnError::InvalidScript(_)
    ));
}

#[test]
fn engine_signals_end_of_script() {
    let script = sample_script();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    let _ = engine.step().unwrap();
    let _ = engine.step().unwrap();
    engine.choose(0).unwrap();
    let _ = engine.step().unwrap();
    let result = engine.step();
    assert!(matches!(
        result,
        Err(visual_novel_engine::VnError::EndOfScript)
    ));
}

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
fn renderer_formats_choice_and_dialogue() {
    let script = sample_script();
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    let _ = engine.step().unwrap();
    let dialogue = engine.step_event().unwrap();
    let renderer = TextRenderer;
    let output = renderer.render(&dialogue, engine.visual_state());
    assert!(output.text.contains("Ava: Hola"));

    let choice = engine.step_event().unwrap();
    let output = renderer.render(&choice, engine.visual_state());
    assert!(output.text.contains("1. Si"));
    assert!(output.text.contains("2. No"));
}

#[test]
fn compiled_script_resolves_targets() {
    let script = sample_script();
    let compiled = script.compile().expect("compile script");
    assert_eq!(compiled.start_ip, 0);
    assert_eq!(compiled.events.len(), 4);
    let choice = compiled.events.get(2).expect("choice event");
    if let EventCompiled::Choice(choice) = choice {
        assert_eq!(choice.options.len(), 2);
        assert_eq!(choice.options[0].target_ip, 3);
        assert_eq!(choice.options[1].target_ip, 0);
    } else {
        panic!("expected compiled choice");
    }
}

#[test]
fn compile_rejects_invalid_targets() {
    let script = script_with_invalid_choice_target();
    let error = script
        .compile()
        .expect_err("should reject missing choice target");
    assert!(matches!(
        error,
        visual_novel_engine::VnError::InvalidScript(_)
    ));
}

#[test]
fn compiled_runtime_matches_raw_sequence() {
    let script = sample_script();
    let compiled_sequence = collect_compiled_sequence(&script, &[0]);
    let raw_sequence = collect_raw_sequence(&script, &[0]);
    assert_eq!(compiled_sequence, raw_sequence);
}

fn collect_compiled_sequence(script: &ScriptRaw, choices: &[usize]) -> Vec<String> {
    let mut engine = Engine::new(
        script.clone(),
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    let mut choice_iter = choices.iter().copied();
    let mut sequence = Vec::new();
    loop {
        let event = match engine.current_event() {
            Ok(event) => event,
            Err(visual_novel_engine::VnError::EndOfScript) => break,
            Err(err) => panic!("unexpected error: {err:?}"),
        };
        sequence.push(event_signature(&event));
        match event {
            EventCompiled::Choice(_) => {
                let choice = choice_iter.next().unwrap_or(0);
                engine.choose(choice).unwrap();
            }
            _ => {
                let _ = engine.step().unwrap();
            }
        }
    }
    sequence
}

fn collect_raw_sequence(script: &ScriptRaw, choices: &[usize]) -> Vec<String> {
    let mut position = script.start_index().unwrap();
    let mut choice_iter = choices.iter().copied();
    let mut sequence = Vec::new();
    while position < script.events.len() {
        let event = script.events.get(position).expect("event");
        sequence.push(event_signature_raw(event));
        match event {
            EventRaw::Jump { target } => {
                position = *script.labels.get(target).unwrap();
            }
            EventRaw::Choice(choice) => {
                let choice_index = choice_iter.next().unwrap_or(0);
                let option = choice.options.get(choice_index).unwrap();
                position = *script.labels.get(&option.target).unwrap();
            }
            EventRaw::SetFlag { .. }
            | EventRaw::Dialogue(_)
            | EventRaw::Scene(_)
            | EventRaw::SetVar { .. }
            | EventRaw::Patch(_)
            | EventRaw::ExtCall { .. }
            | EventRaw::AudioAction(_)
            | EventRaw::Transition(_)
            | EventRaw::SetCharacterPosition(_) => {
                position += 1;
            }
            EventRaw::JumpIf { .. } => {
                // For simplified raw traversal, assume we default to next instruction
                // Real raw traversal checking condition would need state.
                // Just for signature equality, we can assume linear or branch taken?
                // The logical equivalent for this test is matching compiled runtime.
                // If compiled runtime takes branch, raw seq must take branch to match.
                // But raw seq logic here is too simple.
                // Let's just assume Fallthrough for now or panic if used in test.
                // Or better, update position based on "next" which is just +1
                // unless we want to simulate the jump.
                // Given collect_raw_sequence is a helper for `compiled_runtime_matches_raw_sequence`,
                // and that test uses `sample_script` which doesn't have JumpIf/SetVar yet,
                // we can just allow them to advance +1 for now to compile.
                position += 1;
            }
        }
    }
    sequence
}

fn event_signature(event: &EventCompiled) -> String {
    match event {
        EventCompiled::Dialogue(dialogue) => format!("dialogue:{}", dialogue.text),
        EventCompiled::Choice(choice) => format!("choice:{}", choice.prompt),
        EventCompiled::Scene(scene) => {
            format!("scene:{}", scene.background.as_deref().unwrap_or("none"))
        }
        EventCompiled::Jump { target_ip } => format!("jump:{target_ip}"),
        EventCompiled::SetFlag { flag_id, value } => format!("flag:{flag_id}:{value}"),
        EventCompiled::SetVar { var_id, value } => format!("var:{var_id}:{value}"),
        EventCompiled::JumpIf { cond: _, target_ip } => format!("jump_if:{target_ip}"),
        EventCompiled::Patch(_) => "patch".to_string(),
        EventCompiled::ExtCall { command, .. } => format!("ext_call:{command}"),
        EventCompiled::AudioAction(action) => format!("audio:{}:{}", action.action, action.channel),
        EventCompiled::Transition(trans) => format!("transition:{}", trans.kind),
        EventCompiled::SetCharacterPosition(pos) => format!("placement:{}", pos.name),
    }
}

fn event_signature_raw(event: &EventRaw) -> String {
    match event {
        EventRaw::Dialogue(dialogue) => format!("dialogue:{}", dialogue.text),
        EventRaw::Choice(choice) => format!("choice:{}", choice.prompt),
        EventRaw::Scene(scene) => {
            format!("scene:{}", scene.background.as_deref().unwrap_or("none"))
        }
        EventRaw::Jump { target } => format!("jump:{target}"),
        EventRaw::SetFlag { key, value } => format!("flag:{key}:{value}"),
        EventRaw::SetVar { key, value } => format!("var:{key}:{value}"),
        EventRaw::JumpIf { .. } => "jump_if".to_string(),
        EventRaw::Patch(_) => "patch".to_string(),
        EventRaw::ExtCall { command, .. } => format!("ext_call:{command}"),
        EventRaw::AudioAction(action) => format!("audio:{}:{}", action.action, action.channel),
        EventRaw::Transition(trans) => format!("transition:{}", trans.kind),
        EventRaw::SetCharacterPosition(pos) => format!("placement:{}", pos.name),
    }
}
