use crate::event::EventCompiled;

use crate::{Engine, ResourceLimiter, SecurityPolicy};

use super::report::{ReproRunReport, ReproStepTrace, ReproStopReason, REPRO_RUN_REPORT_SCHEMA};
use super::signatures::{
    compiled_event_signature, evaluate_monitors, event_kind_compiled, matches_expected_signature,
};
use super::ReproCase;

pub fn run_repro_case(case: &ReproCase) -> ReproRunReport {
    run_repro_case_with_limits(case, SecurityPolicy::default(), ResourceLimiter::default())
}

pub fn run_repro_case_with_limits(
    case: &ReproCase,
    policy: SecurityPolicy,
    limits: ResourceLimiter,
) -> ReproRunReport {
    let mut traces = Vec::new();
    let mut failing_event_ip = None;
    let stop: (ReproStopReason, String) = match case.script.compile() {
        Ok(compiled) => match Engine::from_compiled(compiled, policy, limits) {
            Ok(mut engine) => {
                let mut steps = 0usize;
                let mut choice_cursor = 0usize;
                loop {
                    if steps >= case.max_steps {
                        break (
                            ReproStopReason::StepLimit,
                            format!("step limit reached ({})", case.max_steps),
                        );
                    }
                    let event = match engine.current_event() {
                        Ok(event) => event,
                        Err(crate::error::VnError::EndOfScript) => {
                            break (ReproStopReason::Finished, "end of script".to_string())
                        }
                        Err(err) => {
                            break (
                                ReproStopReason::RuntimeError,
                                format!("current_event: {err}"),
                            );
                        }
                    };

                    let event_ip = engine.state().position;
                    traces.push(build_step_trace(steps, event_ip, &event, &engine));

                    let step_result = match &event {
                        EventCompiled::Choice(choice) => {
                            let selected = case
                                .choice_route
                                .get(choice_cursor)
                                .copied()
                                .unwrap_or(0)
                                .min(choice.options.len().saturating_sub(1));
                            choice_cursor = choice_cursor.saturating_add(1);
                            engine.choose(selected).map(|_| ())
                        }
                        _ => engine.step().map(|_| ()),
                    };
                    if let Err(err) = step_result {
                        failing_event_ip = Some(event_ip);
                        break (ReproStopReason::RuntimeError, format!("step failed: {err}"));
                    }
                    steps = steps.saturating_add(1);
                }
            }
            Err(err) => (
                ReproStopReason::InitError,
                format!("engine init failed: {err}"),
            ),
        },
        Err(err) => (
            ReproStopReason::CompileError,
            format!("compile failed: {err}"),
        ),
    };

    let (stop_reason, stop_message) = stop;
    let signature_match = matches_expected_signature(
        &case.oracle,
        &stop_reason,
        failing_event_ip,
        traces.as_slice(),
    );
    let monitor_results =
        evaluate_monitors(&case.oracle.monitors, &stop_message, traces.as_slice());
    let matched_monitors = monitor_results
        .iter()
        .filter(|result| result.matched)
        .map(|result| result.monitor_id.clone())
        .collect::<Vec<_>>();
    let oracle_triggered = signature_match || !matched_monitors.is_empty();

    ReproRunReport {
        schema: REPRO_RUN_REPORT_SCHEMA.to_string(),
        stop_reason,
        stop_message,
        failing_event_ip,
        executed_steps: traces.len(),
        max_steps: case.max_steps,
        steps: traces,
        monitor_results,
        matched_monitors,
        signature_match,
        oracle_triggered,
    }
}

fn build_step_trace(
    step: usize,
    event_ip: u32,
    event: &EventCompiled,
    engine: &Engine,
) -> ReproStepTrace {
    ReproStepTrace {
        step,
        event_ip,
        event_kind: event_kind_compiled(event).to_string(),
        event_signature: compiled_event_signature(event),
        visual_background: engine
            .visual_state()
            .background
            .as_ref()
            .map(|value| value.as_ref().to_string()),
        visual_music: engine
            .visual_state()
            .music
            .as_ref()
            .map(|value| value.as_ref().to_string()),
        character_count: engine.visual_state().characters.len(),
    }
}
