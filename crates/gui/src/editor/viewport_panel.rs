//! Viewport panel for the editor workbench.
//!
//! Displays a scene preview with deterministic glyph rendering for each entity kind.

use eframe::egui;
use visual_novel_engine::{Engine, EntityKind, SceneState};

/// Viewport panel widget.
pub struct ViewportPanel<'a> {
    scene: &'a SceneState,
    engine: &'a Option<Engine>,
}

impl<'a> ViewportPanel<'a> {
    pub fn new(scene: &'a SceneState, engine: &'a Option<Engine>) -> Self {
        Self { scene, engine }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Viewport");
        ui.separator();

        let available_size = ui.available_size();
        let viewport_rect = egui::Rect::from_min_size(
            ui.cursor().min,
            egui::vec2(available_size.x, available_size.y - 30.0),
        );
        let painter = ui.painter();

        painter.rect_filled(viewport_rect, 5.0, egui::Color32::from_rgb(24, 24, 34));
        painter.rect_stroke(
            viewport_rect,
            5.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(110)),
        );

        if self.scene.is_empty() {
            painter.text(
                viewport_rect.center(),
                egui::Align2::CENTER_CENTER,
                "No entities in scene",
                egui::FontId::proportional(16.0),
                egui::Color32::from_gray(140),
            );
        } else {
            for entity in self.scene.iter_sorted() {
                let scale = (entity.transform.scale as f32 / 1000.0).clamp(0.25, 4.0);
                let anchor = egui::pos2(
                    viewport_rect.min.x + entity.transform.x as f32,
                    viewport_rect.min.y + entity.transform.y as f32,
                );
                let alpha = ((entity.transform.opacity.min(1000) as f32 / 1000.0) * 255.0) as u8;

                match &entity.kind {
                    EntityKind::Image(data) => {
                        let rect =
                            egui::Rect::from_min_size(anchor, egui::vec2(100.0, 100.0) * scale);
                        if !rect.intersects(viewport_rect) {
                            continue;
                        }
                        draw_image_glyph(
                            painter,
                            rect,
                            alpha,
                            truncate_path(data.path.as_ref()),
                            data.tint,
                        );
                    }
                    EntityKind::Text(data) => {
                        painter.text(
                            anchor,
                            egui::Align2::LEFT_TOP,
                            &data.content,
                            egui::FontId::proportional((data.font_size as f32 * scale).max(8.0)),
                            egui::Color32::from_rgba_unmultiplied(220, 220, 240, alpha),
                        );
                    }
                    EntityKind::Character(data) => {
                        let rect =
                            egui::Rect::from_min_size(anchor, egui::vec2(84.0, 124.0) * scale);
                        if !rect.intersects(viewport_rect) {
                            continue;
                        }
                        draw_character_glyph(
                            painter,
                            rect,
                            alpha,
                            data.name.as_ref(),
                            data.expression.as_deref(),
                        );
                    }
                    EntityKind::Video(data) => {
                        let rect =
                            egui::Rect::from_min_size(anchor, egui::vec2(160.0, 90.0) * scale);
                        if !rect.intersects(viewport_rect) {
                            continue;
                        }
                        draw_video_glyph(painter, rect, alpha, truncate_path(data.path.as_ref()));
                    }
                    EntityKind::Audio(data) => {
                        draw_audio_glyph(
                            painter,
                            anchor,
                            alpha,
                            truncate_path(data.path.as_ref()),
                            data.volume,
                            data.looping,
                        );
                    }
                }
            }
        }

        ui.allocate_rect(viewport_rect, egui::Sense::hover());

        ui.horizontal(|ui| {
            ui.label(format!(
                "Size: {}x{}",
                available_size.x as i32,
                (available_size.y - 30.0) as i32
            ));
            ui.separator();
            ui.label(format!("Entities: {}", self.scene.len()));

            if let Some(engine) = self.engine {
                ui.separator();
                if let Ok(event) = engine.current_event() {
                    ui.label(format!(
                        "Event: {}",
                        format!("{:?}", event).chars().take(50).collect::<String>()
                    ));
                }
            }
        });
    }
}

fn draw_image_glyph(
    painter: &egui::Painter,
    rect: egui::Rect,
    alpha: u8,
    label: String,
    tint: Option<u32>,
) {
    let fallback = egui::Color32::from_rgba_unmultiplied(84, 120, 176, alpha);
    let fill = tint
        .map(rgba_from_u32)
        .unwrap_or(fallback)
        .gamma_multiply(alpha as f32 / 255.0);
    painter.rect_filled(rect, 4.0, fill);
    painter.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(215)),
    );
    painter.line_segment(
        [rect.left_top(), rect.right_bottom()],
        egui::Stroke::new(1.0, egui::Color32::from_gray(230)),
    );
    painter.line_segment(
        [rect.right_top(), rect.left_bottom()],
        egui::Stroke::new(1.0, egui::Color32::from_gray(230)),
    );
    painter.text(
        rect.center_bottom() - egui::vec2(0.0, 8.0),
        egui::Align2::CENTER_BOTTOM,
        label,
        egui::FontId::proportional(11.0),
        egui::Color32::from_gray(20),
    );
}

fn draw_character_glyph(
    painter: &egui::Painter,
    rect: egui::Rect,
    alpha: u8,
    name: &str,
    expression: Option<&str>,
) {
    let frame = egui::Color32::from_rgba_unmultiplied(62, 102, 152, alpha);
    painter.rect_filled(rect, 6.0, frame);
    painter.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(220)),
    );

    let head_r = (rect.width().min(rect.height()) * 0.16).max(6.0);
    let head_center = egui::pos2(rect.center().x, rect.top() + rect.height() * 0.28);
    painter.circle_filled(
        head_center,
        head_r,
        egui::Color32::from_rgba_unmultiplied(232, 237, 245, alpha),
    );

    let torso = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.center().y + rect.height() * 0.08),
        egui::vec2(rect.width() * 0.44, rect.height() * 0.46),
    );
    painter.rect_filled(
        torso,
        4.0,
        egui::Color32::from_rgba_unmultiplied(210, 220, 242, alpha),
    );

    painter.text(
        rect.center_bottom() - egui::vec2(0.0, 10.0),
        egui::Align2::CENTER_BOTTOM,
        name,
        egui::FontId::proportional(11.0),
        egui::Color32::WHITE,
    );
    if let Some(expression) = expression {
        painter.text(
            rect.center_bottom() - egui::vec2(0.0, 24.0),
            egui::Align2::CENTER_BOTTOM,
            expression,
            egui::FontId::proportional(10.0),
            egui::Color32::from_gray(230),
        );
    }
}

fn draw_video_glyph(painter: &egui::Painter, rect: egui::Rect, alpha: u8, label: String) {
    painter.rect_filled(
        rect,
        4.0,
        egui::Color32::from_rgba_unmultiplied(38, 44, 68, alpha),
    );
    painter.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(200)),
    );

    let triangle = vec![
        egui::pos2(rect.center().x - 8.0, rect.center().y - 10.0),
        egui::pos2(rect.center().x - 8.0, rect.center().y + 10.0),
        egui::pos2(rect.center().x + 10.0, rect.center().y),
    ];
    painter.add(egui::Shape::convex_polygon(
        triangle,
        egui::Color32::from_rgba_unmultiplied(238, 242, 250, alpha),
        egui::Stroke::NONE,
    ));
    painter.text(
        rect.center_bottom() - egui::vec2(0.0, 8.0),
        egui::Align2::CENTER_BOTTOM,
        label,
        egui::FontId::proportional(11.0),
        egui::Color32::from_gray(225),
    );
}

fn draw_audio_glyph(
    painter: &egui::Painter,
    anchor: egui::Pos2,
    alpha: u8,
    label: String,
    volume: u32,
    looping: bool,
) {
    let vol = (volume.min(1000) as f32 / 1000.0).clamp(0.0, 1.0);
    let tag = egui::Rect::from_min_size(anchor, egui::vec2(180.0, 22.0));
    painter.rect_filled(
        tag,
        5.0,
        egui::Color32::from_rgba_unmultiplied(52, 96, 74, alpha),
    );
    painter.rect_stroke(
        tag,
        5.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(195)),
    );

    let meter = egui::Rect::from_min_size(
        egui::pos2(tag.left() + 4.0, tag.bottom() - 6.0),
        egui::vec2((tag.width() - 8.0) * vol, 2.0),
    );
    painter.rect_filled(
        meter,
        1.0,
        egui::Color32::from_rgba_unmultiplied(192, 240, 205, alpha),
    );

    let loop_suffix = if looping { " [loop]" } else { "" };
    painter.text(
        tag.left_center() + egui::vec2(6.0, -2.0),
        egui::Align2::LEFT_CENTER,
        format!("audio: {label}{loop_suffix}"),
        egui::FontId::proportional(11.0),
        egui::Color32::from_gray(235),
    );
}

fn truncate_path(path: &str) -> String {
    if path.len() > 18 {
        format!("...{}", &path[path.len() - 15..])
    } else {
        path.to_string()
    }
}

fn rgba_from_u32(rgba: u32) -> egui::Color32 {
    let r = ((rgba >> 24) & 0xFF) as u8;
    let g = ((rgba >> 16) & 0xFF) as u8;
    let b = ((rgba >> 8) & 0xFF) as u8;
    let a = (rgba & 0xFF) as u8;
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}
