use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::event::EventRaw;
use crate::script::ScriptRaw;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportCapabilityReport {
    pub schema: String,
    pub ext_call_commands: Vec<String>,
    pub audio_actions: Vec<String>,
    pub transitions: Vec<String>,
    pub requires_runtime_artifact: bool,
    pub warnings: Vec<String>,
}

pub(super) fn build_capability_report(
    script: &ScriptRaw,
    has_runtime_artifact: bool,
) -> ExportCapabilityReport {
    let mut ext_call_commands = BTreeSet::new();
    let mut audio_actions = BTreeSet::new();
    let mut transitions = BTreeSet::new();
    for event in &script.events {
        match event {
            EventRaw::ExtCall { command, .. } => {
                ext_call_commands.insert(command.clone());
            }
            EventRaw::AudioAction(action) => {
                audio_actions.insert(format!("{}:{}", action.channel, action.action));
            }
            EventRaw::Transition(transition) => {
                transitions.insert(transition.kind.clone());
            }
            _ => {}
        }
    }
    let mut warnings = Vec::new();
    if !ext_call_commands.is_empty() {
        warnings.push("ext_call_requires_runtime_handler".to_string());
    }
    if !audio_actions.is_empty() {
        warnings.push("audio_requires_runtime_audio_backend".to_string());
    }
    if !transitions.is_empty() {
        warnings.push("transitions_require_visual_runtime_support".to_string());
    }
    if !has_runtime_artifact {
        warnings.push("runtime_artifact_missing_launcher_will_not_start_game".to_string());
    }

    ExportCapabilityReport {
        schema: "vnengine.export_capability_report.v1".to_string(),
        ext_call_commands: ext_call_commands.into_iter().collect(),
        audio_actions: audio_actions.into_iter().collect(),
        transitions: transitions.into_iter().collect(),
        requires_runtime_artifact: true,
        warnings,
    }
}
