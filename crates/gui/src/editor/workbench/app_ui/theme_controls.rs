use super::*;

pub(super) fn color_code_control(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    let before = value.clone();
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        let preview = visual_novel_engine::preview_theme_color(value);
        let mut color = preview.rgba.unwrap_or(visual_novel_engine::ThemeColorRgba {
            r: 32,
            g: 36,
            b: 44,
            a: 255,
        });
        draw_color_swatch(ui, color, preview.valid);
        let mut egui_color =
            egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
        if egui::color_picker::color_edit_button_srgba(
            ui,
            &mut egui_color,
            egui::color_picker::Alpha::BlendOrAdditive,
        )
        .changed()
        {
            color = visual_novel_engine::ThemeColorRgba {
                r: egui_color.r(),
                g: egui_color.g(),
                b: egui_color.b(),
                a: egui_color.a(),
            };
            let keep_alpha = value.trim().trim_start_matches('#').len() == 8 || color.a < 255;
            *value = visual_novel_engine::format_theme_color_code_with_alpha(color, keep_alpha);
        }
        ui.add_sized(
            [116.0, 20.0],
            egui::TextEdit::singleline(value).font(egui::TextStyle::Monospace),
        );
        if let Some(error) = preview.error {
            ui.colored_label(egui::Color32::from_rgb(255, 130, 110), error);
        }
    });
    before != *value
}

pub(super) fn player_menu_color_control(
    ui: &mut egui::Ui,
    label: &str,
    color: &mut visual_novel_engine::PlayerMenuColor,
) -> bool {
    let before = *color;
    let mut code = visual_novel_engine::format_theme_color_code_with_alpha(
        visual_novel_engine::ThemeColorRgba {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        },
        true,
    );
    if color_code_control(ui, label, &mut code) {
        if let Ok(parsed) = visual_novel_engine::parse_theme_color_code(&code) {
            color.r = parsed.r;
            color.g = parsed.g;
            color.b = parsed.b;
            color.a = parsed.a;
        }
    }
    before != *color
}

pub(super) fn typography_token_control(
    ui: &mut egui::Ui,
    key: &str,
    token: &mut visual_novel_engine::TypographyToken,
    sample: &str,
) -> bool {
    let before = token.clone();
    ui.group(|ui| {
        ui.label(key);
        ui.horizontal_wrapped(|ui| {
            ui.label("Font");
            ui.text_edit_singleline(&mut token.font_family);
        });
        ui.add(egui::Slider::new(&mut token.size, 8.0..=64.0).text("Size"));
        ui.add(egui::Slider::new(&mut token.weight, 100..=900).text("Weight"));
        ui.add(egui::Slider::new(&mut token.line_height, 0.8..=2.4).text("Line height"));
        render_typography_preview(ui, token, sample);
    });
    before != *token
}

pub(super) fn render_theme_preview(
    ui: &mut egui::Ui,
    theme: &visual_novel_engine::UiTheme,
    sample: &str,
) {
    let stage = theme_color(
        theme,
        "stage.background",
        egui::Color32::from_rgb(16, 18, 24),
    );
    let dialogue_bg = theme_color(
        theme,
        "dialogue.background",
        egui::Color32::from_rgba_unmultiplied(8, 8, 14, 220),
    );
    let dialogue_text = theme_color(theme, "dialogue.text", egui::Color32::WHITE);
    let speaker = theme_color(
        theme,
        "dialogue.speaker",
        egui::Color32::from_rgb(158, 216, 255),
    );
    let choice_bg = theme_color(theme, "button.choice", egui::Color32::from_rgb(38, 52, 73));
    let choice_text = theme_color(theme, "choice.prompt", egui::Color32::WHITE);
    let dialogue_token = theme.typography.get("dialogue");
    let choice_token = theme.typography.get("choice").or(dialogue_token);

    egui::Frame::none()
        .fill(stage)
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(64, 70, 82)))
        .inner_margin(egui::Margin::same(12.0))
        .show(ui, |ui| {
            ui.set_min_height(150.0);
            ui.vertical_centered(|ui| {
                egui::Frame::none()
                    .fill(dialogue_bg)
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Sakura").color(speaker).strong());
                        let mut text = egui::RichText::new(sample).color(dialogue_text);
                        if let Some(token) = dialogue_token {
                            text = text.font(font_id(token));
                            if token.weight >= 600 {
                                text = text.strong();
                            }
                        }
                        ui.label(text);
                    });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    for text in ["Visit the courtyard", "Find the music room"] {
                        egui::Frame::none()
                            .fill(choice_bg)
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                            .show(ui, |ui| {
                                let mut rich = egui::RichText::new(text).color(choice_text);
                                if let Some(token) = choice_token {
                                    rich = rich.font(font_id(token));
                                    if token.weight >= 600 {
                                        rich = rich.strong();
                                    }
                                }
                                ui.label(rich);
                            });
                    }
                });
            });
        });
}

fn render_typography_preview(
    ui: &mut egui::Ui,
    token: &visual_novel_engine::TypographyToken,
    sample: &str,
) {
    let preview = visual_novel_engine::preview_typography_token(token, sample);
    let mut text = egui::RichText::new(preview.sample).font(font_id(token));
    if token.weight >= 600 {
        text = text.strong();
    }
    ui.label(text);
    ui.small(format!(
        "{} px line height | estimated {:.0} x {:.0}",
        preview.line_height_px.round(),
        preview.estimated_width.round(),
        preview.estimated_height.round()
    ));
}

fn draw_color_swatch(ui: &mut egui::Ui, color: visual_novel_engine::ThemeColorRgba, valid: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
    let fill = egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
    ui.painter().circle_filled(rect.center(), 8.0, fill);
    let stroke = if valid {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(210, 215, 225))
    } else {
        egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 120, 100))
    };
    ui.painter().circle_stroke(rect.center(), 8.0, stroke);
}

fn theme_color(
    theme: &visual_novel_engine::UiTheme,
    key: &str,
    fallback: egui::Color32,
) -> egui::Color32 {
    theme
        .colors
        .get(key)
        .and_then(|value| visual_novel_engine::parse_theme_color_code(value).ok())
        .map(|color| egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a))
        .unwrap_or(fallback)
}

fn font_id(token: &visual_novel_engine::TypographyToken) -> egui::FontId {
    let family = if token.font_family.trim().eq_ignore_ascii_case("monospace") {
        egui::FontFamily::Monospace
    } else if token.font_family.trim().eq_ignore_ascii_case("sans") {
        egui::FontFamily::Proportional
    } else {
        egui::FontFamily::Name(token.font_family.clone().into())
    };
    egui::FontId::new(token.size.max(1.0), family)
}
