use visual_novel_engine::{
    Engine, ResourceLimiter, ScriptRaw, SecurityPolicy, StateDigest, TraceUiView, UiTrace,
};

/// Helper to execute a script and capture its trace.
pub fn run_headless(script_json: &str, max_steps: usize) -> UiTrace {
    let script = ScriptRaw::from_json(script_json).expect("parse script");
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("create engine");

    let mut trace = UiTrace::new();

    for step in 0..max_steps {
        let event = match engine.current_event() {
            Ok(e) => e,
            Err(_) => break, // End of script
        };

        let view = TraceUiView::from_event(&event);
        let state_digest =
            StateDigest::from_state(engine.state(), engine.script().flag_count as usize);

        trace.push(step as u32, view, state_digest);

        // Auto-advance (for choices, pick option 0)
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
