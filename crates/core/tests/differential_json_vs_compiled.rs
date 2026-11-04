use visual_novel_engine::{
    runtime::{
        Engine, RuntimeScriptInput, ScriptCompiled, ScriptRaw, StateDigest, TraceUiView, UiTrace,
    },
    ResourceLimiter, SecurityPolicy, VnError, SCRIPT_SCHEMA_VERSION,
};

fn run_engine(mut engine: Engine, max_steps: usize) -> UiTrace {
    let mut trace = UiTrace::new();
    for step in 0..max_steps {
        let event = match engine.current_event() {
            Ok(event) => event,
            Err(VnError::EndOfScript) => break,
            Err(err) => panic!("current_event failed at step {step}: {err}"),
        };
        let view = TraceUiView::from_event(&event);
        let state = StateDigest::from_state(engine.state(), engine.script().flag_count as usize);
        trace.push(step as u32, view, state);
        match &event {
            visual_novel_engine::runtime::EventCompiled::Choice(_) => {
                engine.choose(0).expect("choose option 0");
            }
            _ => {
                engine.step().expect("step");
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
                {{"type": "dialogue", "speaker": "Narrator", "text": "Branch hit"}},
                {{"type": "scene", "background": "", "music": "", "characters": []}}
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
    for input in [script_json.as_bytes(), compiled_bytes.as_slice()] {
        let engine = RuntimeScriptInput::parse(input, ResourceLimiter::default())
            .unwrap()
            .into_engine(SecurityPolicy::default(), ResourceLimiter::default())
            .unwrap();
        assert_eq!(run_engine(engine, 10).steps, trace_json.steps);
        let engine = RuntimeScriptInput::from_reader(input, ResourceLimiter::default())
            .unwrap()
            .into_engine(SecurityPolicy::default(), ResourceLimiter::default())
            .unwrap();
        assert_eq!(run_engine(engine, 10).steps, trace_json.steps);
        for strict in [
            ResourceLimiter {
                max_text_length: 2,
                ..ResourceLimiter::default()
            },
            ResourceLimiter {
                max_label_length: 2,
                ..ResourceLimiter::default()
            },
        ] {
            assert!(RuntimeScriptInput::parse(input, strict)
                .unwrap()
                .into_engine(SecurityPolicy::default(), strict)
                .is_err());
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let renamed = directory.path().join("compiled.json");
    std::fs::write(&renamed, &compiled_bytes).unwrap();
    let engine = RuntimeScriptInput::load(&renamed, ResourceLimiter::default())
        .unwrap()
        .into_engine(SecurityPolicy::default(), ResourceLimiter::default())
        .unwrap();
    assert_eq!(run_engine(engine, 10).steps, trace_json.steps);
    let limits = ResourceLimiter {
        max_script_bytes: compiled_bytes.len() - 1,
        ..ResourceLimiter::default()
    };
    assert!(matches!(
        RuntimeScriptInput::parse(&compiled_bytes, limits),
        Err(VnError::ResourceLimit(_))
    ));
    assert!(matches!(
        RuntimeScriptInput::load(&renamed, limits),
        Err(VnError::ResourceLimit(_))
    ));
    let mut reader = std::io::Cursor::new(&compiled_bytes);
    assert!(matches!(
        RuntimeScriptInput::from_reader(&mut reader, limits),
        Err(VnError::ResourceLimit(_))
    ));
    assert_eq!(reader.position(), limits.max_script_bytes as u64 + 1);
    let mut invalid = ScriptCompiled::from_binary(&compiled_bytes).unwrap();
    invalid.flag_count = u32::MAX;
    assert!(SecurityPolicy::default()
        .validate_compiled(&invalid, ResourceLimiter::default())
        .is_err());
    invalid.flag_count = 0;
    invalid.start_ip = u32::MAX;
    let invalid =
        RuntimeScriptInput::parse(&invalid.to_binary().unwrap(), ResourceLimiter::default())
            .unwrap();
    assert!(invalid
        .into_engine(SecurityPolicy::default(), ResourceLimiter::default())
        .is_err());
    let mut corrupt = compiled_bytes;
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(matches!(
        RuntimeScriptInput::parse(&corrupt, ResourceLimiter::default()),
        Err(VnError::BinaryFormat(_))
    ));
}
