use eframe::egui;

#[derive(Clone, Debug)]
pub struct DialogueOverlayLayout {
    pub panel: egui::Rect,
    pub scale: f32,
    pub corner_radius: f32,
    pub content_padding: egui::Vec2,
    pub clip_padding: f32,
    pub speaker_height: f32,
    pub speaker_text_gap: f32,
    pub speaker_font_size: f32,
    pub text_font_size: f32,
    pub control_height: f32,
}

#[derive(Clone, Debug)]
pub struct ChoiceOverlayLayout {
    pub panel: egui::Rect,
    pub prompt_height: f32,
    pub options_viewport_height: f32,
    pub option_heights: Vec<f32>,
    pub scale: f32,
    pub content_padding: egui::Vec2,
    pub clip_padding: f32,
    pub prompt_option_gap: f32,
    pub option_gap: f32,
    pub prompt_font_size: f32,
    pub option_font_size: f32,
}

pub fn dialogue_overlay_rect(stage_rect: egui::Rect) -> egui::Rect {
    dialogue_overlay_layout(stage_rect).panel
}

pub fn dialogue_overlay_layout(stage_rect: egui::Rect) -> DialogueOverlayLayout {
    let shortest_side = stage_rect.width().min(stage_rect.height()).max(1.0);
    let scale = overlay_chrome_scale(shortest_side);
    let margin = (shortest_side * 0.06).clamp(4.0 * scale, 24.0 * scale);
    let available_width = (stage_rect.width() - margin * 2.0).max(1.0);
    let available_height = (stage_rect.height() - margin * 2.0).max(1.0);
    let desired_height = (stage_rect.height() * 0.36).clamp(112.0 * scale, 200.0 * scale);
    let box_height = desired_height.min(available_height);
    let panel = egui::Rect::from_min_size(
        egui::pos2(
            stage_rect.left() + margin,
            stage_rect.bottom() - box_height - margin,
        ),
        egui::vec2(available_width, box_height),
    );
    DialogueOverlayLayout {
        panel,
        scale,
        corner_radius: 6.0 * scale,
        content_padding: egui::vec2(16.0 * scale, 12.0 * scale),
        clip_padding: 8.0 * scale,
        speaker_height: (22.0 * scale).max(12.0),
        speaker_text_gap: 6.0 * scale,
        speaker_font_size: 15.0 * scale,
        text_font_size: 16.0 * scale,
        control_height: (24.0 * scale).max(16.0),
    }
}

pub fn scene_overlay_rect(stage_rect: egui::Rect) -> egui::Rect {
    let shortest_side = stage_rect.width().min(stage_rect.height()).max(1.0);
    let margin = (shortest_side * 0.04).clamp(4.0, 24.0);
    let max_width = (stage_rect.width() - margin * 2.0).max(48.0);
    let max_height = (stage_rect.height() - margin * 2.0).max(32.0);
    let width = (stage_rect.width() * 0.44)
        .clamp(160.0_f32.min(max_width), 520.0_f32.min(max_width))
        .min(max_width);
    let height = 88.0_f32.min(max_height).max(32.0_f32.min(max_height));
    egui::Rect::from_min_size(
        egui::pos2(
            stage_rect.right() - width - margin,
            stage_rect.bottom() - height - margin,
        ),
        egui::vec2(width, height),
    )
}

pub fn choice_overlay_layout(
    stage_rect: egui::Rect,
    prompt: &str,
    options: &[String],
) -> ChoiceOverlayLayout {
    let shortest_side = stage_rect.width().min(stage_rect.height()).max(1.0);
    let scale = overlay_chrome_scale(shortest_side);
    let margin = (shortest_side * 0.06).clamp(4.0 * scale, 24.0 * scale);
    let max_width = (stage_rect.width() - margin * 2.0).max(80.0);
    let min_width = (280.0 * scale).min(max_width);
    let panel_width = (stage_rect.width() * 0.62)
        .clamp(min_width, (720.0 * scale).min(max_width))
        .min(max_width);
    let content_padding = egui::vec2(18.0 * scale, 14.0 * scale);
    let clip_padding = 8.0 * scale;
    let prompt_option_gap = 10.0 * scale;
    let option_gap = 8.0 * scale;
    let prompt_font_size = 17.0 * scale;
    let option_font_size = 15.0 * scale;
    let text_width = (panel_width - content_padding.x * 2.0).max(56.0 * scale);
    let max_height = (stage_rect.height() - margin * 2.0).max(48.0);
    let prompt_min_height = (20.0 * scale).max(12.0);
    let option_min_height = (28.0 * scale).max(16.0);
    let prompt_max_height = (max_height * 0.32).clamp(prompt_min_height, 86.0 * scale);
    let option_max_height = (max_height * 0.48).clamp(option_min_height, 94.0 * scale);
    let prompt_height = estimate_wrapped_height(
        prompt,
        text_width,
        prompt_font_size,
        prompt_min_height,
        prompt_max_height,
    );
    let option_heights = options
        .iter()
        .map(|option| {
            estimate_wrapped_height(
                option,
                text_width - 24.0 * scale,
                option_font_size,
                option_min_height,
                option_max_height,
            )
        })
        .collect::<Vec<_>>();
    let options_total = option_heights.iter().sum::<f32>()
        + option_gap * option_heights.len().saturating_sub(1) as f32;
    let chrome_height = content_padding.y + prompt_height + prompt_option_gap + content_padding.y;
    let desired_height = chrome_height + options_total;
    let min_height = (120.0 * scale).min(max_height);
    let panel_height = desired_height.clamp(min_height, max_height);
    let options_viewport_height = (panel_height - chrome_height).max(0.0);
    let panel =
        egui::Rect::from_center_size(stage_rect.center(), egui::vec2(panel_width, panel_height));
    ChoiceOverlayLayout {
        panel,
        prompt_height,
        options_viewport_height,
        option_heights,
        scale,
        content_padding,
        clip_padding,
        prompt_option_gap,
        option_gap,
        prompt_font_size,
        option_font_size,
    }
}

fn overlay_chrome_scale(shortest_side: f32) -> f32 {
    (shortest_side / 360.0).clamp(0.45, 1.0)
}

fn estimate_wrapped_height(text: &str, width: f32, font_size: f32, min: f32, max: f32) -> f32 {
    let average_char_width = (font_size * 0.52).max(4.0);
    let chars_per_line = (width / average_char_width).floor().max(10.0) as usize;
    let char_count = text.chars().count().max(1);
    let lines = char_count.div_ceil(chars_per_line).clamp(1, 4) as f32;
    (lines * (font_size + 6.0) + 12.0).clamp(min, max)
}

pub fn soft_wrap_long_tokens(text: &str, max_run: usize) -> String {
    if max_run == 0 {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut run = 0usize;
    for ch in text.chars() {
        if ch.is_whitespace() {
            run = 0;
            out.push(ch);
            continue;
        }
        if run >= max_run {
            out.push('\u{200b}');
            run = 0;
        }
        out.push(ch);
        run += 1;
    }
    out
}
