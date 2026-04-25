//! Observable state structures for deterministic testing.
//!
//! These types represent the "contractual" view of the engine,
//! excluding internal pointers and implementation details.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::event::EventCompiled;
use crate::state::EngineState;

/// A single step in the execution trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTraceStep {
    /// Step number (0-indexed).
    pub step: u32,
    /// The view presented to the user.
    pub view: UiView,
    /// Digest of the engine state at this step.
    pub state: StateDigest,
}

/// What the user sees at a given step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum UiView {
    Dialogue {
        speaker: String,
        text: String,
    },
    Choice {
        prompt: String,
        options: Vec<String>,
    },
    Scene {
        description: String,
    },
    System {
        message: String,
    },
    End,
}

impl UiView {
    pub fn from_event(event: &EventCompiled) -> Self {
        match event {
            EventCompiled::Dialogue(d) => UiView::Dialogue {
                speaker: d.speaker.to_string(),
                text: d.text.to_string(),
            },
            EventCompiled::Choice(c) => UiView::Choice {
                prompt: c.prompt.to_string(),
                options: c.options.iter().map(|o| o.text.to_string()).collect(),
            },
            EventCompiled::Scene(s) => UiView::Scene {
                description: format!(
                    "Scene bg={:?} music={:?} chars={}",
                    s.background,
                    s.music,
                    s.characters.len()
                ),
            },
            EventCompiled::Patch(p) => UiView::Scene {
                description: format!(
                    "Patch bg={:?} music={:?} add={} update={} remove={}",
                    p.background,
                    p.music,
                    p.add.len(),
                    p.update.len(),
                    p.remove.len()
                ),
            },
            EventCompiled::SetFlag { flag_id, value } => UiView::System {
                message: format!("SetFlag: {} = {}", flag_id, value),
            },
            EventCompiled::SetVar { var_id, value } => UiView::System {
                message: format!("SetVar: {} = {}", var_id, value),
            },
            EventCompiled::Jump { .. } => UiView::System {
                message: "Jump".to_string(),
            },
            EventCompiled::JumpIf { .. } => UiView::System {
                message: "JumpIf".to_string(),
            },
            EventCompiled::ExtCall { command, args } => UiView::System {
                message: format!("ExtCall {command}({})", args.join(", ")),
            },
            EventCompiled::AudioAction(_) => UiView::System {
                message: "Audio Action".to_string(),
            },
            EventCompiled::Transition(_) => UiView::System {
                message: "Transition".to_string(),
            },
            EventCompiled::SetCharacterPosition(pos) => UiView::System {
                message: format!("SetCharacterPosition: {} ({}, {})", pos.name, pos.x, pos.y),
            },
        }
    }
}

/// Simplified engine state for deterministic comparison.
/// Uses only contractual data, no internal handles or pointers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateDigest {
    /// Current instruction pointer.
    pub position: u32,
    /// Active flags as a sorted map (flag_id -> value).
    pub flags: BTreeMap<u32, bool>,
    /// Variables as a sorted map (var_id -> value).
    pub vars: BTreeMap<u32, i32>,
    /// Number of dialogue entries in history.
    pub history_len: usize,
    /// Digest of the visual scene.
    pub visual: VisualDigest,
}

impl StateDigest {
    pub fn from_state(state: &EngineState, flag_count: usize) -> Self {
        let mut flags = BTreeMap::new();
        // Manually iterate bits because state.flags is Vec<u64>
        for i in 0..flag_count {
            let chunk = i / 64;
            let mask = 1u64 << (i % 64);
            if let Some(&val) = state.flags.get(chunk) {
                if (val & mask) != 0 {
                    flags.insert(i as u32, true);
                }
            }
        }

        let mut vars = BTreeMap::new();
        for (i, &val) in state.vars.iter().enumerate() {
            if val != 0 {
                vars.insert(i as u32, val);
            }
        }

        Self {
            position: state.position,
            flags,
            vars,
            history_len: state.history.len(),
            visual: VisualDigest::from_visual(&state.visual),
        }
    }
}

/// A complete execution trace.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UiTrace {
    pub steps: Vec<UiTraceStep>,
}

impl UiTrace {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn push(&mut self, step: u32, view: UiView, state: StateDigest) {
        self.steps.push(UiTraceStep { step, view, state });
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualDigest {
    pub background: Option<String>,
    pub music: Option<String>,
    pub characters: Vec<CharacterDigest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterDigest {
    pub name: String,
    pub expression: Option<String>,
    pub position: Option<String>,
}

impl VisualDigest {
    pub fn from_visual(state: &crate::visual::VisualState) -> Self {
        Self {
            background: state.background.as_deref().map(|value| value.to_string()),
            music: state.music.as_deref().map(|value| value.to_string()),
            characters: state
                .characters
                .iter()
                .map(|character| CharacterDigest {
                    name: character.name.to_string(),
                    expression: character
                        .expression
                        .as_deref()
                        .map(|value| value.to_string()),
                    position: character.position.as_deref().map(|value| value.to_string()),
                })
                .collect(),
        }
    }
}
