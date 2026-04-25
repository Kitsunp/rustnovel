use visual_novel_engine::{
    Engine, ResourceLimiter, ScriptCompiled, ScriptRaw, SecurityPolicy, StateDigest, TraceUiView,
    UiTrace, SCRIPT_SCHEMA_VERSION,
};

fn run_engine(mut engine: Engine, max_steps: usize) -> UiTrace {
    let mut trace = UiTrace::new();
    for step in 0..max_steps {
        let event = match engine.current_event() {
            Ok(event) => event,
            Err(_) => break,
        };
        let view = TraceUiView::from_event(&event);
        let state = StateDigest::from_state(engine.state(), engine.script().flag_count as usize);
        trace.push(step as u32, view, state);
        match &event {
            visual_novel_engine::EventCompiled::Choice(_) => {
                let _ = engine.choose(0);
            }
            _ => {
                let _ = engine.step();
            }
        }
    }
    trace
}

#[test]
fn json_and_compiled_paths_match() {
    let script_json = format!(
        r#"{{
            "script_schema_version": "{SCRIPT_SCHEMA_VERSION}",
            "events": [
                {{"type": "dialogue", "speaker": "Narrator", "text": "Start"}},
                {{"type": "set_var", "key": "counter", "value": 2}},
                {{"type": "jump_if", "cond": {{"kind": "var_cmp", "key": "counter", "op": "gt", "value": 1}}, "target": "branch"}},
                {{"type": "dialogue", "speaker": "Narrator", "text": "Skipped"}},
                {{"type": "dialogue", "speaker": "Narrator", "text": "Branch hit"}}
            ],
            "labels": {{
                "start": 0,
                "branch": 4
            }}
        }}"#
    );
    let raw = ScriptRaw::from_json(&script_json).expect("parse");
    let compiled = raw.compile().expect("compile");
    let compiled_bytes = compiled.to_binary().expect("serialize");
    let loaded = ScriptCompiled::from_binary(&compiled_bytes).expect("load");

    let engine_from_json = Engine::new(raw, SecurityPolicy::default(), ResourceLimiter::default())
        .expect("engine json");
    let engine_from_compiled = Engine::from_compiled(
        loaded,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine compiled");

    let trace_json = run_engine(engine_from_json, 10);
    let trace_compiled = run_engine(engine_from_compiled, 10);

    assert_eq!(trace_json.steps, trace_compiled.steps);
}
