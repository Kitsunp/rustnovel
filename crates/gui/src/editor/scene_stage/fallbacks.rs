use eframe::egui;

pub(super) fn paint_caption(ui: &egui::Ui, rect: egui::Rect, label: &str) {
    ui.painter().text(
        rect.center_bottom() - egui::vec2(0.0, 8.0),
        egui::Align2::CENTER_BOTTOM,
        label,
        egui::FontId::default(),
        egui::Color32::WHITE,
    );
}

pub(super) fn paint_asset_fallback(
    ui: &egui::Ui,
    rect: egui::Rect,
    is_selected: bool,
    kind: &str,
    label: &str,
) {
    let color = if is_selected {
        egui::Color32::from_rgb(120, 170, 220)
    } else {
        egui::Color32::from_rgb(80, 110, 155)
    };
    ui.painter().rect_filled(rect, 4.0, color);
    ui.painter().rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(210)),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{kind}\n{label}"),
        egui::FontId::default(),
        egui::Color32::WHITE,
    );
}

pub(super) fn paint_character_fallback(
    ui: &egui::Ui,
    rect: egui::Rect,
    is_selected: bool,
    name: &str,
    expression: Option<&str>,
) {
    let fill = if is_selected {
        egui::Color32::from_rgb(92, 135, 190)
    } else {
        egui::Color32::from_rgb(62, 102, 152)
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter().circle_filled(
        egui::pos2(rect.center().x, rect.top() + rect.height() * 0.27),
        (rect.width().min(rect.height()) * 0.15).max(6.0),
        egui::Color32::from_rgb(232, 237, 245),
    );
    let torso = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.center().y + rect.height() * 0.09),
        egui::vec2(rect.width() * 0.44, rect.height() * 0.44),
    );
    ui.painter()
        .rect_filled(torso, 4.0, egui::Color32::from_rgb(210, 220, 242));
    paint_caption(ui, rect, name);
    if let Some(expression) = expression {
        ui.painter().text(
            rect.center_bottom() - egui::vec2(0.0, 24.0),
            egui::Align2::CENTER_BOTTOM,
            expression,
            egui::FontId::proportional(10.0),
            egui::Color32::from_gray(230),
        );
    }
}

pub(super) fn paint_audio_surface(
    ui: &egui::Ui,
    rect: egui::Rect,
    is_selected: bool,
    label: &str,
    looping: bool,
) {
    let color = if is_selected {
        egui::Color32::from_rgb(110, 165, 120)
    } else {
        egui::Color32::from_rgb(90, 140, 100)
    };
    ui.painter().rect_filled(rect, 4.0, color);
    let suffix = if looping { " loop" } else { "" };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("Audio {label}{suffix}"),
        egui::FontId::default(),
        egui::Color32::WHITE,
    );
}

pub(super) fn paint_video_surface(
    ui: &egui::Ui,
    rect: egui::Rect,
    is_selected: bool,
    label: &str,
    looping: bool,
) {
    let color = if is_selected {
        egui::Color32::from_rgb(82, 92, 128)
    } else {
        egui::Color32::from_rgb(42, 48, 72)
    };
    ui.painter().rect_filled(rect, 4.0, color);
    let triangle = vec![
        egui::pos2(rect.center().x - 8.0, rect.center().y - 10.0),
        egui::pos2(rect.center().x - 8.0, rect.center().y + 10.0),
        egui::pos2(rect.center().x + 10.0, rect.center().y),
    ];
    ui.painter().add(egui::Shape::convex_polygon(
        triangle,
        egui::Color32::WHITE,
        egui::Stroke::NONE,
    ));
    let suffix = if looping { " loop" } else { "" };
    paint_caption(ui, rect, &format!("{label}{suffix}"));
}

pub(super) fn paint_text_surface(
    ui: &egui::Ui,
    rect: egui::Rect,
    is_selected: bool,
    content: &str,
    font_size: u32,
    color: u32,
) {
    let bg = if is_selected {
        egui::Color32::from_rgb(72, 78, 100)
    } else {
        egui::Color32::from_rgb(42, 46, 60)
    };
    ui.painter().rect_filled(rect, 4.0, bg);
    ui.painter().text(
        rect.left_center() + egui::vec2(8.0, 0.0),
        egui::Align2::LEFT_CENTER,
        content,
        egui::FontId::proportional(font_size.max(8) as f32),
        rgba_from_u32(color),
    );
}

fn rgba_from_u32(rgba: u32) -> egui::Color32 {
    let r = ((rgba >> 24) & 0xFF) as u8;
    let g = ((rgba >> 16) & 0xFF) as u8;
    let b = ((rgba >> 8) & 0xFF) as u8;
    let a = (rgba & 0xFF) as u8;
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}
