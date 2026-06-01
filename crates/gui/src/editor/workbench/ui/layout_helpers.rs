use super::*;

pub(super) fn dock_rect(origin: egui::Pos2, rect: super::layout::WorkspacePanelRect) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(origin.x + rect.x, origin.y + rect.y),
        egui::vec2(rect.w.max(1.0), rect.h.max(1.0)),
    )
}

pub(super) fn vertical_splitter(ui: &mut egui::Ui, rect: egui::Rect) -> egui::Response {
    let response = ui
        .allocate_rect(rect, egui::Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    let color = if response.dragged() || response.hovered() {
        egui::Color32::from_gray(90)
    } else {
        egui::Color32::from_gray(46)
    };
    ui.painter()
        .rect_filled(rect.shrink2(egui::vec2(3.0, 0.0)), 0.0, color);
    response
}

pub(super) fn resize_width_override(
    target: &mut Option<f32>,
    reference_width: &mut Option<f32>,
    current: f32,
    delta: f32,
    min: f32,
    max: f32,
    available_width: f32,
) {
    if !delta.is_finite() || delta.abs() < 0.1 {
        return;
    }
    if available_width.is_finite() && available_width >= 360.0 {
        *reference_width = Some(available_width);
    }
    *target = Some((current + delta).clamp(min, max));
}
