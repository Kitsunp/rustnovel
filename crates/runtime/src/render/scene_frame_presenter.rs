use visual_novel_engine::{
    resolve_layout, validate_ui_theme, DisplayProfile, LayoutPolicy, RenderCommand, SceneFrame,
    SceneFramePresenter, StageProfile, UiResponse, UiTheme,
};

#[derive(Clone, Debug, Default)]
pub struct RuntimeSceneFramePresenter {
    pub last_command_count: usize,
    pub last_image_count: usize,
    pub last_button_count: usize,
}

impl SceneFramePresenter for RuntimeSceneFramePresenter {
    fn present(
        &mut self,
        frame: &SceneFrame,
        display: &DisplayProfile,
        theme: &UiTheme,
    ) -> UiResponse {
        let validation = validate_ui_theme(theme);
        let mut diagnostics = validation.warnings;
        diagnostics.extend(validation.errors);
        if frame.layout.is_none() {
            diagnostics.push(format!(
                "scene frame '{}' has no resolved layout; presenter used default policy",
                frame.frame_schema
            ));
            let _layout = resolve_layout(
                display.clone(),
                StageProfile::default(),
                LayoutPolicy::default(),
            );
        }
        self.last_command_count = frame.commands.len();
        self.last_image_count = frame
            .commands
            .iter()
            .filter(|command| matches!(command, RenderCommand::Image { .. }))
            .count();
        self.last_button_count = frame
            .commands
            .iter()
            .filter(|command| matches!(command, RenderCommand::Button { .. }))
            .count();
        UiResponse {
            activated_actions: Vec::new(),
            diagnostics,
        }
    }
}
