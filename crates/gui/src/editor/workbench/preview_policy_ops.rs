use super::*;

impl EditorWorkbench {
    pub fn composer_background_fit_for_node(
        &self,
        node_id: Option<u32>,
    ) -> crate::editor::BackgroundFit {
        node_id
            .and_then(|id| self.composer_background_fit_overrides.get(&id.to_string()))
            .copied()
            .unwrap_or(self.composer_default_background_fit)
    }
}
