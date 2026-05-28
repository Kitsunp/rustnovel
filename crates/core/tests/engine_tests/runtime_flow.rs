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
    let parsed =
        serde_json::from_str::<visual_novel_engine::runtime::EngineState>(&serialized).unwrap();
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
    assert_eq!(history[0].prompt, "Ir?");
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
