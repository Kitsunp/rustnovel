use visual_novel_engine::runtime::{SceneFrame, UiState};

use crate::AssetStore;

/// Immutable render payload shared by software and hardware backends.
#[derive(Clone, Copy)]
pub struct RenderFrame<'a> {
    pub ui: &'a UiState,
    pub scene_frame: &'a SceneFrame,
    pub assets: &'a dyn AssetStore,
}

/// Abstraction for the rendering backend (Software vs Hardware).
pub trait RenderBackend {
    /// Resizes the internal surface/buffers.
    fn resize(&mut self, width: u32, height: u32) -> Result<(), String>;

    /// Renders the current scene frame to the target.
    fn render(&mut self, frame: RenderFrame<'_>) -> Result<(), String>;
}
