use super::*;

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
    engine.step().unwrap();
    engine.step().unwrap();
    let history = &engine.state().history;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].text.as_ref(), "Hola");

    let mut oversized = engine.state().clone();
    let dialogue = oversized.history[0].clone();
    oversized.history = (0..210)
        .map(|index| {
            let mut entry = dialogue.clone();
            entry.text = format!("line {index}").into();
            entry
        })
        .collect();
    let mut direct = oversized.clone();
    direct.record_dialogue(&dialogue);
    assert_eq!(direct.history.len(), 200);
    assert_eq!(direct.history.front().unwrap().text.as_ref(), "line 11");
    assert_eq!(direct.history.back().unwrap().text.as_ref(), "Hola");
    engine.set_state(oversized).unwrap();
    assert_eq!(engine.state().history.len(), 200);
    assert_eq!(
        engine.state().history.front().unwrap().text.as_ref(),
        "line 10"
    );
    assert_eq!(
        engine.state().history.back().unwrap().text.as_ref(),
        "line 209"
    );
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

    engine.step().unwrap(); // scene (ip 0)
    assert!(!engine.is_current_dialogue_read());

    engine.step().unwrap(); // dialogue (ip 1) -> marked as read
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
    engine.step().unwrap();
    engine.step().unwrap();
    let serialized = serde_json::to_string(engine.state()).unwrap();
    let parsed =
        serde_json::from_str::<visual_novel_engine::runtime::EngineState>(&serialized).unwrap();
    assert_eq!(parsed.position, engine.state().position);
    assert_eq!(parsed.history.len(), engine.state().history.len());
    let saved = parsed.clone();
    engine.choose(1).unwrap();
    assert!(!engine.choice_history().is_empty());
    engine.set_state(parsed).unwrap();
    assert_eq!(
        serde_json::to_value(engine.state()).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );

    let mut legacy = saved;
    legacy.position = 0;
    legacy.read_model = None;
    legacy.route_progress = None;
    engine.choose(1).unwrap();
    engine.set_state(legacy.clone()).unwrap();
    let mut fresh = Engine::new(
        sample_script(),
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    fresh.set_state(legacy).unwrap();
    assert_eq!(
        serde_json::to_value(engine.state()).unwrap(),
        serde_json::to_value(fresh.state()).unwrap()
    );
    assert!(engine.choice_history().is_empty());
    assert!(!engine.is_dialogue_read(1));

    let mut oversized = engine.state().clone();
    let choices = &mut oversized.route_progress.as_mut().unwrap().selected_choices;
    choices.extend(
        (0..520).map(|index| visual_novel_engine::ChoiceProgressSnapshot {
            event_ip: 2,
            prompt: format!("choice {index}"),
            option_index: 1,
            option_text: "No".into(),
            target_ip: 0,
        }),
    );
    engine.set_state(oversized).unwrap();
    assert_eq!(engine.choice_history().len(), 512);
    assert_eq!(engine.choice_history().front().unwrap().prompt, "choice 8");
    assert_eq!(engine.choice_history().back().unwrap().prompt, "choice 519");
    let before_error = serde_json::to_value(engine.state()).unwrap();
    let mut invalid = engine.state().clone();
    invalid.position = u32::MAX;
    assert!(engine.set_state(invalid).is_err());
    assert_eq!(serde_json::to_value(engine.state()).unwrap(), before_error);
    let mut completed = engine.state().clone();
    completed.position = engine.script().events.len() as u32;
    engine.set_state(completed).unwrap();
    assert!(matches!(
        engine.current_event(),
        Err(visual_novel_engine::VnError::EndOfScript)
    ));
    let script_id = visual_novel_engine::compute_script_id(&engine.script().to_binary().unwrap());
    let save = visual_novel_engine::SaveData::new(script_id, engine.state().clone());
    let mut other_script = save.clone();
    other_script.script_id[0] ^= 1;
    let before_error = serde_json::to_value(engine.state()).unwrap();
    let err = engine.load_save_data(other_script).unwrap_err();
    assert!(err
        .to_string()
        .contains("save does not match current script"));
    assert_eq!(serde_json::to_value(engine.state()).unwrap(), before_error);
    let mut invalid = save.clone();
    invalid.state.position = u32::MAX;
    assert!(engine.load_save_data(invalid).is_err());
    assert_eq!(serde_json::to_value(engine.state()).unwrap(), before_error);
    engine.load_save_data(save).unwrap();
    assert_eq!(serde_json::to_value(engine.state()).unwrap(), before_error);
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
    engine.step().unwrap();
    engine.step().unwrap();
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

    engine.step().unwrap(); // scene
    engine.step().unwrap(); // dialogue
    engine.choose(1).unwrap(); // choice -> start

    let history = engine.choice_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].event_ip, 2);
    assert_eq!(history[0].prompt, "Ir?");
    assert_eq!(history[0].option_index, 1);
    assert_eq!(history[0].option_text, "No");
    assert_eq!(history[0].target_ip, 0);
}

#[test]
fn json_round_trip() {
    let script = sample_script();
    let expected = serde_json::json!({
        "script_schema_version": visual_novel_engine::SCRIPT_SCHEMA_VERSION,
        "events": script.events,
        "labels": script.labels,
    });
    let serialized = script.to_json().unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&serialized).unwrap(),
        expected
    );
    let parsed = ScriptRaw::from_json(&serialized).unwrap();
    assert_eq!(
        serde_json::to_value(parsed).unwrap(),
        serde_json::to_value(script).unwrap()
    );

    let invalid = format!(
        r#"  {{"script_schema_version":"{}","events":[],"labels":42}}"#,
        visual_novel_engine::SCRIPT_SCHEMA_VERSION
    );
    let visual_novel_engine::VnError::Serialization { src, .. } =
        ScriptRaw::from_json(&invalid).unwrap_err()
    else {
        panic!("invalid payload must report a serialization error");
    };
    assert_eq!(src, invalid, "diagnostics must use the supplied source");
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
    engine.step().unwrap();
    engine.step().unwrap();
    engine.choose(0).unwrap();
    engine.step().unwrap();
    let result = engine.step();
    assert!(matches!(
        result,
        Err(visual_novel_engine::VnError::EndOfScript)
    ));
}
