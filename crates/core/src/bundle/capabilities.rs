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
    #[serde(default, skip_serializing, skip_deserializing)]
    pub warnings: Vec<String>,
}

pub(super) fn build_capability_report(
    script: &ScriptRaw,
    _has_runtime_artifact: bool,
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
            EventRaw::Scene(scene) if has_audio_asset(scene.music.as_ref()) => {
                audio_actions.insert("bgm:scene_music".to_string());
            }
            EventRaw::Patch(patch) if has_audio_asset(patch.music.as_ref()) => {
                audio_actions.insert("bgm:scene_patch_music".to_string());
            }
            EventRaw::Transition(transition) => {
                transitions.insert(transition.kind.clone());
            }
            _ => {}
        }
    }
    ExportCapabilityReport {
        schema: "vnengine.export_capability_report.v1".to_string(),
        ext_call_commands: ext_call_commands.into_iter().collect(),
        audio_actions: audio_actions.into_iter().collect(),
        transitions: transitions.into_iter().collect(),
        requires_runtime_artifact: true,
        warnings: Vec::new(),
    }
}

fn has_audio_asset(value: Option<&String>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}
