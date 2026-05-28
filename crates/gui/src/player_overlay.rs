use eframe::egui;

#[derive(Clone, Debug)]
pub struct ChoiceOverlayLayout {
    pub panel: egui::Rect,
    pub prompt_height: f32,
    pub options_viewport_height: f32,
    pub option_heights: Vec<f32>,
}

pub fn dialogue_overlay_rect(stage_rect: egui::Rect) -> egui::Rect {
    let shortest_side = stage_rect.width().min(stage_rect.height()).max(1.0);
    let margin = (shortest_side * 0.06).clamp(4.0, 24.0);
    let available_width = (stage_rect.width() - margin * 2.0).max(1.0);
    let available_height = (stage_rect.height() - margin * 2.0).max(1.0);
    let desired_height = (stage_rect.height() * 0.36).clamp(112.0, 200.0);
    let box_height = desired_height.min(available_height);
    egui::Rect::from_min_size(
        egui::pos2(
            stage_rect.left() + margin,
            stage_rect.bottom() - box_height - margin,
        ),
        egui::vec2(available_width, box_height),
    )
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
    let margin = (shortest_side * 0.06).clamp(4.0, 24.0);
    let max_width = (stage_rect.width() - margin * 2.0).max(80.0);
    let min_width = 280.0_f32.min(max_width);
    let panel_width = (stage_rect.width() * 0.62)
        .clamp(min_width, 720.0_f32.min(max_width))
        .min(max_width);
    let text_width = (panel_width - 36.0).max(56.0);
    let max_height = (stage_rect.height() - margin * 2.0).max(48.0);
    let prompt_max_height = (max_height * 0.32).clamp(20.0, 86.0);
    let option_max_height = (max_height * 0.48).clamp(28.0, 94.0);
    let prompt_height = estimate_wrapped_height(prompt, text_width, 17.0, 20.0, prompt_max_height);
    let option_heights = options
        .iter()
        .map(|option| {
            estimate_wrapped_height(option, text_width - 24.0, 15.0, 28.0, option_max_height)
        })
        .collect::<Vec<_>>();
    let option_gap = 8.0;
    let options_total = option_heights.iter().sum::<f32>()
        + option_gap * option_heights.len().saturating_sub(1) as f32;
    let chrome_height = 14.0 + prompt_height + 10.0 + 14.0;
    let desired_height = chrome_height + options_total;
    let min_height = 120.0_f32.min(max_height);
    let panel_height = desired_height.clamp(min_height, max_height);
    let options_viewport_height = (panel_height - chrome_height).max(0.0);
    let panel =
        egui::Rect::from_center_size(stage_rect.center(), egui::vec2(panel_width, panel_height));
    ChoiceOverlayLayout {
        panel,
        prompt_height,
        options_viewport_height,
        option_heights,
    }
}

fn estimate_wrapped_height(text: &str, width: f32, font_size: f32, min: f32, max: f32) -> f32 {
    let average_char_width = (font_size * 0.52).max(6.0);
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
