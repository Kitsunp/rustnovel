use std::time::Duration;

use eframe::egui;
use tracing::info;
use visual_novel_engine::{AudioCommand, ChoiceOptionCompiled, Engine};

use super::super::super::node_types::ToastState;
use super::super::state::PlayerSessionState;

pub(super) fn transition_kind_label(kind: u8) -> &'static str {
    match kind {
        0 => "fade",
        1 => "dissolve",
        2 => "cut",
        _ => "unknown",
    }
}

pub(super) fn render_transition(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    engine: &mut Engine,
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
        if let Ok((cmd, _)) = engine.step() {
            audio_commands.extend(cmd);
        }
        ctx.data_mut(|data| data.remove::<(u32, f64)>(transition_id));
    } else {
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

pub(super) fn render_dialogue(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    player: &mut PlayerSessionState,
    speaker: &str,
    text: &str,
    now_sec: f64,
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

    egui::Frame::none()
        .fill(egui::Color32::from_rgb(40, 40, 50))
        .rounding(8.0)
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(rendered_text).size(16.0));
        });

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
                if text_complete {
                    should_advance = true;
                } else {
                    player.reveal_current_line(text, now_sec);
                }
            }
        });
    });

    if !text_complete {
        ctx.request_repaint_after(Duration::from_millis(16));
    } else if player.autoplay_ready(now_sec) {
        player.mark_auto_step(now_sec);
        should_advance = true;
    }

    should_advance
}

pub(super) fn render_choice(
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
            let _ = engine.choose(i);
            audio_commands.extend(engine.take_audio_commands());
            *toast = Some(ToastState::success(format!(
                "Selected: {}",
                option.text.as_ref()
            )));
        }
        ui.add_space(5.0);
    }
}

pub(super) fn render_scene(
    ui: &mut egui::Ui,
    player: &mut PlayerSessionState,
    now_sec: f64,
) -> bool {
    if ui.button("Continue").clicked() {
        return true;
    }
    if player.autoplay_ready(now_sec) {
        player.mark_auto_step(now_sec);
        return true;
    }
    false
}

pub(super) fn render_end(
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
