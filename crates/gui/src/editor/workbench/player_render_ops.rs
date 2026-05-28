use super::*;

impl EditorWorkbench {
    pub fn player_visual_preferences(&self) -> crate::editor::PlayerVisualPreferences {
        crate::editor::PlayerVisualPreferences {
            preview_quality: self.composer_preview_quality,
            stage_fit: self.composer_stage_fit,
        }
    }

    pub fn render_player_mode(&mut self, ctx: &egui::Context) {
        self.ensure_player_audio_backend();
        let stage_resolution = self
            .manifest
            .as_ref()
            .map(|manifest| manifest.settings.resolution);
        let visual_prefs = self.player_visual_preferences();
        let player_menu = self
            .manifest
            .as_ref()
            .map(|manifest| manifest.settings.player_menu.clone())
            .unwrap_or_default();
        let mut visual_context = crate::editor::player_ui::PlayerVisualContext {
            project_root: self.project_root.as_deref(),
            stage_resolution,
            preview_quality: visual_prefs.preview_quality,
            stage_fit: visual_prefs.stage_fit,
            background_fit: self.composer_background_fit_for_node(self.selected_node),
            image_cache: &mut self.composer_image_cache,
            image_failures: &mut self.composer_image_failures,
            resource_service: &mut self.resource_service,
        };
        let audio_commands = crate::editor::player_ui::render_player_ui(
            &mut self.engine,
            &mut self.toast,
            &mut self.player_state,
            &mut self.player_locale,
            &self.localization_catalog,
            &player_menu,
            ctx,
            &mut visual_context,
        );
        self.apply_player_audio_commands(audio_commands);
    }
}
