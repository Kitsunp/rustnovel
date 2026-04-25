use eframe::egui;
use visual_novel_engine::{AudioCommand, ChoiceHistoryEntry, Engine, LocalizationCatalog};

use super::super::super::node_types::ToastState;
use super::super::state::{PlayerSessionState, SkipMode};

pub(super) fn render_header_bar(
    ui: &mut egui::Ui,
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    player: &mut PlayerSessionState,
    now_sec: f64,
    audio_commands: &mut Vec<AudioCommand>,
) {
    ui.horizontal(|ui| {
        ui.heading("Player Mode");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Restart").clicked() {
                tracing::info!("Restarting story");
                if engine.jump_to_label("start").is_ok() {
                    engine.clear_session_history();
                    player.reset_for_restart(now_sec);
                    audio_commands.extend(engine.take_audio_commands());
                    *toast = Some(ToastState::success("Story restarted"));
                }
            }
        });
    });
}

pub(super) fn render_player_controls(
    ui: &mut egui::Ui,
    player: &mut PlayerSessionState,
    player_locale: &mut String,
    localization_catalog: &LocalizationCatalog,
) {
    ui.horizontal_wrapped(|ui| {
        if !localization_catalog.locale_codes().is_empty() {
            egui::ComboBox::from_id_source("player_locale_selector")
                .selected_text(format!("Locale: {}", player_locale))
                .show_ui(ui, |ui| {
                    for locale in localization_catalog.locale_codes() {
                        ui.selectable_value(player_locale, locale.clone(), locale);
                    }
                });
        }

        ui.checkbox(&mut player.autoplay_enabled, "Auto");
        ui.add(egui::Slider::new(&mut player.autoplay_delay_ms, 200..=5000).text("Auto delay ms"));
        ui.add(
            egui::Slider::new(&mut player.text_chars_per_second, 10.0..=240.0).text("Text chars/s"),
        );

        egui::ComboBox::from_id_source("player_skip_mode")
            .selected_text(match player.skip_mode {
                SkipMode::Off => "Skip: Off",
                SkipMode::ReadOnly => "Skip: Read",
                SkipMode::All => "Skip: All",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut player.skip_mode, SkipMode::Off, "Skip: Off");
                ui.selectable_value(&mut player.skip_mode, SkipMode::ReadOnly, "Skip: Read");
                ui.selectable_value(&mut player.skip_mode, SkipMode::All, "Skip: All");
            });

        ui.separator();
        ui.checkbox(&mut player.show_backlog, "Backlog");
        ui.checkbox(&mut player.show_choice_history, "Choice history");
    });

    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label("Audio mix (preview):");
        ui.checkbox(&mut player.bgm_muted, "Mute BGM");
        ui.add(egui::Slider::new(&mut player.bgm_volume, 0.0..=1.0).text("BGM"));
        ui.checkbox(&mut player.sfx_muted, "Mute SFX");
        ui.add(egui::Slider::new(&mut player.sfx_volume, 0.0..=1.0).text("SFX"));
        ui.checkbox(&mut player.voice_muted, "Mute Voice");
        ui.add(egui::Slider::new(&mut player.voice_volume, 0.0..=1.0).text("Voice"));
    });

    if let Some(last_event) = &player.last_audio_event {
        ui.label(format!("Audio trace: {last_event}"));
    }
    if let Some(last_error) = &player.last_audio_error {
        ui.colored_label(
            egui::Color32::YELLOW,
            format!("Audio warning: {last_error}"),
        );
    }
}

pub(super) fn render_backlog_window(
    ctx: &egui::Context,
    engine: &Engine,
    player: &mut PlayerSessionState,
) {
    if !player.show_backlog {
        return;
    }
    let mut open = player.show_backlog;
    egui::Window::new("Backlog")
        .open(&mut open)
        .default_width(420.0)
        .show(ctx, |ui| {
            if engine.state().history.is_empty() {
                ui.label("No dialogue history yet.");
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for line in engine.state().history.iter().rev() {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(line.speaker.as_ref()).strong());
                        ui.label(line.text.as_ref());
                    });
                    ui.add_space(6.0);
                }
            });
        });
    player.show_backlog = open;
}

pub(super) fn render_choice_history_window(
    ctx: &egui::Context,
    engine: &Engine,
    player: &mut PlayerSessionState,
) {
    if !player.show_choice_history {
        return;
    }
    let mut open = player.show_choice_history;
    egui::Window::new("Choice History")
        .open(&mut open)
        .default_width(420.0)
        .show(ctx, |ui| {
            if engine.choice_history().is_empty() {
                ui.label("No choices selected yet.");
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for entry in engine.choice_history().iter().rev() {
                    render_choice_history_entry(ui, entry);
                    ui.add_space(6.0);
                }
            });
        });
    player.show_choice_history = open;
}

fn render_choice_history_entry(ui: &mut egui::Ui, entry: &ChoiceHistoryEntry) {
    ui.group(|ui| {
        ui.label(format!(
            "ip {} -> option {}",
            entry.event_ip,
            entry.option_index + 1
        ));
        ui.label(format!("\"{}\"", entry.option_text));
        ui.label(format!("target ip {}", entry.target_ip));
    });
}
