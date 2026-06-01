use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use eframe::egui;
use tracing::instrument;
use visual_novel_engine::{
    localization_key,
    runtime::{AudioCommand, Engine, EventCompiled},
    LocalizationCatalog, PlayerMenuAction, PlayerMenuConfig, PlayerMenuTabKind,
};

use super::super::node_types::ToastState;
use super::state::PlayerSessionState;
use crate::app::{
    player_menu_action_button_size, player_menu_action_text, player_menu_color,
    player_menu_content_height, player_menu_reference_viewport, player_menu_text_button_size,
    player_menu_window_height, player_menu_window_width,
};
use crate::editor::resource_service::EditorResourceService;

#[path = "content.rs"]
mod content;
#[path = "controls.rs"]
mod controls;

#[path = "render/menu_preview.rs"]
mod menu_preview;
use menu_preview::render_player_menu_preview;
pub struct PlayerVisualContext<'a> {
    pub project_root: Option<&'a Path>,
    pub stage_resolution: Option<(u32, u32)>,
    pub preview_quality: crate::editor::PreviewQuality,
    pub stage_fit: crate::editor::StageFit,
    pub background_fit: crate::editor::BackgroundFit,
    pub image_cache: &'a mut HashMap<String, egui::TextureHandle>,
    pub image_failures: &'a mut HashMap<String, String>,
    pub resource_service: &'a mut EditorResourceService,
}

pub struct PlayerLocalizationContext<'a> {
    pub locale: &'a mut String,
    pub catalog: &'a LocalizationCatalog,
}

pub struct PlayerUiContext<'a, 'visual> {
    pub localization: PlayerLocalizationContext<'a>,
    pub menu_config: &'a PlayerMenuConfig,
    pub visual: &'a mut PlayerVisualContext<'visual>,
}

pub fn render_player_ui(
    engine: &mut Option<Engine>,
    toast: &mut Option<ToastState>,
    player: &mut PlayerSessionState,
    ctx: &egui::Context,
    context: &mut PlayerUiContext<'_, '_>,
) -> Vec<AudioCommand> {
    let mut audio_commands = Vec::new();
    egui::CentralPanel::default().show(ctx, |ui| {
        if let Some(ref mut eng) = engine {
            audio_commands.extend(render_event_ui(ui, ctx, eng, toast, player, context));
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
    context: &mut PlayerUiContext<'_, '_>,
) -> Vec<AudioCommand> {
    let mut audio_commands = Vec::new();
    let now_sec = ctx.input(|i| i.time);
    let current_ip = engine.state().position;
    let ip_changed = player.on_position_changed(current_ip, now_sec);
    if ip_changed {
        audio_commands.extend(engine.take_audio_commands());
    }
    if !player.menu_initialized {
        let menu = context.menu_config.normalized();
        player.show_menu = menu.enabled && menu.open_on_start;
        player.menu_tab = menu.initial_tab();
        player.advance_on_text_panel_click = menu.advance_on_text_panel_click;
        player.menu_initialized = true;
    }
    if context.menu_config.enabled && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        player.show_menu = !player.show_menu;
    }

    controls::render_header_bar(ui, engine, toast, player, now_sec, &mut audio_commands);
    ui.separator();
    controls::render_player_controls(
        ui,
        player,
        &mut *context.localization.locale,
        context.localization.catalog,
    );
    controls::render_backlog_window(ctx, engine, player);
    controls::render_choice_history_window(ctx, engine, player);
    ui.separator();

    match engine.current_event() {
        Ok(event) => {
            if player.should_skip_current(&event, engine) {
                if matches!(event, EventCompiled::ExtCall { .. }) {
                    resume_engine(engine, toast, &mut audio_commands);
                } else {
                    step_engine(engine, toast, &mut audio_commands);
                }
                ctx.request_repaint_after(Duration::from_millis(16));
                return audio_commands;
            }

            ui.add_space(14.0);
            let stage_geometry =
                render_visual_state_for_event(ui, engine, &event, &mut *context.visual);
            let advance_on_text_panel_click = player.advance_on_text_panel_click;
            match event {
                EventCompiled::Dialogue(d) => {
                    let localized_speaker = localize_inline_value(
                        d.speaker.as_ref(),
                        context.localization.locale.as_str(),
                        context.localization.catalog,
                    );
                    let localized_text = localize_inline_value(
                        d.text.as_ref(),
                        context.localization.locale.as_str(),
                        context.localization.catalog,
                    );
                    let should_advance = if let Some(geometry) = stage_geometry {
                        content::render_dialogue_overlay(
                            ui,
                            ctx,
                            player,
                            geometry,
                            content::DialogueOverlayContext {
                                speaker: &localized_speaker,
                                text: &localized_text,
                                now_sec,
                                advance_on_text_panel_click,
                            },
                        )
                    } else {
                        content::render_dialogue(
                            ui,
                            ctx,
                            player,
                            &localized_speaker,
                            &localized_text,
                            now_sec,
                            advance_on_text_panel_click,
                        )
                    };
                    if should_advance {
                        step_engine(engine, toast, &mut audio_commands);
                    }
                }
                EventCompiled::Choice(c) => {
                    let localized_prompt = localize_inline_value(
                        c.prompt.as_ref(),
                        context.localization.locale.as_str(),
                        context.localization.catalog,
                    );
                    let localized_options = c
                        .options
                        .iter()
                        .map(|option| {
                            localize_inline_value(
                                option.text.as_ref(),
                                context.localization.locale.as_str(),
                                context.localization.catalog,
                            )
                        })
                        .collect::<Vec<_>>();
                    if let Some(geometry) = stage_geometry {
                        content::render_choice_overlay(
                            ui,
                            geometry,
                            engine,
                            toast,
                            content::ChoiceOverlayContent {
                                prompt: &localized_prompt,
                                localized_options: &localized_options,
                                options: &c.options,
                            },
                            &mut audio_commands,
                        );
                    } else {
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
                }
                EventCompiled::Scene(_) => {
                    let description = "Scene updated";
                    let should_advance = if let Some(geometry) = stage_geometry {
                        content::render_scene_overlay(
                            ui,
                            player,
                            geometry,
                            description,
                            now_sec,
                            advance_on_text_panel_click,
                        )
                    } else {
                        content::render_scene(ui, player, now_sec)
                    };
                    if should_advance {
                        step_engine(engine, toast, &mut audio_commands);
                    }
                }
                EventCompiled::Transition(t) => {
                    content::render_transition(
                        ui,
                        ctx,
                        engine,
                        toast,
                        t.kind,
                        t.duration_ms,
                        &mut audio_commands,
                    );
                }
                EventCompiled::ExtCall { .. } => {
                    resume_engine(engine, toast, &mut audio_commands);
                    ctx.request_repaint_after(Duration::from_millis(16));
                }
                EventCompiled::Jump { .. }
                | EventCompiled::SetFlag { .. }
                | EventCompiled::SetVar { .. }
                | EventCompiled::JumpIf { .. }
                | EventCompiled::Patch(_)
                | EventCompiled::AudioAction(_)
                | EventCompiled::SetCharacterPosition(_) => {
                    step_engine(engine, toast, &mut audio_commands);
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
    render_player_menu_preview(
        ctx,
        engine,
        toast,
        player,
        context.menu_config,
        now_sec,
        &mut audio_commands,
    );
    audio_commands
}

fn step_engine(
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    audio_commands: &mut Vec<AudioCommand>,
) {
    match engine.step() {
        Ok((cmd, _)) => audio_commands.extend(cmd),
        Err(err) => {
            *toast = Some(ToastState::error(format!(
                "Engine step failed at ip {}: {err}",
                engine.state().position
            )));
        }
    }
}

fn resume_engine(
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    audio_commands: &mut Vec<AudioCommand>,
) {
    match engine.resume() {
        Ok(()) => audio_commands.extend(engine.take_audio_commands()),
        Err(err) => {
            *toast = Some(ToastState::error(format!(
                "Engine resume failed at ip {}: {err}",
                engine.state().position
            )));
        }
    }
}

pub fn is_end_of_script_error(error: &visual_novel_engine::VnError) -> bool {
    matches!(error, visual_novel_engine::VnError::EndOfScript)
}

fn render_visual_state_for_event(
    ui: &mut egui::Ui,
    engine: &Engine,
    event: &EventCompiled,
    visual: &mut PlayerVisualContext<'_>,
) -> Option<crate::editor::scene_stage::StageGeometry> {
    let display_visual =
        crate::editor::scene_stage::display_visual_for_event(engine.visual_state(), event);
    let scene = crate::editor::scene_stage::scene_from_visual_state(&display_visual);
    if scene.is_empty() {
        return None;
    }

    let available = ui.available_size();
    let stage_size = visual
        .stage_resolution
        .map(|(w, h)| (w.max(1) as f32, h.max(1) as f32))
        .unwrap_or((1280.0, 720.0));
    let viewport_size = player_stage_viewport_size(available, stage_size);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(viewport_size.x, viewport_size.y),
        egui::Sense::hover(),
    );
    let geometry = crate::editor::scene_stage::stage_geometry(rect, stage_size, visual.stage_fit);
    let mut painter = crate::editor::scene_stage::SceneStagePainter::new(
        visual.project_root,
        visual.preview_quality,
        visual.image_cache,
        visual.image_failures,
        visual.resource_service,
    )
    .with_background_fit(visual.background_fit);
    painter.paint_read_only(ui, &scene, geometry);
    ui.add_space(12.0);
    Some(geometry)
}

pub fn player_stage_viewport_size(available: egui::Vec2, stage_size: (f32, f32)) -> egui::Vec2 {
    crate::editor::visual_composer_preview::stage_viewport_size(available, stage_size, 0.0, 0.62)
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

pub fn byte_index_for_char(text: &str, char_count: usize) -> usize {
    text.char_indices()
        .nth(char_count)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}
