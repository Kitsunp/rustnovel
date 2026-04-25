use std::collections::{BTreeMap, HashMap};

use super::super::{ChoicePolicy, DryRunReport};
use super::route_sim::simulate_raw_sequence;
use crate::editor::validator::{LintCode, LintIssue, ValidationPhase};
use visual_novel_engine::{EventRaw, ScriptRaw};

pub(in crate::editor::compiler) fn check_preview_runtime_parity(
    script: &ScriptRaw,
    report: &DryRunReport,
    policy: &ChoicePolicy,
) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let runtime_steps = &report.steps;
    let raw_steps = simulate_raw_sequence(script, report.max_steps, policy);
    let route_label = policy.label();
    let overlap = runtime_steps.len().min(raw_steps.len());

    for idx in 0..overlap {
        let runtime = &runtime_steps[idx];
        let raw = &raw_steps[idx];

        if runtime.event_kind != raw.event_kind {
            issues.push(
                LintIssue::error(
                    None,
                    ValidationPhase::DryRun,
                    LintCode::DryRunParityMismatch,
                    format!(
                        "Parity mismatch [route={}] at step {}: preview {}@{} vs runtime {}@{}",
                        route_label.as_str(),
                        idx,
                        raw.event_kind,
                        raw.event_ip,
                        runtime.event_kind,
                        runtime.event_ip
                    ),
                )
                .with_event_ip(Some(runtime.event_ip)),
            );
            break;
        }

        if runtime.event_signature != raw.event_signature {
            issues.push(
                LintIssue::error(
                    None,
                    ValidationPhase::DryRun,
                    LintCode::DryRunParityMismatch,
                    format!(
                        "Parity payload mismatch [route={}] at step {}: preview '{}' vs runtime '{}'",
                        route_label.as_str(),
                        idx,
                        raw.event_signature,
                        runtime.event_signature
                    ),
                )
                .with_event_ip(Some(runtime.event_ip)),
            );
            break;
        }

        if runtime.visual_background != raw.visual_background
            || runtime.visual_music != raw.visual_music
            || runtime.character_count != raw.character_count
        {
            issues.push(
                LintIssue::error(
                    None,
                    ValidationPhase::DryRun,
                    LintCode::DryRunParityMismatch,
                    format!(
                        "Parity visual mismatch [route={}] at step {}: preview bg={:?}, music={:?}, chars={} vs runtime bg={:?}, music={:?}, chars={}",
                        route_label.as_str(),
                        idx,
                        raw.visual_background,
                        raw.visual_music,
                        raw.character_count,
                        runtime.visual_background,
                        runtime.visual_music,
                        runtime.character_count
                    ),
                )
                .with_event_ip(Some(runtime.event_ip)),
            );
            break;
        }
    }

    if runtime_steps.len() != raw_steps.len() {
        let mismatch_step = overlap;
        let mismatch_ip = runtime_steps
            .get(mismatch_step)
            .map(|entry| entry.event_ip)
            .or_else(|| raw_steps.get(mismatch_step).map(|entry| entry.event_ip));
        issues.push(
            LintIssue::error(
                None,
                ValidationPhase::DryRun,
                LintCode::DryRunParityMismatch,
                format!(
                    "Parity length mismatch [route={}]: preview={} runtime={}",
                    route_label.as_str(),
                    raw_steps.len(),
                    runtime_steps.len()
                ),
            )
            .with_event_ip(mismatch_ip),
        );
    }

    issues
}

pub(in crate::editor::compiler) fn build_minimal_repro_script(
    script: &ScriptRaw,
    failure_ip: u32,
    radius: usize,
) -> Option<ScriptRaw> {
    if script.events.is_empty() {
        return Some(ScriptRaw::new(Vec::new(), BTreeMap::new()));
    }

    let failure_idx = (failure_ip as usize).min(script.events.len().saturating_sub(1));
    let start_idx = failure_idx.saturating_sub(radius);
    let end_idx = (failure_idx + radius + 1).min(script.events.len());
    let mut events = script.events[start_idx..end_idx].to_vec();

    let mut old_to_new_label: HashMap<String, String> = HashMap::new();
    let mut labels = BTreeMap::new();

    for offset in 0..events.len() {
        let local_name = format!("repro_{}", offset);
        labels.insert(local_name.clone(), offset);
    }

    for (label, old_idx) in &script.labels {
        if *old_idx >= start_idx && *old_idx < end_idx {
            old_to_new_label.insert(label.clone(), format!("repro_{}", old_idx - start_idx));
        }
    }

    labels.insert("start".to_string(), 0);

    for event in &mut events {
        if !rewrite_event_targets(event, &old_to_new_label) {
            return None;
        }
    }

    Some(ScriptRaw::new(events, labels))
}

fn rewrite_event_targets(event: &mut EventRaw, old_to_new_label: &HashMap<String, String>) -> bool {
    match event {
        EventRaw::Jump { target } => {
            let Some(mapped) = old_to_new_label.get(target).cloned() else {
                return false;
            };
            *target = mapped;
        }
        EventRaw::JumpIf { target, .. } => {
            let Some(mapped) = old_to_new_label.get(target).cloned() else {
                return false;
            };
            *target = mapped;
        }
        EventRaw::Choice(choice) => {
            for option in &mut choice.options {
                let Some(mapped) = old_to_new_label.get(&option.target).cloned() else {
                    return false;
                };
                option.target = mapped;
            }
        }
        _ => {}
    }
    true
}
