//! Rendering helpers for compiled events.

use std::fmt::Write;

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
            let _ = writeln!(output, "Background: {background}");
        }
        if let Some(music) = scene.music.as_deref().or(visual.music.as_deref()) {
            let _ = writeln!(output, "Music: {music}");
        }
        if !visual.characters.is_empty() {
            let mut roster = String::with_capacity(visual.characters.len() * 24);
            for (idx, character) in visual.characters.iter().enumerate() {
                if idx > 0 {
                    roster.push_str(", ");
                }
                roster.push_str(character.name.as_ref());
                if let Some(expression) = &character.expression {
                    let _ = write!(roster, " ({expression})");
                }
                if let Some(position) = &character.position {
                    let _ = write!(roster, " @ {position}");
                }
            }
            let _ = writeln!(output, "Characters: {roster}");
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
                    let _ = writeln!(options, "{}. {}", idx + 1, option.text);
                }
                options.truncate(options.trim_end_matches('\n').len());
                let mut text = String::with_capacity(choice.prompt.len() + 1 + options.len());
                let _ = write!(text, "{}\n{}", choice.prompt, options);
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
