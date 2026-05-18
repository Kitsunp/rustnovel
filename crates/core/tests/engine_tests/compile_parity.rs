use super::*;

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
