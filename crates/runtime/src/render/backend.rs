use visual_novel_engine::UiState;

/// Abstraction for the rendering backend (Software vs Hardware).
pub trait RenderBackend {
    /// Resizes the internal surface/buffers.
    fn resize(&mut self, width: u32, height: u32);

    /// Renders the current UI state to the target.
    fn render(&mut self, ui: &UiState) -> Result<(), String>;
}
