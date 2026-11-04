use crate::audio::AudioCommand;
use crate::error::VnResult;
use crate::event::EventCompiled;
use crate::localization::LocalizationCatalog;
use crate::resource::ResourceLimiter;
use crate::runtime::Engine;
use crate::script::{ScriptCompiled, ScriptRaw};

use super::super::{NodeGraph, PreparedRuntimeEntry};
use super::{
    build_presentation_snapshot, compose_scene_snapshot, ComposerSnapshot, PresentationSnapshot,
};

#[derive(Clone, Debug)]
pub struct ComposerPreviewSession {
    engine: Engine,
    pending_audio: Vec<AudioCommand>,
}

impl ComposerPreviewSession {
    pub fn start_from_node(graph: &NodeGraph, node_id: u32) -> VnResult<Self> {
        let (script, compiled) = super::super::script_sync::strict_projection(graph)?;
        let mut engine = preview_engine_from_projection(script, compiled)?;
        let pending_audio = jump_preview_engine(&mut engine, &format!("node_{node_id}"))?;
        Ok(Self {
            engine,
            pending_audio,
        })
    }

    pub fn advance(&mut self) -> VnResult<()> {
        if let Some(commands) = advance_preview_engine(&mut self.engine, None)? {
            self.pending_audio.extend(commands);
        }
        Ok(())
    }

    pub fn choose(&mut self, option_index: usize) -> VnResult<()> {
        self.engine.choose(option_index)?;
        self.pending_audio.extend(self.engine.take_audio_commands());
        Ok(())
    }

    pub fn take_audio_commands(&mut self) -> Vec<AudioCommand> {
        std::mem::take(&mut self.pending_audio)
    }

    pub fn snapshot(
        &self,
        graph: &NodeGraph,
        stage_resolution: Option<(u32, u32)>,
        locale: Option<&str>,
        catalog: Option<&LocalizationCatalog>,
    ) -> ComposerSnapshot {
        compose_scene_snapshot(
            graph,
            None,
            stage_resolution,
            Some(&self.engine),
            locale,
            catalog,
        )
    }

    pub fn presentation_snapshot(
        &self,
        graph: &NodeGraph,
        stage_resolution: Option<(u32, u32)>,
        locale: Option<&str>,
        catalog: Option<&LocalizationCatalog>,
    ) -> PresentationSnapshot {
        build_presentation_snapshot(
            graph,
            None,
            stage_resolution,
            Some(&self.engine),
            locale,
            catalog,
        )
    }
}

fn preview_engine_from_projection(script: ScriptRaw, compiled: ScriptCompiled) -> VnResult<Engine> {
    let policy = super::super::PREVIEW_SECURITY_POLICY;
    let limits = ResourceLimiter::default();
    PreparedRuntimeEntry::from_projection(script, compiled).into_engine(policy, limits)
}

/// Returns None when the preview is waiting for a choice; otherwise returns the
/// audio produced by the same runtime transition used by all preview clients.
pub fn advance_preview_engine(
    engine: &mut Engine,
    choice: Option<usize>,
) -> VnResult<Option<Vec<AudioCommand>>> {
    let commands = match engine.current_event()? {
        EventCompiled::Choice(_) => {
            let Some(index) = choice else {
                return Ok(None);
            };
            engine.choose(index)?;
            engine.take_audio_commands()
        }
        EventCompiled::ExtCall { .. } => {
            engine.resume()?;
            engine.take_audio_commands()
        }
        _ => engine.step()?.0,
    };
    Ok(Some(commands))
}

pub fn jump_preview_engine(engine: &mut Engine, label: &str) -> VnResult<Vec<AudioCommand>> {
    engine.jump_to_label(label)?;
    engine.clear_session_history();
    Ok(engine.take_audio_commands())
}

#[cfg(test)]
mod projection_tests {
    use super::*;
    use crate::event::{DialogueRaw, EventRaw};
    use std::collections::BTreeMap;

    #[test]
    fn validates_raw_before_compiled_and_rejects_invalid_compiled() {
        let mut raw = ScriptRaw::new(
            vec![EventRaw::Dialogue(DialogueRaw {
                speaker: String::new(),
                text: "hello".into(),
            })],
            BTreeMap::from([("start".to_string(), 0)]),
        );
        let mut compiled = raw.compile().unwrap();
        compiled.start_ip = u32::MAX;
        let error = preview_engine_from_projection(raw.clone(), compiled.clone()).unwrap_err();
        assert!(error
            .to_string()
            .contains("compiled start_ip outside events"));
        if let EventRaw::Dialogue(dialogue) = &mut raw.events[0] {
            dialogue.text = "x".repeat(ResourceLimiter::default().max_text_length + 1);
        }
        let error = preview_engine_from_projection(raw, compiled).unwrap_err();
        assert!(error.to_string().contains("dialogue text"));
    }
}
