use crate::audio::AudioCommand;
use crate::error::VnResult;
use crate::event::EventCompiled;
use crate::localization::LocalizationCatalog;
use crate::resource::ResourceLimiter;
use crate::runtime::Engine;
use crate::security::SecurityPolicy;

use super::super::NodeGraph;
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
        let script = graph.to_script_strict()?;
        let mut engine = Engine::new(
            script,
            SecurityPolicy::default(),
            ResourceLimiter::default(),
        )?;
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
