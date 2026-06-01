use std::time::Duration;

use eframe::egui;
use tracing::info;
use visual_novel_engine::runtime::{AudioCommand, ChoiceOptionCompiled, Engine};

use super::super::super::node_types::ToastState;
use super::super::state::PlayerSessionState;

pub struct DialogueOverlayContext<'a> {
    pub speaker: &'a str,
    pub text: &'a str,
    pub now_sec: f64,
    pub advance_on_text_panel_click: bool,
}

pub struct ChoiceOverlayContent<'a> {
    pub prompt: &'a str,
    pub localized_options: &'a [String],
    pub options: &'a [ChoiceOptionCompiled],
}

pub fn transition_kind_label(kind: u8) -> &'static str {
    match kind {
        0 => "fade",
        1 => "dissolve",
        2 => "cut",
        _ => "unknown",
    }
}

pub fn render_transition(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    kind: u8,
    duration_ms: u32,
    audio_commands: &mut Vec<AudioCommand>,
) {
    let ip = engine.state().position;
    let now = ctx.input(|i| i.time);
    let transition_id = egui::Id::new("player_transition_state");

    let mut state = ctx.data_mut(|data| data.get_temp::<(u32, f64)>(transition_id));
    if !matches!(state, Some((prev_ip, _)) if prev_ip == ip) {
        ctx.data_mut(|data| data.insert_temp(transition_id, (ip, now)));
        state = Some((ip, now));
    }

    let start_time = state.map(|(_, t)| t).unwrap_or(now);
    let duration_secs = (duration_ms.max(1) as f64) / 1000.0;
    let elapsed = (now - start_time).max(0.0);
    let progress = (elapsed / duration_secs).clamp(0.0, 1.0) as f32;

    ui.label(format!(
        "Transition {} ({} ms)",
        transition_kind_label(kind),
        duration_ms
    ));
    ui.add(
        egui::ProgressBar::new(progress)
            .desired_width(280.0)
            .show_percentage(),
    );

    if progress >= 1.0 || ui.button("Skip Transition").clicked() {
        match engine.step() {
            Ok((cmd, _)) => audio_commands.extend(cmd),
            Err(err) => {
                *toast = Some(ToastState::error(format!(
                    "Transition advance failed at ip {}: {err}",
                    engine.state().position
                )));
            }
        }
        ctx.data_mut(|data| data.remove::<(u32, f64)>(transition_id));
    } else {
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

pub fn render_dialogue(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    player: &mut PlayerSessionState,
    speaker: &str,
    text: &str,
    now_sec: f64,
    advance_on_text_panel_click: bool,
) -> bool {
    let rendered_text = player.visible_text(text, now_sec);
    let text_complete = player.is_text_fully_revealed(text, now_sec);

    egui::Frame::none()
        .fill(egui::Color32::from_rgb(60, 60, 80))
        .rounding(8.0)
        .inner_margin(egui::Margin::symmetric(16.0, 12.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(speaker).size(18.0).strong());
        });

    ui.add_space(10.0);

    let text_panel_response = egui::Frame::none()
        .fill(egui::Color32::from_rgb(40, 40, 50))
        .rounding(8.0)
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(rendered_text).size(16.0));
        })
        .response;

    ui.add_space(20.0);
    let mut should_advance = false;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if text_complete {
                "Continue"
            } else {
                "Show full"
            };
            if ui.button(label).clicked() {
                reveal_or_advance_dialogue(
                    player,
                    text,
                    now_sec,
                    text_complete,
                    &mut should_advance,
                );
            }
        });
    });

    if advance_on_text_panel_click
        && ui
            .interact(
                text_panel_response.rect,
                egui::Id::new("editor_player_dialogue_text_panel"),
                egui::Sense::click(),
            )
            .clicked()
    {
        reveal_or_advance_dialogue(player, text, now_sec, text_complete, &mut should_advance);
    }

    if !text_complete {
        ctx.request_repaint_after(Duration::from_millis(16));
    } else if player.autoplay_ready(now_sec) {
        player.mark_auto_step(now_sec);
        should_advance = true;
    }

    should_advance
}

fn reveal_or_advance_dialogue(
    player: &mut PlayerSessionState,
    text: &str,
    now_sec: f64,
    text_complete: bool,
    should_advance: &mut bool,
) {
    if text_complete {
        *should_advance = true;
    } else {
        player.reveal_current_line(text, now_sec);
    }
}

pub fn render_dialogue_overlay(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    player: &mut PlayerSessionState,
    geometry: crate::editor::scene_stage::StageGeometry,
    dialogue: DialogueOverlayContext<'_>,
) -> bool {
    let DialogueOverlayContext {
        speaker,
        text,
        now_sec,
        advance_on_text_panel_click,
    } = dialogue;
    let rendered_text = player.visible_text(text, now_sec);
    let text_complete = player.is_text_fully_revealed(text, now_sec);
    let layout = crate::player_overlay::dialogue_overlay_layout(geometry.stage_rect);
    let rect = layout.panel;

    ui.painter().rect_filled(
        rect,
        layout.corner_radius,
        egui::Color32::from_rgba_premultiplied(8, 8, 14, 220),
    );
    ui.painter().rect_stroke(
        rect,
        layout.corner_radius,
        egui::Stroke::new(1.0, egui::Color32::from_gray(130)),
    );
    let mut should_advance = false;
    ui.allocate_ui_at_rect(rect.shrink2(layout.content_padding), |ui| {
        ui.set_clip_rect(rect.shrink(layout.clip_padding));
        ui.add_sized(
            [ui.available_width(), layout.speaker_height],
            egui::Label::new(
                egui::RichText::new(speaker)
                    .color(egui::Color32::from_rgb(180, 210, 255))
                    .size(layout.speaker_font_size),
            )
            .wrap(true),
        );
        ui.add_space(layout.speaker_text_gap);
        let text_height = (rect.height()
            - layout.content_padding.y * 2.0
            - layout.speaker_height
            - layout.speaker_text_gap
            - layout.control_height)
            .max(12.0 * layout.scale);
        ui.add_sized(
            [ui.available_width(), text_height],
            egui::Label::new(
                egui::RichText::new(rendered_text)
                    .color(egui::Color32::WHITE)
                    .size(layout.text_font_size),
            )
            .wrap(true),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if text_complete {
                "Continue"
            } else {
                "Show full"
            };
            if ui
                .add_sized(
                    [96.0 * layout.scale, layout.control_height],
                    egui::Button::new(egui::RichText::new(label).size(layout.text_font_size)),
                )
                .clicked()
            {
                reveal_or_advance_dialogue(
                    player,
                    text,
                    now_sec,
                    text_complete,
                    &mut should_advance,
                );
            }
        });
    });

    if advance_on_text_panel_click
        && ui
            .interact(
                rect,
                egui::Id::new("player_dialogue_overlay"),
                egui::Sense::click(),
            )
            .clicked()
    {
        reveal_or_advance_dialogue(player, text, now_sec, text_complete, &mut should_advance);
    }

    if !text_complete {
        ctx.request_repaint_after(Duration::from_millis(16));
    } else if player.autoplay_ready(now_sec) {
        player.mark_auto_step(now_sec);
        should_advance = true;
    }

    should_advance
}

pub fn render_choice(
    ui: &mut egui::Ui,
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    prompt: &str,
    localized_options: &[String],
    options: &[ChoiceOptionCompiled],
    audio_commands: &mut Vec<AudioCommand>,
) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(80, 60, 60))
        .rounding(8.0)
        .inner_margin(egui::Margin::symmetric(16.0, 12.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(prompt).size(18.0).strong());
        });

    ui.add_space(15.0);
    for (i, option) in options.iter().enumerate() {
        let label = localized_options
            .get(i)
            .map(String::as_str)
            .unwrap_or(option.text.as_ref());
        if ui
            .add(egui::Button::new(label).min_size(egui::vec2(200.0, 40.0)))
            .clicked()
        {
            info!("Choice selected: {} ({})", option.text.as_ref(), i);
            match engine.choose(i) {
                Ok(_) => {
                    audio_commands.extend(engine.take_audio_commands());
                    *toast = Some(ToastState::success(format!(
                        "Selected: {}",
                        option.text.as_ref()
                    )));
                }
                Err(err) => {
                    *toast = Some(ToastState::error(format!(
                        "Choice failed at ip {}: {err}",
                        engine.state().position
                    )));
                }
            }
        }
        ui.add_space(5.0);
    }
}

pub fn render_choice_overlay(
    ui: &mut egui::Ui,
    geometry: crate::editor::scene_stage::StageGeometry,
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    content: ChoiceOverlayContent<'_>,
    audio_commands: &mut Vec<AudioCommand>,
) {
    let ChoiceOverlayContent {
        prompt,
        localized_options,
        options,
    } = content;
    let option_labels = options
        .iter()
        .enumerate()
        .map(|(idx, option)| {
            localized_options
                .get(idx)
                .cloned()
                .unwrap_or_else(|| option.text.as_ref().to_string())
        })
        .collect::<Vec<_>>();
    let layout =
        crate::player_overlay::choice_overlay_layout(geometry.stage_rect, prompt, &option_labels);
    let corner_radius = 6.0 * layout.scale;
    ui.painter().rect_filled(
        layout.panel,
        corner_radius,
        egui::Color32::from_rgba_premultiplied(10, 12, 18, 230),
    );
    ui.painter().rect_stroke(
        layout.panel,
        corner_radius,
        egui::Stroke::new(1.0, egui::Color32::from_gray(120)),
    );

    let mut selected = None;
    ui.allocate_ui_at_rect(layout.panel.shrink2(layout.content_padding), |ui| {
        ui.set_clip_rect(layout.panel.shrink(layout.clip_padding));
        ui.add_sized(
            [ui.available_width(), layout.prompt_height],
            egui::Label::new(
                egui::RichText::new(crate::player_overlay::soft_wrap_long_tokens(prompt, 28))
                    .color(egui::Color32::WHITE)
                    .size(layout.prompt_font_size),
            )
            .wrap(true),
        );
        ui.add_space(layout.prompt_option_gap);
        egui::ScrollArea::vertical()
            .id_source("player_choice_overlay_scroll")
            .max_height(layout.options_viewport_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (idx, option) in option_labels.iter().enumerate() {
                    let row_height = layout.option_heights.get(idx).copied().unwrap_or(38.0);
                    if ui
                        .add_sized(
                            [ui.available_width(), row_height],
                            egui::Button::new(
                                egui::RichText::new(crate::player_overlay::soft_wrap_long_tokens(
                                    option, 32,
                                ))
                                .size(layout.option_font_size),
                            ),
                        )
                        .clicked()
                    {
                        selected = Some((idx, option.clone()));
                    }
                    ui.add_space(layout.option_gap);
                }
            });
    });
    if let Some((idx, option)) = selected {
        info!("Choice selected: {} ({})", option, idx);
        match engine.choose(idx) {
            Ok(_) => {
                audio_commands.extend(engine.take_audio_commands());
                *toast = Some(ToastState::success(format!("Selected: {option}")));
            }
            Err(err) => {
                *toast = Some(ToastState::error(format!(
                    "Choice failed at ip {}: {err}",
                    engine.state().position
                )));
            }
        }
    }
}

pub fn render_scene(ui: &mut egui::Ui, player: &mut PlayerSessionState, now_sec: f64) -> bool {
    if ui.button("Continue").clicked() {
        return true;
    }
    if player.autoplay_ready(now_sec) {
        player.mark_auto_step(now_sec);
        return true;
    }
    false
}

pub fn render_scene_overlay(
    ui: &mut egui::Ui,
    player: &mut PlayerSessionState,
    geometry: crate::editor::scene_stage::StageGeometry,
    description: &str,
    now_sec: f64,
    advance_on_text_panel_click: bool,
) -> bool {
    let rect = crate::player_overlay::scene_overlay_rect(geometry.stage_rect);
    ui.painter().rect_filled(
        rect,
        6.0,
        egui::Color32::from_rgba_premultiplied(8, 8, 14, 215),
    );
    ui.painter().rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(120)),
    );

    let mut should_advance = false;
    ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(14.0, 10.0)), |ui| {
        ui.set_clip_rect(rect.shrink(8.0));
        ui.add_sized(
            [ui.available_width(), 34.0],
            egui::Label::new(
                egui::RichText::new(crate::player_overlay::soft_wrap_long_tokens(
                    description,
                    42,
                ))
                .color(egui::Color32::WHITE),
            )
            .wrap(true),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Continue").clicked() {
                should_advance = true;
            }
        });
    });
    if advance_on_text_panel_click
        && ui
            .interact(
                rect,
                egui::Id::new("editor_player_scene_overlay"),
                egui::Sense::click(),
            )
            .clicked()
    {
        should_advance = true;
    }

    if player.autoplay_ready(now_sec) {
        player.mark_auto_step(now_sec);
        should_advance = true;
    }
    should_advance
}

pub fn render_end(
    ui: &mut egui::Ui,
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    player: &mut PlayerSessionState,
    now_sec: f64,
    audio_commands: &mut Vec<AudioCommand>,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(50.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(60, 40, 60))
            .rounding(8.0)
            .inner_margin(egui::Margin::same(24.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("The End").size(32.0).strong());
            });

        ui.add_space(30.0);
        if ui.button("Play Again").clicked() {
            info!("Restarting from end");
            if engine.jump_to_label("start").is_ok() {
                engine.clear_session_history();
                player.reset_for_restart(now_sec);
                audio_commands.extend(engine.take_audio_commands());
                *toast = Some(ToastState::success("Story restarted"));
            }
        }
    });
}
