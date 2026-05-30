use eframe::egui;
use visual_novel_engine::{
    resolve_layout, validate_ui_theme, DisplayProfile, ImageFit, LayoutPolicy, LayoutRect,
    RenderCommand, SceneFrame, SceneFramePresenter, StageProfile, UiResponse, UiTheme,
};

#[derive(Clone, Debug, Default)]
pub struct EguiSceneFramePresenter {
    pub show_debug_bounds: bool,
}

impl SceneFramePresenter for EguiSceneFramePresenter {
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
        }
        let _layout = frame.layout.clone().unwrap_or_else(|| {
            resolve_layout(
                display.clone(),
                StageProfile::default(),
                LayoutPolicy::default(),
            )
        });
        UiResponse {
            activated_actions: Vec::new(),
            diagnostics,
        }
    }
}

impl EguiSceneFramePresenter {
    pub fn present_egui(
        &mut self,
        ui: &mut egui::Ui,
        frame: &SceneFrame,
        display: &DisplayProfile,
        theme: &UiTheme,
    ) -> UiResponse {
        let mut response = self.present(frame, display, theme);
        let layout = frame.layout.clone().unwrap_or_else(|| {
            resolve_layout(
                display.clone(),
                StageProfile::default(),
                LayoutPolicy::default(),
            )
        });
        let available = ui.available_size_before_wrap();
        let desired = egui::vec2(
            layout.display.logical_size[0]
                .min(available.x.max(1.0))
                .max(1.0),
            layout.display.logical_size[1]
                .min(available.y.max(1.0))
                .max(1.0),
        );
        let (viewport_rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
        let sx = viewport_rect.width() / layout.display.logical_size[0].max(1.0);
        let sy = viewport_rect.height() / layout.display.logical_size[1].max(1.0);
        let stage_rect = scale_rect(viewport_rect, layout.stage_rect, sx, sy);
        let painter = ui.painter_at(viewport_rect);

        for command in &frame.commands {
            match command {
                RenderCommand::Clear { color } => {
                    painter.rect_filled(viewport_rect, 0.0, color_token(theme, color));
                }
                RenderCommand::Panel { style, rect } => {
                    let fill = component_fill(theme, style);
                    painter.rect_filled(scale_rect(stage_rect, *rect, sx, sy), 4.0, fill);
                }
                RenderCommand::Text { text, style, rect } => {
                    let color = color_token(theme, style);
                    painter.text(
                        scale_rect(stage_rect, *rect, sx, sy).left_top(),
                        egui::Align2::LEFT_TOP,
                        text,
                        egui::FontId::proportional(16.0),
                        color,
                    );
                }
                RenderCommand::Image {
                    asset,
                    rect,
                    fit,
                    z: _,
                } => {
                    let rect = scale_rect(stage_rect, *rect, sx, sy);
                    let stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(120));
                    painter.rect_stroke(rect, 2.0, stroke);
                    let fit = match fit {
                        ImageFit::Contain => "contain",
                        ImageFit::Cover => "cover",
                        ImageFit::Stretch => "stretch",
                    };
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{asset}\n{fit}"),
                        egui::FontId::monospace(12.0),
                        egui::Color32::from_gray(190),
                    );
                }
                RenderCommand::Button {
                    id,
                    label,
                    style: _,
                    rect,
                } => {
                    let rect = scale_rect(stage_rect, *rect, sx, sy);
                    if ui.put(rect, egui::Button::new(label)).clicked() {
                        response.activated_actions.push(id.clone());
                    }
                }
            }
        }

        if self.show_debug_bounds {
            painter.rect_stroke(
                stage_rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 180, 255)),
            );
        }
        response
    }
}

fn scale_rect(origin: egui::Rect, rect: LayoutRect, sx: f32, sy: f32) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(origin.left() + rect.x * sx, origin.top() + rect.y * sy),
        egui::vec2(rect.width * sx, rect.height * sy),
    )
}

fn component_fill(theme: &UiTheme, style: &str) -> egui::Color32 {
    theme
        .components
        .components
        .get(style)
        .and_then(|component| component.background.as_deref())
        .map(|token| color_token(theme, token))
        .unwrap_or_else(|| color_token(theme, style))
}

fn color_token(theme: &UiTheme, token: &str) -> egui::Color32 {
    theme
        .colors
        .get(token)
        .map(String::as_str)
        .unwrap_or(token)
        .strip_prefix('#')
        .and_then(parse_hex_color)
        .unwrap_or(egui::Color32::TRANSPARENT)
}

fn parse_hex_color(hex: &str) -> Option<egui::Color32> {
    if !(hex.len() == 6 || hex.len() == 8) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    let a = if hex.len() == 8 {
        u8::from_str_radix(&hex[6..8], 16).ok()?
    } else {
        255
    };
    Some(egui::Color32::from_rgba_unmultiplied(r, g, b, a))
}
