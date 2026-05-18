use eframe::egui;

const STATUS_HEIGHT: f32 = 28.0;
const VIEWPORT_VERTICAL_BUDGET: f32 = 0.78;

pub(crate) fn composer_viewport_size(available: egui::Vec2, stage_size: (f32, f32)) -> egui::Vec2 {
    crate::editor::visual_composer_preview::stage_viewport_size(
        available,
        stage_size,
        STATUS_HEIGHT,
        VIEWPORT_VERTICAL_BUDGET,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_keeps_tall_composer_from_swallowing_editor_space() {
        let size = composer_viewport_size(egui::vec2(900.0, 1000.0), (1280.0, 720.0));
        assert_eq!(size.x, 900.0);
        assert!(
            size.y <= 900.0 * 9.0 / 16.0 + 12.0 + 0.1,
            "viewport should stop at stage aspect instead of consuming all vertical space"
        );
        assert!(size.y < 700.0);
    }

    #[test]
    fn viewport_reserves_status_space_in_short_windows() {
        let size = composer_viewport_size(egui::vec2(720.0, 180.0), (1280.0, 720.0));
        assert!(size.y <= 152.0);
        assert!(size.y >= 96.0);
    }

    #[test]
    fn viewport_degrades_without_negative_height_when_space_is_tiny() {
        let size = composer_viewport_size(egui::vec2(320.0, 20.0), (1280.0, 720.0));
        assert_eq!(size.x, 320.0);
        assert_eq!(size.y, 0.0);
    }
}
