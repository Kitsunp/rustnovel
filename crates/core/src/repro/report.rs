use serde::{Deserialize, Serialize};

use crate::error::{VnError, VnResult};

pub const REPRO_RUN_REPORT_SCHEMA: &str = "vnengine.repro_run_report.v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ReproOracle {
    #[serde(default)]
    pub expected_stop_reason: Option<ReproStopReason>,
    #[serde(default)]
    pub expected_event_ip: Option<u32>,
    #[serde(default)]
    pub expected_event_kind: Option<String>,
    #[serde(default)]
    pub monitors: Vec<ReproMonitor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReproStopReason {
    Finished,
    StepLimit,
    RuntimeError,
    CompileError,
    InitError,
}

impl ReproStopReason {
    pub fn label(&self) -> &'static str {
        match self {
            ReproStopReason::Finished => "finished",
            ReproStopReason::StepLimit => "step_limit",
            ReproStopReason::RuntimeError => "runtime_error",
            ReproStopReason::CompileError => "compile_error",
            ReproStopReason::InitError => "init_error",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReproMonitor {
    EventKindAtStep {
        monitor_id: String,
        step: usize,
        expected: String,
    },
    EventSignatureContains {
        monitor_id: String,
        step: usize,
        needle: String,
    },
    VisualBackgroundAtStep {
        monitor_id: String,
        step: usize,
        expected: Option<String>,
    },
    VisualMusicAtStep {
        monitor_id: String,
        step: usize,
        expected: Option<String>,
    },
    CharacterCountAtLeast {
        monitor_id: String,
        step: usize,
        min: usize,
    },
    StopMessageContains {
        monitor_id: String,
        needle: String,
    },
    StalledSignatureWindow {
        monitor_id: String,
        window: usize,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReproStepTrace {
    pub step: usize,
    pub event_ip: u32,
    pub event_kind: String,
    pub event_signature: String,
    pub visual_background: Option<String>,
    pub visual_music: Option<String>,
    pub character_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReproMonitorResult {
    pub monitor_id: String,
    pub matched: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReproRunReport {
    // Stable schema tag keeps run reports machine-auditable over time.
    pub schema: String,
    pub stop_reason: ReproStopReason,
    pub stop_message: String,
    pub failing_event_ip: Option<u32>,
    pub executed_steps: usize,
    pub max_steps: usize,
    pub steps: Vec<ReproStepTrace>,
    pub monitor_results: Vec<ReproMonitorResult>,
    pub matched_monitors: Vec<String>,
    pub signature_match: bool,
    pub oracle_triggered: bool,
}

impl ReproRunReport {
    pub fn to_json(&self) -> VnResult<String> {
        serde_json::to_string_pretty(self).map_err(|err| VnError::Serialization {
            message: err.to_string(),
            src: "".to_string(),
            span: (0, 0).into(),
        })
    }
}
