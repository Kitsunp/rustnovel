use visual_novel_engine::{
    runtime::{Engine, ScriptRaw, StateDigest, TraceUiView, UiTrace},
    ResourceLimiter, SecurityPolicy, VnError,
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
            Err(VnError::EndOfScript) => break,
            Err(err) => panic!("current_event failed at step {step}: {err}"),
        };

        let view = TraceUiView::from_event(&event);
        let state_digest =
            StateDigest::from_state(engine.state(), engine.script().flag_count as usize);

        trace.push(step as u32, view, state_digest);

        // Auto-advance (for choices, pick option 0)
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
