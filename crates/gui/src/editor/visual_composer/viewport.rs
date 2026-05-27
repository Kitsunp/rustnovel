use eframe::egui;

const STATUS_HEIGHT: f32 = 28.0;
const VIEWPORT_VERTICAL_BUDGET: f32 = 0.78;

pub fn composer_viewport_size(available: egui::Vec2, stage_size: (f32, f32)) -> egui::Vec2 {
    crate::editor::visual_composer_preview::stage_viewport_size(
        available,
        stage_size,
        STATUS_HEIGHT,
        VIEWPORT_VERTICAL_BUDGET,
    )
}
