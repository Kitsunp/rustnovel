use super::*;

impl EditorWorkbench {
    pub(crate) fn composer_background_fit_for_node(
        &self,
        node_id: Option<u32>,
    ) -> crate::editor::BackgroundFit {
        node_id
            .and_then(|id| self.composer_background_fit_overrides.get(&id.to_string()))
            .copied()
            .unwrap_or(self.composer_default_background_fit)
    }

    pub(crate) fn set_composer_background_fit_for_node(
        &mut self,
        node_id: Option<u32>,
        fit: crate::editor::BackgroundFit,
    ) {
        if let Some(node_id) = node_id {
            self.composer_background_fit_overrides
                .insert(node_id.to_string(), fit);
        } else {
            self.composer_default_background_fit = fit;
        }
    }
}
