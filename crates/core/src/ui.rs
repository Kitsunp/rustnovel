//! UI mapping helpers for runtime consumers.

use crate::event::EventCompiled;
use crate::visual::VisualState;

/// UI state derived from the current event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiState {
    pub view: UiView,
}

/// Distinct UI views for runtimes.
#[derive(Clone, Debug, PartialEq, Eq)]
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
}

impl UiState {
    /// Build a UI view from the current event and visual state.
    pub fn from_event(event: &EventCompiled, visual: &VisualState) -> Self {
        let view = match event {
            EventCompiled::Dialogue(dialogue) => UiView::Dialogue {
                speaker: dialogue.speaker.as_ref().to_string(),
                text: dialogue.text.as_ref().to_string(),
            },
            EventCompiled::Choice(choice) => UiView::Choice {
                prompt: choice.prompt.as_ref().to_string(),
                options: choice
                    .options
                    .iter()
                    .map(|option| option.text.as_ref().to_string())
                    .collect(),
            },
            EventCompiled::Scene(scene) => {
                let mut visual = visual.clone();
                visual.apply_scene(scene);
                UiView::Scene {
                    description: summarize_scene(&visual),
                }
            }
            EventCompiled::Patch(_) => {
                // Apply patch happened in engine, visual state is already updated
                UiView::Scene {
                    description: summarize_scene(visual),
                }
            }
            EventCompiled::Jump { target_ip } => UiView::System {
                message: format!("Jump to {target_ip}"),
            },
            EventCompiled::SetFlag { flag_id, value } => UiView::System {
                message: format!("Flag {flag_id} = {value}"),
            },
            EventCompiled::SetVar { var_id, value } => UiView::System {
                message: format!("Var {var_id} = {value}"),
            },
            EventCompiled::JumpIf { target_ip, .. } => UiView::System {
                message: format!("JumpIf to {target_ip}"),
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
                message: format!(
                    "SetCharacterPosition {} ({}, {}) scale={:?}",
                    pos.name, pos.x, pos.y, pos.scale
                ),
            },
        };
        Self { view }
    }
}

fn summarize_scene(visual: &VisualState) -> String {
    let mut parts = Vec::new();
    if let Some(background) = &visual.background {
        parts.push(format!("Background: {background}"));
    }
    if let Some(music) = &visual.music {
        parts.push(format!("Music: {music}"));
    }
    if !visual.characters.is_empty() {
        let mut roster = Vec::new();
        for character in &visual.characters {
            let mut entry = character.name.as_ref().to_string();
            if let Some(expression) = &character.expression {
                entry.push_str(" (");
                entry.push_str(expression.as_ref());
                entry.push(')');
            }
            if let Some(position) = &character.position {
                entry.push_str(" @ ");
                entry.push_str(position.as_ref());
            }
            if character.x.is_some() || character.y.is_some() || character.scale.is_some() {
                entry.push_str(&format!(
                    " [x={:?}, y={:?}, scale={:?}]",
                    character.x, character.y, character.scale
                ));
            }
            roster.push(entry);
        }
        parts.push(format!("Characters: {}", roster.join(", ")));
    }
    if parts.is_empty() {
        "Scene updated".to_string()
    } else {
        parts.join(" | ")
    }
}
