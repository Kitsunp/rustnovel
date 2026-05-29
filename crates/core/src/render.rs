//! Rendering helpers for compiled events.

use crate::event::{EventCompiled, SceneUpdateCompiled};
use crate::visual::VisualState;

/// Renderer interface used by the engine.
pub trait RenderBackend {
    fn render(&self, event: &EventCompiled, visual: &VisualState) -> RenderOutput;
}

/// Rendered text output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderOutput {
    pub text: String,
}

/// Simple renderer that formats events as text.
#[derive(Clone, Debug, Default)]
pub struct TextRenderer;

impl TextRenderer {
    fn render_scene(&self, scene: &SceneUpdateCompiled, visual: &VisualState) -> String {
        let mut output = String::with_capacity(128);
        if let Some(background) = scene.background.as_deref().or(visual.background.as_deref()) {
            output.push_str("Background: ");
            output.push_str(background);
            output.push('\n');
        }
        if let Some(music) = scene.music.as_deref().or(visual.music.as_deref()) {
            output.push_str("Music: ");
            output.push_str(music);
            output.push('\n');
        }
        if !visual.characters.is_empty() {
            let mut roster = String::with_capacity(visual.characters.len() * 24);
            for (idx, character) in visual.characters.iter().enumerate() {
                if idx > 0 {
                    roster.push_str(", ");
                }
                roster.push_str(character.name.as_ref());
                if let Some(expression) = &character.expression {
                    roster.push_str(" (");
                    roster.push_str(expression.as_ref());
                    roster.push(')');
                }
                if let Some(position) = &character.position {
                    roster.push_str(" @ ");
                    roster.push_str(position.as_ref());
                }
            }
            output.push_str("Characters: ");
            output.push_str(&roster);
            output.push('\n');
        }
        if output.is_empty() {
            "Scene updated".to_string()
        } else {
            output.truncate(output.trim_end_matches('\n').len());
            output
        }
    }
}

impl RenderBackend for TextRenderer {
    fn render(&self, event: &EventCompiled, visual: &VisualState) -> RenderOutput {
        let text = match event {
            EventCompiled::Dialogue(dialogue) => {
                format!("{}: {}", dialogue.speaker, dialogue.text)
            }
            EventCompiled::Choice(choice) => {
                let mut options = String::with_capacity(choice.options.len().saturating_mul(12));
                for (idx, option) in choice.options.iter().enumerate() {
                    options.push_str(&(idx + 1).to_string());
                    options.push_str(". ");
                    options.push_str(option.text.as_ref());
                    options.push('\n');
                }
                options.truncate(options.trim_end_matches('\n').len());
                let mut text = String::with_capacity(choice.prompt.len() + 1 + options.len());
                text.push_str(choice.prompt.as_ref());
                text.push('\n');
                text.push_str(&options);
                text
            }
            EventCompiled::Scene(scene) => self.render_scene(scene, visual),
            EventCompiled::Patch(_) => "Patch applied".to_string(),
            EventCompiled::Jump { target_ip } => format!("Jump to {target_ip}"),
            EventCompiled::SetFlag { flag_id, value } => {
                format!("Flag {flag_id} = {value}")
            }
            EventCompiled::SetVar { var_id, value } => {
                format!("Var {var_id} = {value}")
            }
            EventCompiled::JumpIf { target_ip, .. } => {
                format!("JumpIf to {target_ip}")
            }
            EventCompiled::ExtCall { command, args } => {
                format!("ExtCall {command}({})", args.join(", "))
            }
            EventCompiled::AudioAction(_) => "Audio Action".to_string(),
            EventCompiled::Transition(_) => "Transition".to_string(),
            EventCompiled::SetCharacterPosition(pos) => {
                format!("SetCharacterPosition {} ({}, {})", pos.name, pos.x, pos.y)
            }
        };
        RenderOutput { text }
    }
}
