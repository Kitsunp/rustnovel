pub(super) use crate::event_signature::{compiled_event_signature, event_kind_compiled};

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
