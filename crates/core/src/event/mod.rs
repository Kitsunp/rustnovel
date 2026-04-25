//! Event definitions for raw and compiled scripts.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::resource::StringBudget;

pub mod branching;
pub mod choice;
pub mod dialogue;
pub mod scene;

#[cfg(any(feature = "python", feature = "python-embed"))]
mod python_bridge;
#[cfg(any(feature = "python", feature = "python-embed"))]
mod python_bridge_helpers;

pub use branching::{CmpOp, CondCompiled, CondRaw};
pub use choice::{ChoiceCompiled, ChoiceOptionCompiled, ChoiceOptionRaw, ChoiceRaw};
pub use dialogue::{DialogueCompiled, DialogueRaw};
pub use scene::{
    CharacterPatchCompiled, CharacterPatchRaw, CharacterPlacementCompiled, CharacterPlacementRaw,
    ScenePatchCompiled, ScenePatchRaw, SceneUpdateCompiled, SceneUpdateRaw,
    SetCharacterPositionCompiled, SetCharacterPositionRaw,
};

#[cfg(any(feature = "python", feature = "python-embed"))]
pub use python_bridge::PyEvent;

/// Shared string storage used by compiled events.
pub type SharedStr = Arc<str>;

/// JSON-facing events used in `ScriptRaw`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventRaw {
    Dialogue(DialogueRaw),
    Choice(ChoiceRaw),
    Scene(SceneUpdateRaw),
    Jump { target: String },
    SetFlag { key: String, value: bool },
    SetVar { key: String, value: i32 },
    JumpIf { cond: CondRaw, target: String },
    Patch(ScenePatchRaw),
    ExtCall { command: String, args: Vec<String> },
    AudioAction(AudioActionRaw),

    Transition(SceneTransitionRaw),
    SetCharacterPosition(SetCharacterPositionRaw),
}

impl StringBudget for EventRaw {
    fn string_bytes(&self) -> usize {
        match self {
            EventRaw::Dialogue(inner) => inner.string_bytes(),
            EventRaw::Choice(inner) => inner.string_bytes(),
            EventRaw::Scene(inner) => inner.string_bytes(),
            EventRaw::Jump { target } => target.len(),
            EventRaw::SetFlag { key, .. } => key.len(),
            EventRaw::SetVar { key, .. } => key.len(),
            EventRaw::JumpIf { cond, target } => cond.string_bytes() + target.len(),
            EventRaw::Patch(inner) => inner.string_bytes(),
            EventRaw::ExtCall { command, args } => command.len() + args.string_bytes(),
            EventRaw::AudioAction(inner) => inner.string_bytes(),
            EventRaw::Transition(inner) => inner.string_bytes(),
            EventRaw::SetCharacterPosition(inner) => inner.string_bytes(),
        }
    }
}

/// Runtime events with pre-resolved targets and interned strings.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventCompiled {
    Dialogue(DialogueCompiled),
    Choice(ChoiceCompiled),
    Scene(SceneUpdateCompiled),
    Jump { target_ip: u32 },
    SetFlag { flag_id: u32, value: bool },
    SetVar { var_id: u32, value: i32 },
    JumpIf { cond: CondCompiled, target_ip: u32 },
    Patch(ScenePatchCompiled),
    ExtCall { command: String, args: Vec<String> },
    AudioAction(AudioActionCompiled),
    Transition(SceneTransitionCompiled),
    SetCharacterPosition(SetCharacterPositionCompiled),
}

impl EventRaw {
    pub const TYPE_NAMES: &'static [&'static str] = &[
        "dialogue",
        "choice",
        "scene",
        "jump",
        "set_flag",
        "set_var",
        "jump_if",
        "patch",
        "ext_call",
        "audio_action",
        "transition",
        "set_character_position",
    ];

    /// Serializes the raw event to JSON.
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Serializes the raw event to a JSON string.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "null".to_string())
    }
}

impl EventCompiled {
    /// Serializes the compiled event to JSON.
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Serializes the compiled event to a JSON string.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "null".to_string())
    }
}

/// Raw definition for audio actions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AudioActionRaw {
    pub channel: String, // "bgm", "sfx", "voice"
    pub action: String,  // "play", "stop", "fade_out"
    pub asset: Option<String>,
    pub volume: Option<f32>,
    pub fade_duration_ms: Option<u64>,
    pub loop_playback: Option<bool>,
}

impl StringBudget for AudioActionRaw {
    fn string_bytes(&self) -> usize {
        self.channel.len() + self.action.len() + self.asset.as_ref().map(|s| s.len()).unwrap_or(0)
    }
}

/// Compiled definition for audio actions.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AudioActionCompiled {
    pub channel: u8, // 0=BGM, 1=SFX, 2=Voice.
    pub action: u8,  // 0=Play, 1=Stop, 2=FadeOut.
    pub asset: Option<SharedStr>,
    pub volume: Option<f32>,
    pub fade_duration_ms: Option<u64>,
    pub loop_playback: Option<bool>,
}

/// Raw definition for scene transitions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct SceneTransitionRaw {
    pub kind: String, // "fade_black", "dissolve"
    pub duration_ms: u32,
    pub color: Option<String>, // Hex "#000000"
}

impl StringBudget for SceneTransitionRaw {
    fn string_bytes(&self) -> usize {
        self.kind.len() + self.color.as_ref().map(|s| s.len()).unwrap_or(0)
    }
}

/// Compiled definition for scene transitions.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SceneTransitionCompiled {
    pub kind: u8, // 0=Fade, 1=Dissolve
    pub duration_ms: u32,
    pub color: Option<SharedStr>,
}

#[cfg(any(feature = "python", feature = "python-embed"))]
impl EventRaw {
    pub fn to_python(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::PyObject> {
        use pyo3::IntoPyObject;
        let event = pyo3::Py::new(py, PyEvent::from_raw(self.clone()))?;
        Ok(event.into_pyobject(py)?.into_any().unbind())
    }
}

#[cfg(any(feature = "python", feature = "python-embed"))]
impl EventCompiled {
    pub fn to_python(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::PyObject> {
        use pyo3::IntoPyObject;
        let event = pyo3::Py::new(py, PyEvent::from_compiled(self.clone()))?;
        Ok(event.into_pyobject(py)?.into_any().unbind())
    }
}
