use crate::event::{CondCompiled, EventCompiled};

use super::report::{
    ReproMonitor, ReproMonitorResult, ReproOracle, ReproStepTrace, ReproStopReason,
};

pub(super) fn matches_expected_signature(
    oracle: &ReproOracle,
    stop_reason: &ReproStopReason,
    failing_event_ip: Option<u32>,
    steps: &[ReproStepTrace],
) -> bool {
    if oracle.expected_stop_reason.is_none()
        && oracle.expected_event_ip.is_none()
        && oracle.expected_event_kind.is_none()
    {
        return false;
    }

    if let Some(expected) = &oracle.expected_stop_reason {
        if expected != stop_reason {
            return false;
        }
    }

    if let Some(expected_ip) = oracle.expected_event_ip {
        if failing_event_ip == Some(expected_ip) {
            // ok
        } else if !steps.iter().any(|step| step.event_ip == expected_ip) {
            return false;
        }
    }

    if let Some(expected_kind) = oracle
        .expected_event_kind
        .as_ref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    {
        let kind_match = if let Some(expected_ip) = oracle.expected_event_ip {
            steps
                .iter()
                .find(|step| step.event_ip == expected_ip)
                .map(|step| step.event_kind.eq_ignore_ascii_case(expected_kind.as_str()))
                .unwrap_or(false)
        } else {
            steps
                .iter()
                .any(|step| step.event_kind.eq_ignore_ascii_case(expected_kind.as_str()))
        };
        if !kind_match {
            return false;
        }
    }

    true
}

pub(super) fn evaluate_monitors(
    monitors: &[ReproMonitor],
    stop_message: &str,
    steps: &[ReproStepTrace],
) -> Vec<ReproMonitorResult> {
    monitors
        .iter()
        .map(|monitor| match monitor {
            ReproMonitor::EventKindAtStep {
                monitor_id,
                step,
                expected,
            } => {
                let expected_norm = expected.trim().to_ascii_lowercase();
                let matched = steps
                    .get(*step)
                    .map(|trace| {
                        trace
                            .event_kind
                            .eq_ignore_ascii_case(expected_norm.as_str())
                    })
                    .unwrap_or(false);
                ReproMonitorResult {
                    monitor_id: monitor_id.clone(),
                    matched,
                    detail: format!("step={} expected_kind='{}'", step, expected_norm),
                }
            }
            ReproMonitor::EventSignatureContains {
                monitor_id,
                step,
                needle,
            } => {
                let matched = steps
                    .get(*step)
                    .map(|trace| trace.event_signature.contains(needle))
                    .unwrap_or(false);
                ReproMonitorResult {
                    monitor_id: monitor_id.clone(),
                    matched,
                    detail: format!("step={} needle='{}'", step, needle),
                }
            }
            ReproMonitor::VisualBackgroundAtStep {
                monitor_id,
                step,
                expected,
            } => {
                let got = steps
                    .get(*step)
                    .and_then(|trace| trace.visual_background.clone());
                let matched = got == *expected;
                ReproMonitorResult {
                    monitor_id: monitor_id.clone(),
                    matched,
                    detail: format!("step={} expected_bg={:?} got={:?}", step, expected, got),
                }
            }
            ReproMonitor::VisualMusicAtStep {
                monitor_id,
                step,
                expected,
            } => {
                let got = steps
                    .get(*step)
                    .and_then(|trace| trace.visual_music.clone());
                let matched = got == *expected;
                ReproMonitorResult {
                    monitor_id: monitor_id.clone(),
                    matched,
                    detail: format!("step={} expected_music={:?} got={:?}", step, expected, got),
                }
            }
            ReproMonitor::CharacterCountAtLeast {
                monitor_id,
                step,
                min,
            } => {
                let got = steps
                    .get(*step)
                    .map(|trace| trace.character_count)
                    .unwrap_or(0);
                let matched = got >= *min;
                ReproMonitorResult {
                    monitor_id: monitor_id.clone(),
                    matched,
                    detail: format!("step={} min_chars={} got={}", step, min, got),
                }
            }
            ReproMonitor::StopMessageContains { monitor_id, needle } => {
                let matched = stop_message.contains(needle);
                ReproMonitorResult {
                    monitor_id: monitor_id.clone(),
                    matched,
                    detail: format!("stop_message contains '{}'", needle),
                }
            }
            ReproMonitor::StalledSignatureWindow { monitor_id, window } => {
                let window_size = (*window).max(2);
                let mut matched = false;
                let mut streak = 1usize;
                for idx in 1..steps.len() {
                    if steps[idx].event_signature == steps[idx - 1].event_signature {
                        streak = streak.saturating_add(1);
                        if streak >= window_size {
                            matched = true;
                            break;
                        }
                    } else {
                        streak = 1;
                    }
                }
                ReproMonitorResult {
                    monitor_id: monitor_id.clone(),
                    matched,
                    detail: format!("window={window_size}"),
                }
            }
        })
        .collect()
}

pub(super) fn event_kind_compiled(event: &EventCompiled) -> &'static str {
    match event {
        EventCompiled::Dialogue(_) => "dialogue",
        EventCompiled::Choice(_) => "choice",
        EventCompiled::Scene(_) => "scene",
        EventCompiled::Jump { .. } => "jump",
        EventCompiled::SetFlag { .. } => "set_flag",
        EventCompiled::SetVar { .. } => "set_var",
        EventCompiled::JumpIf { .. } => "jump_if",
        EventCompiled::Patch(_) => "patch",
        EventCompiled::ExtCall { .. } => "ext_call",
        EventCompiled::AudioAction(_) => "audio_action",
        EventCompiled::Transition(_) => "transition",
        EventCompiled::SetCharacterPosition(_) => "set_character_position",
    }
}

pub(super) fn compiled_event_signature(event: &EventCompiled) -> String {
    match event {
        EventCompiled::Dialogue(dialogue) => {
            format!(
                "dialogue|{}|{}",
                dialogue.speaker.as_ref(),
                dialogue.text.as_ref()
            )
        }
        EventCompiled::Choice(choice) => {
            format!("choice|{}|{}", choice.prompt.as_ref(), choice.options.len())
        }
        EventCompiled::Scene(scene) => format!(
            "scene|bg={:?}|music={:?}|chars={}",
            scene.background.as_deref(),
            scene.music.as_deref(),
            scene.characters.len()
        ),
        EventCompiled::Jump { .. } => "jump".to_string(),
        EventCompiled::SetFlag { value, .. } => format!("set_flag|{}", value),
        EventCompiled::SetVar { value, .. } => format!("set_var|{}", value),
        EventCompiled::JumpIf { cond, .. } => format!("jump_if|{}", cond_signature(cond)),
        EventCompiled::Patch(patch) => format!(
            "patch|bg={:?}|music={:?}|add={}|upd={}|rm={}",
            patch.background.as_deref(),
            patch.music.as_deref(),
            patch.add.len(),
            patch.update.len(),
            patch.remove.len()
        ),
        EventCompiled::ExtCall { command, args } => {
            format!("ext_call|{}|{}", command, args.len())
        }
        EventCompiled::AudioAction(action) => format!(
            "audio|{}|{}|asset={:?}|vol={}|fade={:?}|loop={:?}",
            compiled_audio_channel(action.channel),
            compiled_audio_action(action.action),
            action.asset.as_deref(),
            fmt_opt_f32(action.volume),
            action.fade_duration_ms,
            action.loop_playback
        ),
        EventCompiled::Transition(trans) => format!(
            "transition|{}|{}|{:?}",
            compiled_transition_kind(trans.kind),
            trans.duration_ms,
            trans.color.as_deref()
        ),
        EventCompiled::SetCharacterPosition(pos) => format!(
            "set_character_position|{}|{}|{}|{}",
            pos.name.as_ref(),
            pos.x,
            pos.y,
            fmt_opt_f32(pos.scale)
        ),
    }
}

fn cond_signature(cond: &CondCompiled) -> String {
    match cond {
        CondCompiled::Flag { is_set, .. } => format!("flag|{}", is_set),
        CondCompiled::VarCmp { op, value, .. } => format!("var|{:?}|{}", op, value),
    }
}

fn compiled_audio_channel(channel: u8) -> &'static str {
    match channel {
        0 => "bgm",
        1 => "sfx",
        2 => "voice",
        _ => "unknown",
    }
}

fn compiled_audio_action(action: u8) -> &'static str {
    match action {
        0 => "play",
        1 => "stop",
        2 => "fade_out",
        _ => "unknown",
    }
}

fn compiled_transition_kind(kind: u8) -> &'static str {
    match kind {
        0 => "fade",
        1 => "dissolve",
        2 => "cut",
        _ => "unknown",
    }
}

fn fmt_opt_f32(value: Option<f32>) -> String {
    match value {
        Some(v) => format!("{v:.3}"),
        None => "none".to_string(),
    }
}
