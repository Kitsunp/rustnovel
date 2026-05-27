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
}

impl ComposerPreviewSession {
    pub fn start_from_node(graph: &NodeGraph, node_id: u32) -> VnResult<Self> {
        let script = graph.to_script_strict()?;
        let mut engine = Engine::new(
            script,
            SecurityPolicy::default(),
            ResourceLimiter::default(),
        )?;
        engine.jump_to_label(&format!("node_{node_id}"))?;
        Ok(Self { engine })
    }

    pub fn advance(&mut self) -> VnResult<()> {
        match self.engine.current_event()? {
            EventCompiled::Choice(_) => Ok(()),
            EventCompiled::ExtCall { .. } => self.engine.resume(),
            _ => self.engine.step().map(|_| ()),
        }
    }

    pub fn choose(&mut self, option_index: usize) -> VnResult<()> {
        self.engine.choose(option_index).map(|_| ())
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
