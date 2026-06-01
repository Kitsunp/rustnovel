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

struct PreviewMenuRuntime<'a> {
    engine: &'a mut Engine,
    toast: &'a mut Option<ToastState>,
    player: &'a mut PlayerSessionState,
    audio_commands: &'a mut Vec<AudioCommand>,
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

fn render_player_menu_preview(
    ctx: &egui::Context,
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    player: &mut PlayerSessionState,
    menu_config: &PlayerMenuConfig,
    now_sec: f64,
    audio_commands: &mut Vec<AudioCommand>,
) {
    let menu = menu_config.normalized();
    if !menu.enabled || !player.show_menu {
        return;
    }
    if menu.tab_label(player.menu_tab).is_none() {
        player.menu_tab = menu.initial_tab();
    }
    let mut open = player.show_menu;
    let viewport = player_menu_reference_viewport(ctx);
    let menu_width = player_menu_window_width(viewport.x, &menu.style);
    let menu_height = player_menu_window_height(viewport.y, &menu.style);
    let max_menu_height = (viewport.y - 24.0).max(180.0);
    let mut frame = egui::Frame::window(&ctx.style());
    frame.fill = player_menu_color(menu.style.background, menu.style.panel_alpha);
    egui::Window::new(menu.title.clone())
        .open(&mut open)
        .default_width(menu_width)
        .default_height(menu_height)
        .max_width((viewport.x - 24.0).max(240.0))
        .min_height(180.0)
        .max_height(max_menu_height)
        .anchor(menu_anchor(menu.layout.panel_anchor), egui::Vec2::ZERO)
        .frame(frame)
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_max_width(menu_width);
            if menu.layout.quick_action_placement
                == visual_novel_engine::PlayerMenuQuickActionPlacement::MenuHeader
            {
                ui.horizontal_wrapped(|ui| {
                    for action in menu.quick_actions.iter().filter(|action| action.visible) {
                        let size = player_menu_action_button_size(
                            ui.available_width(),
                            action.action,
                            &action.label,
                            &menu.style,
                        );
                        if ui
                            .add_sized(
                                size,
                                egui::Button::new(player_menu_action_text(
                                    action.action,
                                    &action.label,
                                    &menu.style,
                                ))
                                .rounding(menu.style.button_corner_radius),
                            )
                            .clicked()
                        {
                            execute_preview_menu_action(
                                action.action,
                                engine,
                                toast,
                                player,
                                now_sec,
                                audio_commands,
                            );
                        }
                    }
                });
                ui.separator();
            }
            match menu.layout.tabs_position {
                visual_novel_engine::PlayerMenuTabsPosition::Top => {
                    render_preview_menu_tabs(ui, player, &menu);
                    ui.separator();
                    let mut runtime = PreviewMenuRuntime {
                        engine,
                        toast,
                        player,
                        audio_commands,
                    };
                    render_scrollable_preview_menu_tab(ui, ctx, &menu, now_sec, &mut runtime);
                }
                visual_novel_engine::PlayerMenuTabsPosition::Left => {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| render_preview_menu_tabs(ui, player, &menu));
                        ui.separator();
                        ui.vertical(|ui| {
                            let mut runtime = PreviewMenuRuntime {
                                engine,
                                toast,
                                player,
                                audio_commands,
                            };
                            render_scrollable_preview_menu_tab(
                                ui,
                                ctx,
                                &menu,
                                now_sec,
                                &mut runtime,
                            )
                        });
                    });
                }
            }
        });
    player.show_menu = open && player.show_menu;
}

fn render_scrollable_preview_menu_tab(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    menu: &PlayerMenuConfig,
    now_sec: f64,
    runtime: &mut PreviewMenuRuntime<'_>,
) {
    let viewport = player_menu_reference_viewport(ctx);
    let max_height = player_menu_content_height(viewport.y, &menu.style);
    egui::ScrollArea::vertical()
        .id_source("player_preview_menu_active_tab_scroll")
        .max_height(max_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            render_preview_menu_tab(
                ui,
                &mut *runtime.engine,
                &mut *runtime.toast,
                &mut *runtime.player,
                menu,
                now_sec,
                &mut *runtime.audio_commands,
            )
        });
}

fn render_preview_menu_tabs(
    ui: &mut egui::Ui,
    player: &mut PlayerSessionState,
    menu: &PlayerMenuConfig,
) {
    for tab in menu.tabs.iter().filter(|tab| tab.visible) {
        let selected = player.menu_tab == tab.kind;
        let label = if selected {
            egui::RichText::new(tab.label.clone()).color(player_menu_color(menu.style.accent, 255))
        } else {
            egui::RichText::new(tab.label.clone())
        };
        if ui.selectable_label(selected, label).clicked() {
            player.menu_tab = tab.kind;
        }
    }
}

fn render_preview_menu_tab(
    ui: &mut egui::Ui,
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    player: &mut PlayerSessionState,
    menu: &PlayerMenuConfig,
    now_sec: f64,
    audio_commands: &mut Vec<AudioCommand>,
) {
    match player.menu_tab {
        PlayerMenuTabKind::Saves => {
            render_preview_saves_tab(ui, engine, toast, player, menu, now_sec, audio_commands)
        }
        PlayerMenuTabKind::History => render_preview_history_tab(ui, engine),
        PlayerMenuTabKind::Routes => render_preview_routes_tab(ui, engine),
        PlayerMenuTabKind::Settings => render_preview_settings_tab(ui, player),
        PlayerMenuTabKind::System => {
            render_preview_system_tab(ui, engine, toast, player, menu, now_sec, audio_commands)
        }
    }
}

fn render_preview_settings_tab(ui: &mut egui::Ui, player: &mut PlayerSessionState) {
    ui.checkbox(&mut player.autoplay_enabled, "Auto");
    ui.checkbox(
        &mut player.advance_on_text_panel_click,
        "Text panel advances",
    );
    ui.add(egui::Slider::new(&mut player.autoplay_delay_ms, 200..=5000).text("Auto delay ms"));
    ui.add(egui::Slider::new(&mut player.text_chars_per_second, 10.0..=240.0).text("Text chars/s"));
    egui::ComboBox::from_id_source("player_menu_preview_skip_mode")
        .selected_text(match player.skip_mode {
            super::state::SkipMode::Off => "Skip: Off",
            super::state::SkipMode::ReadOnly => "Skip: Read",
            super::state::SkipMode::All => "Skip: All",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut player.skip_mode,
                super::state::SkipMode::Off,
                "Skip: Off",
            );
            ui.selectable_value(
                &mut player.skip_mode,
                super::state::SkipMode::ReadOnly,
                "Skip: Read",
            );
            ui.selectable_value(
                &mut player.skip_mode,
                super::state::SkipMode::All,
                "Skip: All",
            );
        });
    ui.separator();
    ui.label("Audio mix (preview):");
    ui.checkbox(&mut player.bgm_muted, "Mute BGM");
    ui.add(egui::Slider::new(&mut player.bgm_volume, 0.0..=1.0).text("BGM"));
    ui.checkbox(&mut player.sfx_muted, "Mute SFX");
    ui.add(egui::Slider::new(&mut player.sfx_volume, 0.0..=1.0).text("SFX"));
    ui.checkbox(&mut player.voice_muted, "Mute Voice");
    ui.add(egui::Slider::new(&mut player.voice_volume, 0.0..=1.0).text("Voice"));
}

fn render_preview_saves_tab(
    ui: &mut egui::Ui,
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    player: &mut PlayerSessionState,
    menu: &PlayerMenuConfig,
    now_sec: f64,
    audio_commands: &mut Vec<AudioCommand>,
) {
    ui.horizontal_wrapped(|ui| {
        let save_size =
            player_menu_text_button_size(ui.available_width(), "Quick Save", &menu.style);
        if ui
            .add_sized(
                save_size,
                egui::Button::new("Quick Save").rounding(menu.style.button_corner_radius),
            )
            .clicked()
        {
            player.quick_save_state = Some(engine.state().clone());
            *toast = Some(ToastState::success("Quick saved preview state"));
        }
        let can_load = player.quick_save_state.is_some();
        let load_response = ui
            .add_enabled_ui(can_load, |ui| {
                ui.add_sized(
                    player_menu_text_button_size(ui.available_width(), "Quick Load", &menu.style),
                    egui::Button::new("Quick Load").rounding(menu.style.button_corner_radius),
                )
            })
            .inner;
        if load_response.clicked() {
            if let Some(state) = player.quick_save_state.clone() {
                if engine.set_state(state).is_ok() {
                    player.reset_for_restart(now_sec);
                    audio_commands.extend(engine.take_audio_commands());
                    *toast = Some(ToastState::success("Quick loaded preview state"));
                }
            }
        }
    });
    ui.label("Preview saves are in-memory and reset when the editor player restarts.");
}

fn render_preview_history_tab(ui: &mut egui::Ui, engine: &Engine) {
    if engine.state().history.is_empty() {
        ui.label("No dialogue history yet.");
        return;
    }
    egui::ScrollArea::vertical()
        .id_source("player_preview_history_scroll")
        .max_height(320.0)
        .show(ui, |ui| {
            for entry in &engine.state().history {
                ui.label(format!("{}: {}", entry.speaker, entry.text));
                ui.separator();
            }
        });
}

fn render_preview_routes_tab(ui: &mut egui::Ui, engine: &Engine) {
    let route_tree = engine.route_tree();
    crate::editor::RouteTreeView::new(&route_tree)
        .with_choice_history(engine.choice_history())
        .ui(ui);
}

fn render_preview_system_tab(
    ui: &mut egui::Ui,
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    player: &mut PlayerSessionState,
    menu: &PlayerMenuConfig,
    now_sec: f64,
    audio_commands: &mut Vec<AudioCommand>,
) {
    if ui
        .add_sized(
            player_menu_text_button_size(ui.available_width(), "Restart Story", &menu.style),
            egui::Button::new(
                egui::RichText::new("Restart Story")
                    .color(player_menu_color(menu.style.warning, 255)),
            )
            .rounding(menu.style.button_corner_radius),
        )
        .clicked()
    {
        restart_preview_story(engine, toast, player, now_sec, audio_commands);
    }
    if ui
        .add_sized(
            player_menu_text_button_size(ui.available_width(), "Close Menu", &menu.style),
            egui::Button::new("Close Menu").rounding(menu.style.button_corner_radius),
        )
        .clicked()
    {
        player.show_menu = false;
    }
}

fn execute_preview_menu_action(
    action: PlayerMenuAction,
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    player: &mut PlayerSessionState,
    now_sec: f64,
    audio_commands: &mut Vec<AudioCommand>,
) {
    match action {
        PlayerMenuAction::ResumeGame => player.show_menu = false,
        PlayerMenuAction::OpenMenu => player.show_menu = true,
        PlayerMenuAction::QuickSave => {
            player.quick_save_state = Some(engine.state().clone());
            *toast = Some(ToastState::success("Quick saved preview state"));
        }
        PlayerMenuAction::QuickLoad => {
            if let Some(state) = player.quick_save_state.clone() {
                if engine.set_state(state).is_ok() {
                    player.reset_for_restart(now_sec);
                    audio_commands.extend(engine.take_audio_commands());
                    *toast = Some(ToastState::success("Quick loaded preview state"));
                }
            } else {
                *toast = Some(ToastState::warning("No preview quick save yet"));
            }
        }
        PlayerMenuAction::OpenSaves => player.menu_tab = PlayerMenuTabKind::Saves,
        PlayerMenuAction::OpenHistory => player.menu_tab = PlayerMenuTabKind::History,
        PlayerMenuAction::OpenRoutes => player.menu_tab = PlayerMenuTabKind::Routes,
        PlayerMenuAction::OpenSettings => player.menu_tab = PlayerMenuTabKind::Settings,
        PlayerMenuAction::OpenSystem => player.menu_tab = PlayerMenuTabKind::System,
        PlayerMenuAction::ToggleHistoryWindow => player.show_backlog = !player.show_backlog,
        PlayerMenuAction::ToggleFullscreen => {
            *toast = Some(ToastState::warning(
                "Fullscreen is available in the exported player",
            ));
        }
        PlayerMenuAction::RestartStory => {
            restart_preview_story(engine, toast, player, now_sec, audio_commands);
        }
        PlayerMenuAction::QuitGame => player.show_menu = false,
    }
}

fn restart_preview_story(
    engine: &mut Engine,
    toast: &mut Option<ToastState>,
    player: &mut PlayerSessionState,
    now_sec: f64,
    audio_commands: &mut Vec<AudioCommand>,
) {
    if engine.jump_to_label("start").is_ok() {
        engine.clear_session_history();
        player.reset_for_restart(now_sec);
        audio_commands.extend(engine.take_audio_commands());
        *toast = Some(ToastState::success("Story restarted"));
    }
}

fn menu_anchor(anchor: visual_novel_engine::PlayerMenuPanelAnchor) -> egui::Align2 {
    match anchor {
        visual_novel_engine::PlayerMenuPanelAnchor::Center => egui::Align2::CENTER_CENTER,
        visual_novel_engine::PlayerMenuPanelAnchor::TopLeft => egui::Align2::LEFT_TOP,
        visual_novel_engine::PlayerMenuPanelAnchor::TopRight => egui::Align2::RIGHT_TOP,
        visual_novel_engine::PlayerMenuPanelAnchor::BottomLeft => egui::Align2::LEFT_BOTTOM,
        visual_novel_engine::PlayerMenuPanelAnchor::BottomRight => egui::Align2::RIGHT_BOTTOM,
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
