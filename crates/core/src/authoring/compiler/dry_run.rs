use super::{
    route_sim::select_choice_index, signatures::compiled_event_signature,
    signatures::event_kind_compiled, ChoicePolicy, DryRunReport, DryRunStepTrace, DryRunStopReason,
};
use crate::authoring::{LintCode, LintIssue, ValidationPhase};
use crate::{Engine, EventCompiled, VnError};

#[derive(Debug, Clone)]
pub struct DryRunOutcome {
    pub issues: Vec<LintIssue>,
    pub report: DryRunReport,
}

pub fn run_dry_run(mut engine: Engine, policy: &ChoicePolicy, max_steps: usize) -> DryRunOutcome {
    let mut issues = Vec::new();
    let mut traces = Vec::new();
    let mut steps = 0usize;
    let mut choice_cursor = 0usize;
    let mut failing_event_ip = None;

    let (stop_reason, stop_message) = loop {
        if steps >= max_steps {
            let stop_message =
                format!("Dry Run reached {max_steps} steps; possible loop or blocking flow");
            issues.push(
                LintIssue::warning(
                    Some(engine.state().position),
                    ValidationPhase::DryRun,
                    LintCode::DryRunStepLimit,
                    stop_message.clone(),
                )
                .with_event_ip(Some(engine.state().position)),
            );
            break (DryRunStopReason::StepLimit, stop_message);
        }

        let ip = engine.state().position;
        let event = match engine.current_event() {
            Ok(event) => event,
            Err(VnError::EndOfScript) => {
                let msg = format!("Dry Run finished in {steps} step(s)");
                issues.push(LintIssue::info(
                    None,
                    ValidationPhase::DryRun,
                    LintCode::DryRunFinished,
                    msg.clone(),
                ));
                break (DryRunStopReason::Finished, msg);
            }
            Err(err) => {
                let stop_message = format!("Dry Run runtime error before step {steps}: {err}");
                failing_event_ip = Some(ip);
                issues.push(
                    LintIssue::error(
                        Some(ip),
                        ValidationPhase::DryRun,
                        LintCode::DryRunRuntimeError,
                        stop_message.clone(),
                    )
                    .with_event_ip(Some(ip)),
                );
                break (DryRunStopReason::RuntimeError, stop_message);
            }
        };
        if matches!(event, EventCompiled::ExtCall { .. }) {
            issues.push(
                LintIssue::warning(
                    Some(ip),
                    ValidationPhase::DryRun,
                    LintCode::DryRunExtCallSimulated,
                    format!("Dry Run simulated external call at ip {ip}"),
                )
                .with_event_ip(Some(ip)),
            );
        }

        traces.push(DryRunStepTrace {
            step: steps,
            event_ip: ip,
            event_kind: event_kind_compiled(&event).to_string(),
            event_signature: compiled_event_signature(&event),
            simulation_note: matches!(event, EventCompiled::ExtCall { .. })
                .then(|| "external_call_simulated".to_string()),
            visual_background: engine
                .state()
                .visual
                .background
                .as_ref()
                .map(|value| value.as_ref().to_string()),
            visual_music: engine
                .state()
                .visual
                .music
                .as_ref()
                .map(|value| value.as_ref().to_string()),
            character_count: engine.state().visual.characters.len(),
        });

        let run_result = match event {
            EventCompiled::Choice(choice) => {
                if choice.options.is_empty() {
                    Err(VnError::InvalidChoice)
                } else {
                    let idx =
                        select_choice_index(policy, steps, choice.options.len(), choice_cursor);
                    choice_cursor = choice_cursor.saturating_add(1);
                    engine.choose(idx).map(|_| ())
                }
            }
            EventCompiled::ExtCall { .. } => engine.resume(),
            _ => engine.step().map(|_| ()),
        };

        if let Err(err) = run_result {
            let stop_message = format!("Dry Run runtime error at ip {ip}: {err}");
            failing_event_ip = Some(ip);
            issues.push(
                LintIssue::error(
                    Some(ip),
                    ValidationPhase::DryRun,
                    LintCode::DryRunRuntimeError,
                    stop_message.clone(),
                )
                .with_event_ip(Some(ip)),
            );
            break (DryRunStopReason::RuntimeError, stop_message);
        }

        steps += 1;
    };

    DryRunOutcome {
        issues,
        report: DryRunReport {
            max_steps,
            executed_steps: steps,
            routes_discovered: 0,
            routes_executed: 1,
            route_limit_hit: false,
            depth_limit_hit: false,
            stop_reason,
            stop_message,
            failing_event_ip,
            steps: traces,
        },
    }
}
