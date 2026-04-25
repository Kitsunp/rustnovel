use super::super::{
    ChoicePolicy, DryRunReport, DryRunStepTrace, DryRunStopReason, DRY_RUN_MAX_STEPS,
};
use super::route_sim::select_choice_index;
use super::signatures::{compiled_event_signature, event_kind_compiled};
use crate::editor::validator::{LintCode, LintIssue, ValidationPhase};
use visual_novel_engine::{Engine, EventCompiled, VnError};

#[derive(Debug, Clone)]
pub(in crate::editor::compiler) struct DryRunOutcome {
    pub(crate) issues: Vec<LintIssue>,
    pub(crate) report: DryRunReport,
}

pub(in crate::editor::compiler) fn run_dry_run(
    mut engine: Engine,
    policy: &ChoicePolicy,
) -> DryRunOutcome {
    let mut issues = Vec::new();
    let mut traces = Vec::new();
    let mut steps = 0usize;
    let mut choice_cursor = 0usize;
    let mut failing_event_ip = None;

    let (stop_reason, stop_message) = loop {
        if steps >= DRY_RUN_MAX_STEPS {
            let stop_message = format!(
                "Dry Run reached {} steps; possible loop or blocking flow",
                DRY_RUN_MAX_STEPS
            );
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
            Err(_) => {
                let msg = format!("Dry Run finished in {} step(s)", steps);
                issues.push(LintIssue::info(
                    None,
                    ValidationPhase::DryRun,
                    LintCode::DryRunFinished,
                    msg.clone(),
                ));
                break (DryRunStopReason::Finished, msg);
            }
        };

        traces.push(DryRunStepTrace {
            step: steps,
            event_ip: ip,
            event_kind: event_kind_compiled(&event).to_string(),
            event_signature: compiled_event_signature(&event),
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
            let stop_message = format!("Dry Run runtime error at ip {}: {}", ip, err);
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
            max_steps: DRY_RUN_MAX_STEPS,
            executed_steps: steps,
            stop_reason,
            stop_message,
            failing_event_ip,
            steps: traces,
        },
    }
}
