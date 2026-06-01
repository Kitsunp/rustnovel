use eframe::egui;

const RESERVED_HEIGHT: f32 = 0.0;
const VIEWPORT_VERTICAL_BUDGET: f32 = 1.0;

pub fn composer_viewport_size(available: egui::Vec2, stage_size: (f32, f32)) -> egui::Vec2 {
    crate::editor::visual_composer_preview::stage_viewport_size(
        available,
        stage_size,
        RESERVED_HEIGHT,
        VIEWPORT_VERTICAL_BUDGET,
    )
}
