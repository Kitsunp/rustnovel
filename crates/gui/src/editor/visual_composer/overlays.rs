use eframe::egui;
use std::collections::HashMap;
use visual_novel_engine::runtime::{Engine, EventCompiled};

use super::LayerOverride;
use super::VisualComposerAction;
use crate::editor::{ComposerPreviewMode, StoryNode};

#[derive(Clone, Debug)]
pub struct ChoiceOverlayLayout {
    pub panel: egui::Rect,
    pub prompt_height: f32,
    pub options_viewport_height: f32,
    pub option_heights: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverlaySource {
    Dialogue {
        speaker: String,
        text: String,
    },
    Choice {
        prompt: String,
        options: Vec<String>,
    },
    Transition {
        kind: u8,
    },
}

pub fn render_runtime_controls(
    ui: &mut egui::Ui,
    engine: &Option<Engine>,
    action: &mut Option<VisualComposerAction>,
) {
    let Some(engine) = engine else {
        return;
    };
    match engine.current_event() {
        Ok(EventCompiled::Choice(choice)) => {
            for (idx, option) in choice.options.iter().enumerate() {
                if ui
                    .small_button(format!("Pick {}", idx + 1))
                    .on_hover_text(option.text.as_ref())
                    .clicked()
                {
                    *action = Some(VisualComposerAction::TestChoose(idx));
                }
            }
        }
        Ok(_) => {
            if ui.small_button("Next").clicked() {
                *action = Some(VisualComposerAction::TestAdvance);
            }
        }
        Err(_) => {}
    }
}

pub fn render_runtime_overlay(
    ui: &mut egui::Ui,
    geometry: crate::editor::scene_stage::StageGeometry,
    engine: &Option<Engine>,
    selected_authoring_node: Option<&StoryNode>,
    preview_mode: ComposerPreviewMode,
    layer_overrides: &HashMap<String, LayerOverride>,
    action: &mut Option<VisualComposerAction>,
) {
    let Some(source) = selected_overlay_source(
        engine.as_ref(),
        selected_authoring_node,
        preview_mode,
        layer_overrides,
    ) else {
        return;
    };
    match source {
        OverlaySource::Dialogue { speaker, text } => {
            render_dialogue_overlay(ui, geometry, speaker.as_str(), text.as_str(), action);
        }
        OverlaySource::Choice { prompt, options } => {
            render_choice_overlay(ui, geometry, prompt.as_str(), &options, action);
        }
        OverlaySource::Transition { kind } => {
            let alpha = if kind == 1 { 96 } else { 150 };
            ui.painter().rect_filled(
                geometry.stage_rect,
                0.0,
                egui::Color32::from_rgba_premultiplied(0, 0, 0, alpha),
            );
        }
    }
}

pub fn selected_overlay_source(
    engine: Option<&Engine>,
    selected_authoring_node: Option<&StoryNode>,
    preview_mode: ComposerPreviewMode,
    layer_overrides: &HashMap<String, LayerOverride>,
) -> Option<OverlaySource> {
    let authoring = || authoring_overlay_source(selected_authoring_node, layer_overrides);
    let runtime = || {
        let event = engine.and_then(|engine| engine.current_event().ok())?;
        runtime_overlay_source(&event, layer_overrides)
    };
    if preview_mode.uses_runtime_state() {
        runtime().or_else(authoring)
    } else {
        authoring().or_else(runtime)
    }
}

pub fn runtime_overlay_visible(
    event: &EventCompiled,
    layer_overrides: &HashMap<String, LayerOverride>,
) -> bool {
    runtime_overlay_object_id(event)
        .and_then(|object_id| layer_overrides.get(object_id))
        .is_none_or(|override_state| override_state.visible)
}

pub fn runtime_overlay_object_id(event: &EventCompiled) -> Option<&'static str> {
    match event {
        EventCompiled::Dialogue(_) => Some("overlay:dialogue"),
        EventCompiled::Choice(_) => Some("overlay:choice"),
        EventCompiled::Transition(_) => Some("overlay:transition"),
        _ => None,
    }
}

fn authoring_overlay_source(
    selected_authoring_node: Option<&StoryNode>,
    layer_overrides: &HashMap<String, LayerOverride>,
) -> Option<OverlaySource> {
    match selected_authoring_node? {
        StoryNode::Dialogue { speaker, text }
            if overlay_object_visible("overlay:dialogue", layer_overrides) =>
        {
            Some(OverlaySource::Dialogue {
                speaker: speaker.clone(),
                text: text.clone(),
            })
        }
        StoryNode::Choice { prompt, options }
            if overlay_object_visible("overlay:choice", layer_overrides) =>
        {
            Some(OverlaySource::Choice {
                prompt: prompt.clone(),
                options: options.clone(),
            })
        }
        _ => None,
    }
}

fn runtime_overlay_source(
    event: &EventCompiled,
    layer_overrides: &HashMap<String, LayerOverride>,
) -> Option<OverlaySource> {
    if !runtime_overlay_visible(event, layer_overrides) {
        return None;
    }
    match event {
        EventCompiled::Dialogue(dialogue) => Some(OverlaySource::Dialogue {
            speaker: dialogue.speaker.as_ref().to_string(),
            text: dialogue.text.as_ref().to_string(),
        }),
        EventCompiled::Choice(choice) => Some(OverlaySource::Choice {
            prompt: choice.prompt.as_ref().to_string(),
            options: choice
                .options
                .iter()
                .map(|option| option.text.as_ref().to_string())
                .collect(),
        }),
        EventCompiled::Transition(transition) => Some(OverlaySource::Transition {
            kind: transition.kind,
        }),
        _ => None,
    }
}

fn overlay_object_visible(
    object_id: &str,
    layer_overrides: &HashMap<String, LayerOverride>,
) -> bool {
    layer_overrides
        .get(object_id)
        .is_none_or(|override_state| override_state.visible)
}

fn render_dialogue_overlay(
    ui: &mut egui::Ui,
    geometry: crate::editor::scene_stage::StageGeometry,
    speaker: &str,
    text: &str,
    action: &mut Option<VisualComposerAction>,
) {
    let box_height = (geometry.stage_rect.height() * 0.26).clamp(90.0, 180.0);
    let rect = egui::Rect::from_min_size(
        egui::pos2(
            geometry.stage_rect.left() + 24.0,
            geometry.stage_rect.bottom() - box_height - 24.0,
        ),
        egui::vec2(geometry.stage_rect.width() - 48.0, box_height),
    );
    ui.painter().rect_filled(
        rect,
        6.0,
        egui::Color32::from_rgba_premultiplied(8, 8, 14, 220),
    );
    ui.painter().rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(130)),
    );
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(16.0, 12.0)), |ui| {
        ui.set_clip_rect(rect.shrink(8.0));
        ui.add_sized(
            [ui.available_width(), 22.0],
            egui::Label::new(
                egui::RichText::new(speaker).color(egui::Color32::from_rgb(180, 210, 255)),
            )
            .wrap(true),
        );
        ui.add_space(6.0);
        ui.add_sized(
            [ui.available_width(), (box_height - 54.0).max(24.0)],
            egui::Label::new(egui::RichText::new(text).color(egui::Color32::WHITE)).wrap(true),
        );
    });
    if ui
        .interact(
            rect,
            egui::Id::new("composer_dialogue_overlay"),
            egui::Sense::click(),
        )
        .clicked()
    {
        *action = Some(VisualComposerAction::TestAdvance);
    }
}

fn render_choice_overlay(
    ui: &mut egui::Ui,
    geometry: crate::editor::scene_stage::StageGeometry,
    prompt: &str,
    options: &[String],
    action: &mut Option<VisualComposerAction>,
) {
    let layout = choice_overlay_layout(geometry.stage_rect, prompt, options);
    ui.painter().rect_filled(
        layout.panel,
        6.0,
        egui::Color32::from_rgba_premultiplied(10, 12, 18, 230),
    );
    ui.painter().rect_stroke(
        layout.panel,
        6.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(120)),
    );

    ui.allocate_ui_at_rect(layout.panel.shrink2(egui::vec2(18.0, 14.0)), |ui| {
        ui.set_clip_rect(layout.panel.shrink(8.0));
        ui.add_sized(
            [ui.available_width(), layout.prompt_height],
            egui::Label::new(
                egui::RichText::new(soft_wrap_long_tokens(prompt, 28)).color(egui::Color32::WHITE),
            )
            .wrap(true),
        );
        ui.add_space(10.0);
        egui::ScrollArea::vertical()
            .id_source("composer_choice_overlay_scroll")
            .max_height(layout.options_viewport_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (idx, option) in options.iter().enumerate() {
                    let row_height = layout.option_heights.get(idx).copied().unwrap_or(38.0);
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), row_height),
                        egui::Sense::click(),
                    );
                    let fill = if response.hovered() {
                        egui::Color32::from_rgb(52, 88, 116)
                    } else {
                        egui::Color32::from_rgb(36, 54, 72)
                    };
                    ui.painter().rect_filled(rect, 4.0, fill);
                    ui.put(
                        rect.shrink2(egui::vec2(12.0, 6.0)),
                        egui::Label::new(
                            egui::RichText::new(soft_wrap_long_tokens(option, 32))
                                .color(egui::Color32::WHITE),
                        )
                        .wrap(true),
                    );
                    if response.clicked() {
                        *action = Some(VisualComposerAction::TestChoose(idx));
                    }
                    ui.add_space(8.0);
                }
            });
    });
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
