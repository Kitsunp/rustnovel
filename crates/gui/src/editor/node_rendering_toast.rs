use super::*;

/// Renders a toast notification if one is active.
///
/// Call this at the end of the UI rendering to ensure toast appears on top.
pub fn render_toast(ui: &egui::Ui, toast: &mut Option<ToastState>) {
    let Some(t) = toast else {
        return;
    };

    // Decrement frame counter
    if t.frames_remaining > 0 {
        t.frames_remaining -= 1;
    }

    // Calculate alpha for fade out (last 30 frames)
    let alpha = if t.frames_remaining < 30 {
        (t.frames_remaining as f32 / 30.0 * 255.0) as u8
    } else {
        255
    };

    if t.frames_remaining == 0 {
        *toast = None;
        return;
    }

    // Render toast in bottom-right corner
    let screen_rect = ui.ctx().screen_rect();
    let toast_pos = egui::pos2(screen_rect.max.x - 20.0, screen_rect.max.y - 60.0);

    egui::Area::new(egui::Id::new("toast_notification"))
        .fixed_pos(toast_pos)
        .pivot(egui::Align2::RIGHT_BOTTOM)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            let bg_color = t.kind.color().linear_multiply(0.9);
            let bg_color = egui::Color32::from_rgba_unmultiplied(
                bg_color.r(),
                bg_color.g(),
                bg_color.b(),
                alpha,
            );

            egui::Frame::none()
                .fill(bg_color)
                .rounding(8.0)
                .inner_margin(egui::Margin::symmetric(16.0, 10.0))
                .show(ui, |ui| {
                    let text_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(t.kind.icon())
                                .size(16.0)
                                .color(text_color),
                        );
                        ui.label(egui::RichText::new(&t.message).color(text_color));
                    });
                });
        });

    // Request repaint to animate
    ui.ctx().request_repaint();
}
