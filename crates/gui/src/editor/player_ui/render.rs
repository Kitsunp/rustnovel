use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use eframe::egui;
use tracing::instrument;
use visual_novel_engine::{
    localization_key, AudioCommand, Engine, EventCompiled, LocalizationCatalog,
};

use super::super::node_types::ToastState;
use super::state::PlayerSessionState;

#[path = "content.rs"]
mod content;
#[path = "controls.rs"]
mod controls;

pub(crate) struct PlayerVisualContext<'a> {
    pub project_root: Option<&'a Path>,
    pub stage_resolution: Option<(u32, u32)>,
    pub preview_quality: crate::editor::PreviewQuality,
    pub stage_fit: crate::editor::StageFit,
    pub image_cache: &'a mut HashMap<String, egui::TextureHandle>,
    pub image_failures: &'a mut HashMap<String, String>,
}

struct PlayerLocalizationContext<'a> {
    locale: &'a mut String,
    catalog: &'a LocalizationCatalog,
}

pub fn render_player_ui(
    engine: &mut Option<Engine>,
    toast: &mut Option<ToastState>,
    player: &mut PlayerSessionState,
    player_locale: &mut String,
    localization_catalog: &LocalizationCatalog,
    ctx: &egui::Context,
    visual: &mut PlayerVisualContext<'_>,
) -> Vec<AudioCommand> {
    let mut audio_commands = Vec::new();
    egui::CentralPanel::default().show(ctx, |ui| {
        if let Some(ref mut eng) = engine {
            let mut localization = PlayerLocalizationContext {
                locale: player_locale,
                catalog: localization_catalog,
            };
            audio_commands.extend(render_event_ui(
                ui,
                ctx,
                eng,
                toast,
                player,
                &mut localization,
                visual,
            ));
        } else {
            render_no_script_ui(ui);
        }
    });
    audio_commands
}

fn render_no_script_ui(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.heading("No script loaded");
            ui.add_space(10.0);
            ui.label("Use File -> Open Script to load a story");
        });
    });
}

#[instrument(skip_all)]
fn render_event_ui(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    player: &mut PlayerSessionState,
    localization: &mut PlayerLocalizationContext<'_>,
    visual: &mut PlayerVisualContext<'_>,
) -> Vec<AudioCommand> {
    let mut audio_commands = Vec::new();
    let now_sec = ctx.input(|i| i.time);
    let current_ip = engine.state().position;
    let ip_changed = player.on_position_changed(current_ip, now_sec);
    if ip_changed {
        audio_commands.extend(engine.take_audio_commands());
    }

    controls::render_header_bar(ui, engine, toast, player, now_sec, &mut audio_commands);
    ui.separator();
    controls::render_player_controls(ui, player, localization.locale, localization.catalog);
    controls::render_backlog_window(ctx, engine, player);
    controls::render_choice_history_window(ctx, engine, player);
    ui.separator();

    match engine.current_event() {
        Ok(event) => {
            if player.should_skip_current(&event, engine) {
                if matches!(event, EventCompiled::ExtCall { .. }) {
                    let _ = engine.resume();
                    audio_commands.extend(engine.take_audio_commands());
                } else if let Ok((cmd, _)) = engine.step() {
                    audio_commands.extend(cmd);
                }
                ctx.request_repaint_after(Duration::from_millis(16));
                return audio_commands;
            }

            ui.add_space(14.0);
            render_visual_state_for_event(ui, engine, &event, visual);
            match event {
                EventCompiled::Dialogue(d) => {
                    let localized_speaker = localize_inline_value(
                        d.speaker.as_ref(),
                        localization.locale,
                        localization.catalog,
                    );
                    let localized_text = localize_inline_value(
                        d.text.as_ref(),
                        localization.locale,
                        localization.catalog,
                    );
                    if content::render_dialogue(
                        ui,
                        ctx,
                        player,
                        &localized_speaker,
                        &localized_text,
                        now_sec,
                    ) {
                        if let Ok((cmd, _)) = engine.step() {
                            audio_commands.extend(cmd);
                        }
                    }
                }
                EventCompiled::Choice(c) => {
                    let localized_prompt = localize_inline_value(
                        c.prompt.as_ref(),
                        localization.locale,
                        localization.catalog,
                    );
                    let localized_options = c
                        .options
                        .iter()
                        .map(|option| {
                            localize_inline_value(
                                option.text.as_ref(),
                                localization.locale,
                                localization.catalog,
                            )
                        })
                        .collect::<Vec<_>>();
                    content::render_choice(
                        ui,
                        engine,
                        toast,
                        &localized_prompt,
                        &localized_options,
                        &c.options,
                        &mut audio_commands,
                    );
                }
                EventCompiled::Scene(_) => {
                    if content::render_scene(ui, player, now_sec) {
                        if let Ok((cmd, _)) = engine.step() {
                            audio_commands.extend(cmd);
                        }
                    }
                }
                EventCompiled::Transition(t) => {
                    content::render_transition(
                        ui,
                        ctx,
                        engine,
                        t.kind,
                        t.duration_ms,
                        &mut audio_commands,
                    );
                }
                EventCompiled::ExtCall { .. } => {
                    let _ = engine.resume();
                    audio_commands.extend(engine.take_audio_commands());
                    ctx.request_repaint_after(Duration::from_millis(16));
                }
                EventCompiled::Jump { .. }
                | EventCompiled::SetFlag { .. }
                | EventCompiled::SetVar { .. }
                | EventCompiled::JumpIf { .. }
                | EventCompiled::Patch(_)
                | EventCompiled::AudioAction(_)
                | EventCompiled::SetCharacterPosition(_) => {
                    if let Ok((cmd, _)) = engine.step() {
                        audio_commands.extend(cmd);
                    }
                    ctx.request_repaint_after(Duration::from_millis(16));
                }
            }
        }
        Err(e) => {
            if is_end_of_script_error(&e) {
                content::render_end(ui, engine, toast, player, now_sec, &mut audio_commands);
            } else {
                ui.colored_label(
                    egui::Color32::RED,
                    format!(
                        "Player runtime error at ip {}: {}",
                        engine.state().position,
                        e
                    ),
                );
            }
        }
    }
    audio_commands
}

pub(crate) fn is_end_of_script_error(error: &visual_novel_engine::VnError) -> bool {
    matches!(error, visual_novel_engine::VnError::EndOfScript)
}

fn render_visual_state_for_event(
    ui: &mut egui::Ui,
    engine: &Engine,
    event: &EventCompiled,
    visual: &mut PlayerVisualContext<'_>,
) {
    let display_visual =
        crate::editor::scene_stage::display_visual_for_event(engine.visual_state(), event);
    let scene = crate::editor::scene_stage::scene_from_visual_state(&display_visual);
    if scene.is_empty() {
        return;
    }

    let available = ui.available_size();
    let aspect_height = available.x * 9.0 / 16.0;
    let max_height = (available.y * 0.62).max(180.0);
    let desired_height = aspect_height.clamp(160.0, max_height);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(available.x.max(220.0), desired_height),
        egui::Sense::hover(),
    );
    let stage_size = visual
        .stage_resolution
        .map(|(w, h)| (w.max(1) as f32, h.max(1) as f32))
        .unwrap_or((1280.0, 720.0));
    let geometry = crate::editor::scene_stage::stage_geometry(rect, stage_size, visual.stage_fit);
    let mut painter = crate::editor::scene_stage::SceneStagePainter::new(
        visual.project_root,
        visual.preview_quality,
        visual.image_cache,
        visual.image_failures,
    );
    painter.paint_read_only(ui, &scene, geometry);
    ui.add_space(12.0);
}

fn localize_inline_value(
    raw: &str,
    locale: &str,
    localization_catalog: &LocalizationCatalog,
) -> String {
    if let Some(key) = localization_key(raw) {
        localization_catalog.resolve_or_key(locale, key)
    } else {
        raw.to_string()
    }
}

pub(crate) fn byte_index_for_char(text: &str, char_count: usize) -> usize {
    text.char_indices()
        .nth(char_count)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}
