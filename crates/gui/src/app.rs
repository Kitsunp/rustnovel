use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use eframe::egui;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use visual_novel_engine::{
    compute_script_id,
    runtime::{ChoiceHistoryEntry, Engine, EventCompiled, ScriptRaw, UiState, UiView, VisualState},
    PlayerMenuAction, PlayerMenuConfig, PlayerMenuPanelAnchor, PlayerMenuQuickActionPlacement,
    PlayerMenuStyleConfig, PlayerMenuTabKind, PlayerMenuTabsPosition, ResourceLimiter,
    SaveSlotEntry, SaveSlotStore, ScriptId, SecurityPolicy, VnError,
};

use crate::assets::{AssetManager, AssetStore, SecurityMode};
use crate::persist::{PersistError, UserPreferences};
use crate::widgets::{event_kind, history_bytes};

#[path = "app/audio.rs"]
mod audio;
#[path = "app/eframe_impl.rs"]
mod eframe_impl;

pub use audio::{
    audio_command_label as player_audio_command_label, sanitize_volume, PlayerAudioChannel,
    PlayerAudioMix,
};
use audio::{PlayerAudioChannelState, PlayerAudioController};

const DEFAULT_STAGE_SIZE: (f32, f32) = (1280.0, 720.0);
const PASSTHROUGH_EVENT_LIMIT: usize = 32;
const PLAYER_STATUS_LINE_HEIGHT: f32 = 22.0;
const PLAYER_STATUS_BAR_PADDING: f32 = 6.0;

#[derive(Clone, Debug, Default)]
pub struct DisplayInfo {
    pub width: f32,
    pub height: f32,
    pub scale_factor: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct VnConfig {
    pub title: String,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub fullscreen: bool,
    pub scale_factor: Option<f32>,
    pub assets_root: Option<PathBuf>,
    pub asset_cache_budget_mb: Option<u64>,
    pub security_mode: SecurityMode,
    pub manifest_path: Option<PathBuf>,
    pub require_manifest: Option<bool>,
    #[serde(default)]
    pub preferences_path: Option<PathBuf>,
    #[serde(default)]
    pub player_menu: PlayerMenuConfig,
}

impl Default for VnConfig {
    fn default() -> Self {
        Self {
            title: "Visual Novel".to_string(),
            width: None,
            height: None,
            fullscreen: false,
            scale_factor: None,
            assets_root: None,
            asset_cache_budget_mb: Some(128),
            security_mode: SecurityMode::Trusted,
            manifest_path: None,
            require_manifest: None,
            preferences_path: None,
            player_menu: PlayerMenuConfig::default(),
        }
    }
}

impl VnConfig {
    pub fn resolve(&self, display: Option<DisplayInfo>) -> ResolvedConfig {
        let mut width = self.width.unwrap_or(1280.0);
        let mut height = self.height.unwrap_or(720.0);
        let mut fullscreen = self.fullscreen;
        let mut ui_scale = 1.0;
        let mut scale_factor = self.scale_factor.unwrap_or(1.0);

        if let Some(display) = display {
            scale_factor = self.scale_factor.unwrap_or(display.scale_factor.max(1.0));
            if (self.width.is_none() || self.height.is_none()) && display.height < 720.0 {
                fullscreen = true;
                width = display.width;
                height = display.height;
                ui_scale = 1.1;
            }
        }

        let asset_cache_budget_mb = self.asset_cache_budget_mb.unwrap_or(128);
        let asset_cache_budget_bytes = (asset_cache_budget_mb * 1024 * 1024) as usize;
        let assets_root = self
            .assets_root
            .clone()
            .unwrap_or_else(|| PathBuf::from("assets"));
        let require_manifest = self
            .require_manifest
            .unwrap_or(self.security_mode == SecurityMode::Untrusted);
        let player_menu = self.player_menu.normalized();

        ResolvedConfig {
            title: self.title.clone(),
            width,
            height,
            fullscreen,
            scale_factor,
            ui_scale,
            assets_root,
            asset_cache_budget_bytes,
            security_mode: self.security_mode,
            manifest_path: self.manifest_path.clone(),
            require_manifest,
            player_menu,
        }
    }

    pub fn preferences_path(&self) -> PathBuf {
        if let Some(path) = &self.preferences_path {
            return path.clone();
        }
        ProjectDirs::from("com", "vnengine", "visual_novel")
            .map(|dirs| dirs.config_dir().join("prefs.json"))
            .unwrap_or_else(|| PathBuf::from("prefs.json"))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedConfig {
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub fullscreen: bool,
    pub scale_factor: f32,
    pub ui_scale: f32,
    pub assets_root: PathBuf,
    pub asset_cache_budget_bytes: usize,
    pub security_mode: SecurityMode,
    pub manifest_path: Option<PathBuf>,
    pub require_manifest: bool,
    pub player_menu: PlayerMenuConfig,
}

#[derive(Debug, Error)]
pub enum GuiError {
    #[error("script error: {0}")]
    Script(#[from] VnError),
    #[error("gui error: {0}")]
    Gui(#[from] eframe::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("persist error: {0}")]
    Persist(#[from] PersistError),
    #[error("asset error: {0}")]
    Asset(#[from] crate::assets::AssetError),
}

pub fn run_app(script_json: String, config: Option<VnConfig>) -> Result<(), GuiError> {
    let script = ScriptRaw::from_json(&script_json)?;
    let engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )?;
    let compiled_bytes = engine.script().to_binary()?;
    let script_id = compute_script_id(&compiled_bytes);
    let config = config.unwrap_or_default();
    let preferences_path = config.preferences_path();
    let preferences = UserPreferences::load_from(&preferences_path).unwrap_or_default();
    let resolved = config.resolve(None);
    let title = resolved.title.clone();
    let options = native_options(&resolved, &preferences);
    let asset_store = AssetStore::new(
        resolved.assets_root.clone(),
        resolved.security_mode,
        resolved.manifest_path.clone(),
        resolved.require_manifest,
    )?;
    let assets = AssetManager::new(asset_store, resolved.asset_cache_budget_bytes);

    eframe::run_native(
        &title,
        options,
        Box::new(move |cc| {
            Box::new(VnApp::new(
                engine,
                resolved,
                preferences,
                preferences_path,
                script_id,
                assets,
                cc,
            ))
        }),
    )?;
    Ok(())
}

pub fn player_stage_viewport_size(available: egui::Vec2, stage_size: (f32, f32)) -> egui::Vec2 {
    let max_width = available.x.max(1.0);
    let max_height = available.y.max(1.0);
    let (stage_w, stage_h) = stage_size;
    if !stage_w.is_finite() || !stage_h.is_finite() || stage_w <= 0.0 || stage_h <= 0.0 {
        return egui::vec2(max_width, max_height);
    }

    let aspect = stage_w / stage_h;
    let mut width = max_width;
    let mut height = width / aspect;
    if height > max_height {
        height = max_height;
        width = height * aspect;
    }
    egui::vec2(
        width.max(1.0).min(max_width),
        height.max(1.0).min(max_height),
    )
}

pub fn player_status_bar_height(has_status: bool, has_error: bool) -> f32 {
    let lines = usize::from(has_status) + usize::from(has_error);
    if lines == 0 {
        0.0
    } else {
        PLAYER_STATUS_BAR_PADDING + PLAYER_STATUS_LINE_HEIGHT * lines as f32
    }
}

pub fn player_stage_available_size(
    available: egui::Vec2,
    reserved_status_height: f32,
) -> egui::Vec2 {
    egui::vec2(
        available.x.max(1.0),
        (available.y - reserved_status_height.max(0.0)).max(1.0),
    )
}

pub fn player_event_requires_input(event: &EventCompiled) -> bool {
    matches!(
        event,
        EventCompiled::Dialogue(_) | EventCompiled::Choice(_) | EventCompiled::Transition(_)
    )
}

pub fn player_menu_window_width(viewport_width: f32, style: &PlayerMenuStyleConfig) -> f32 {
    let viewport_width = viewport_width.max(1.0);
    let viewport_cap = (viewport_width - 24.0).max(240.0);
    let fraction_cap = (viewport_width * style.panel_width_fraction).min(viewport_cap);
    style
        .panel_width
        .min(fraction_cap)
        .min(viewport_cap)
        .max(viewport_cap.min(240.0))
}

pub fn player_menu_reference_viewport(ctx: &egui::Context) -> egui::Vec2 {
    let current = ctx.available_rect().size();
    let monitor = ctx.input(|input| input.viewport().monitor_size);
    match monitor {
        Some(monitor) if monitor.x.is_finite() && monitor.y.is_finite() => {
            egui::vec2(current.x.min(monitor.x), current.y.min(monitor.y))
        }
        _ => current,
    }
}

pub fn player_menu_window_height(viewport_height: f32, style: &PlayerMenuStyleConfig) -> f32 {
    let viewport_height = viewport_height.max(1.0);
    let viewport_cap = (viewport_height - 32.0).max(180.0);
    let fraction_cap = (viewport_height * style.panel_height_fraction).min(viewport_cap);
    fraction_cap.max(viewport_cap.min(180.0))
}

pub fn player_menu_content_height(viewport_height: f32, style: &PlayerMenuStyleConfig) -> f32 {
    (player_menu_window_height(viewport_height, style) - 132.0).max(96.0)
}

pub fn player_menu_action_button_size(
    available_width: f32,
    action: PlayerMenuAction,
    label: &str,
    style: &PlayerMenuStyleConfig,
) -> egui::Vec2 {
    let desired = menu_action_button_width(action, label).max(style.button_min_width);
    egui::vec2(
        desired.min(available_width.max(1.0)),
        style.button_height.max(1.0),
    )
}

pub fn player_menu_text_button_size(
    available_width: f32,
    label: &str,
    style: &PlayerMenuStyleConfig,
) -> egui::Vec2 {
    let desired = (label.chars().count() as f32 * 8.0 + 28.0).max(style.button_min_width);
    egui::vec2(
        desired.min(available_width.max(1.0)),
        style.button_height.max(1.0),
    )
}

pub fn player_menu_color(color: visual_novel_engine::PlayerMenuColor, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_premultiplied(color.r, color.g, color.b, alpha.min(color.a))
}

pub fn player_menu_action_text(
    action: PlayerMenuAction,
    label: &str,
    style: &PlayerMenuStyleConfig,
) -> egui::RichText {
    let color = match action {
        PlayerMenuAction::ResumeGame
        | PlayerMenuAction::OpenMenu
        | PlayerMenuAction::OpenSaves
        | PlayerMenuAction::OpenHistory
        | PlayerMenuAction::OpenRoutes
        | PlayerMenuAction::OpenSettings
        | PlayerMenuAction::OpenSystem => Some(player_menu_color(style.accent, 255)),
        PlayerMenuAction::RestartStory | PlayerMenuAction::ToggleFullscreen => {
            Some(player_menu_color(style.warning, 255))
        }
        PlayerMenuAction::QuitGame => Some(player_menu_color(style.danger, 255)),
        PlayerMenuAction::QuickSave
        | PlayerMenuAction::QuickLoad
        | PlayerMenuAction::ToggleHistoryWindow => None,
    };
    let text = egui::RichText::new(label.to_string());
    match color {
        Some(color) => text.color(color),
        None => text,
    }
}

pub fn player_menu_action_enabled(action: PlayerMenuAction, has_quicksave: bool) -> bool {
    !matches!(action, PlayerMenuAction::QuickLoad) || has_quicksave
}

pub fn player_text_panel_advance_enabled(menu: &PlayerMenuConfig, prefs: &UserPreferences) -> bool {
    prefs
        .advance_on_text_panel_click
        .unwrap_or(menu.advance_on_text_panel_click)
}

pub fn player_route_history_label(index: usize, entry: &ChoiceHistoryEntry) -> String {
    let prompt = entry.prompt.trim();
    let option = entry.option_text.trim();
    let prompt = if prompt.is_empty() { "Choice" } else { prompt };
    let option = if option.is_empty() {
        format!("Option {}", entry.option_index + 1)
    } else {
        option.to_string()
    };
    format!("{}. {prompt} -> {option}", index + 1)
}

fn native_options(resolved: &ResolvedConfig, prefs: &UserPreferences) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([resolved.width.max(1.0), resolved.height.max(1.0)]);
    if resolved.fullscreen || prefs.fullscreen {
        viewport = viewport.with_fullscreen(true);
    }

    eframe::NativeOptions {
        viewport,
        vsync: prefs.vsync,
        ..Default::default()
    }
}

struct VnApp {
    engine: Engine,
    config: ResolvedConfig,
    prefs: UserPreferences,
    prefs_path: PathBuf,
    show_menu: bool,
    show_history: bool,
    show_inspector: bool,
    last_error: Option<String>,
    assets: AssetManager,
    audio: PlayerAudioController,
    applied_scale: f32,
    label_jump_input: String,
    script_id: ScriptId,
    last_status: Option<String>,
    save_store: SaveSlotStore,
    menu_tab: PlayerMenuTabKind,
}

impl VnApp {
    fn new(
        engine: Engine,
        config: ResolvedConfig,
        mut prefs: UserPreferences,
        prefs_path: PathBuf,
        script_id: ScriptId,
        assets: AssetManager,
        cc: &eframe::CreationContext<'_>,
    ) -> Self {
        if config.fullscreen {
            prefs.fullscreen = true;
        }
        let audio = PlayerAudioController::new(
            config.assets_root.clone(),
            config.security_mode,
            config.manifest_path.clone(),
            config.require_manifest,
        );
        let save_store = SaveSlotStore::new(player_save_root_for_script(&prefs_path, &script_id));
        let menu_tab = config.player_menu.initial_tab();
        let show_menu = config.player_menu.enabled && config.player_menu.open_on_start;
        let mut app = Self {
            engine,
            config,
            prefs,
            prefs_path,
            show_menu,
            show_history: false,
            show_inspector: false,
            last_error: None,
            assets,
            audio,
            applied_scale: 0.0,
            label_jump_input: String::new(),
            script_id,
            last_status: None,
            save_store,
            menu_tab,
        };
        let scale = app.config.scale_factor * app.prefs.ui_scale;
        cc.egui_ctx.set_pixels_per_point(scale.max(0.5));
        app.applied_scale = scale;
        app.apply_audio_preferences();
        let initial_audio = app.engine.take_audio_commands();
        app.apply_audio_commands(initial_audio);
        app
    }

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.heading(&self.config.title);
            ui.separator();
            if self.config.player_menu.enabled
                && self.config.player_menu.layout.quick_action_placement
                    == PlayerMenuQuickActionPlacement::Toolbar
            {
                self.render_quick_action_buttons(ui);
            }
            if let Some(last_warning) = self.audio.last_warning() {
                ui.separator();
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!("Audio warning: {last_warning}"),
                );
            }
        });
    }

    fn render_quick_action_buttons(&mut self, ui: &mut egui::Ui) {
        let style = self.config.player_menu.style.clone();
        let has_quicksave = self.save_store.has_quicksave().unwrap_or(false);
        for action in self.config.player_menu.quick_actions.clone() {
            if !action.visible {
                continue;
            }
            let size = player_menu_action_button_size(
                ui.available_width(),
                action.action,
                &action.label,
                &style,
            );
            let response = ui
                .add_enabled_ui(
                    player_menu_action_enabled(action.action, has_quicksave),
                    |ui| {
                        ui.add_sized(
                            size,
                            egui::Button::new(player_menu_action_text(
                                action.action,
                                &action.label,
                                &style,
                            ))
                            .rounding(style.button_corner_radius),
                        )
                    },
                )
                .inner;
            if response.clicked() {
                self.execute_menu_action(action.action, None);
            }
        }
    }

    fn render_player_stage(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        reserved_status_height: f32,
    ) {
        let event = self.engine.current_event();
        let visual = self.preview_visual_state(event.as_ref().ok());
        let available = player_stage_available_size(ui.available_size(), reserved_status_height);
        let viewport = player_stage_viewport_size(available, DEFAULT_STAGE_SIZE);
        let (viewport_rect, _) = ui.allocate_exact_size(viewport, egui::Sense::hover());
        let stage_rect = fit_rect_to_stage(viewport_rect, DEFAULT_STAGE_SIZE);

        let painter = ui.painter().with_clip_rect(stage_rect);
        painter.rect_filled(stage_rect, 0.0, egui::Color32::from_rgb(16, 18, 24));
        self.paint_background(ui, stage_rect, visual.background.as_deref());
        self.paint_character_labels(ui, stage_rect, &visual);

        match event {
            Ok(event) => self.render_event_overlay(ui, ctx, stage_rect, event),
            Err(VnError::EndOfScript) => self.render_end_overlay(ui, stage_rect),
            Err(err) => self.last_error = Some(err.to_string()),
        }
    }

    fn paint_background(
        &mut self,
        ui: &mut egui::Ui,
        stage_rect: egui::Rect,
        background: Option<&str>,
    ) {
        let Some(background) = background else {
            return;
        };
        match self.assets.texture_for_asset(ui.ctx(), background) {
            Ok(Some(texture)) => {
                let image_rect = cover_rect(stage_rect, texture.size_vec2());
                let painter = ui.painter().with_clip_rect(stage_rect);
                painter.image(
                    texture.id(),
                    image_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            Ok(None) => {}
            Err(err) => self.last_error = Some(format!("Asset error: {err}")),
        }
    }

    fn paint_character_labels(&self, ui: &egui::Ui, stage_rect: egui::Rect, visual: &VisualState) {
        if visual.characters.is_empty() {
            return;
        }
        let painter = ui.painter().with_clip_rect(stage_rect);
        let mut x = stage_rect.left() + 18.0;
        let y = stage_rect.top() + 18.0;
        for character in &visual.characters {
            let text = character
                .expression
                .as_ref()
                .map(|expression| format!("{} ({})", character.name, expression))
                .unwrap_or_else(|| character.name.as_ref().to_string());
            let galley = ui.painter().layout_no_wrap(
                text,
                egui::FontId::proportional(15.0),
                egui::Color32::WHITE,
            );
            let rect =
                egui::Rect::from_min_size(egui::pos2(x, y), galley.size() + egui::vec2(20.0, 10.0));
            painter.rect_filled(
                rect,
                5.0,
                egui::Color32::from_rgba_premultiplied(8, 10, 16, 190),
            );
            painter.galley(
                rect.min + egui::vec2(10.0, 5.0),
                galley,
                egui::Color32::WHITE,
            );
            x = rect.right() + 8.0;
        }
    }

    fn render_event_overlay(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        stage_rect: egui::Rect,
        event: EventCompiled,
    ) {
        let view = UiState::from_event(&event, self.engine.visual_state()).view;
        match view {
            UiView::Dialogue { speaker, text } => {
                self.render_dialogue_overlay(ui, ctx, stage_rect, &speaker, &text);
            }
            UiView::Choice { prompt, options } => {
                self.render_choice_overlay(ui, stage_rect, &prompt, &options);
            }
            UiView::Scene { .. } => {
                self.render_scene_overlay(ui, stage_rect, "Scene updated");
            }
            UiView::System { message } => {
                self.render_scene_overlay(ui, stage_rect, &message);
            }
        }
    }

    fn render_dialogue_overlay(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        stage_rect: egui::Rect,
        speaker: &str,
        text: &str,
    ) {
        let rect = crate::player_overlay::dialogue_overlay_rect(stage_rect);
        ui.painter().rect_filled(
            rect,
            6.0,
            egui::Color32::from_rgba_premultiplied(8, 8, 14, 225),
        );
        ui.painter().rect_stroke(
            rect,
            6.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(130)),
        );

        let mut should_advance = false;
        ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(18.0, 12.0)), |ui| {
            ui.set_clip_rect(rect.shrink(8.0));
            ui.label(
                egui::RichText::new(speaker)
                    .color(egui::Color32::from_rgb(185, 214, 255))
                    .strong(),
            );
            ui.add_space(6.0);
            ui.add_sized(
                [ui.available_width(), (rect.height() - 72.0).max(28.0)],
                egui::Label::new(egui::RichText::new(text).color(egui::Color32::WHITE)).wrap(true),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Continue").clicked() {
                    should_advance = true;
                }
            });
        });
        if player_text_panel_advance_enabled(&self.config.player_menu, &self.prefs) {
            if ui
                .interact(
                    rect,
                    egui::Id::new(("standalone_dialogue_overlay", self.engine.state().position)),
                    egui::Sense::click(),
                )
                .clicked()
            {
                should_advance = true;
            }
        }
        if should_advance {
            self.advance();
            ctx.request_repaint();
        }
    }

    fn render_choice_overlay(
        &mut self,
        ui: &mut egui::Ui,
        stage_rect: egui::Rect,
        prompt: &str,
        options: &[String],
    ) {
        let layout = crate::player_overlay::choice_overlay_layout(stage_rect, prompt, options);
        ui.painter().rect_filled(
            layout.panel,
            6.0,
            egui::Color32::from_rgba_premultiplied(10, 12, 18, 232),
        );
        ui.painter().rect_stroke(
            layout.panel,
            6.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(120)),
        );

        let mut selected = None;
        ui.allocate_ui_at_rect(layout.panel.shrink2(egui::vec2(18.0, 14.0)), |ui| {
            ui.set_clip_rect(layout.panel.shrink(8.0));
            ui.add_sized(
                [ui.available_width(), layout.prompt_height],
                egui::Label::new(
                    egui::RichText::new(crate::player_overlay::soft_wrap_long_tokens(prompt, 28))
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .wrap(true),
            );
            ui.add_space(10.0);
            egui::ScrollArea::vertical()
                .id_source("standalone_player_choice_overlay_scroll")
                .max_height(layout.options_viewport_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (idx, option) in options.iter().enumerate() {
                        let row_height = layout.option_heights.get(idx).copied().unwrap_or(38.0);
                        if ui
                            .add_sized(
                                [ui.available_width(), row_height],
                                egui::Button::new(crate::player_overlay::soft_wrap_long_tokens(
                                    option, 32,
                                )),
                            )
                            .clicked()
                        {
                            selected = Some(idx);
                        }
                        ui.add_space(8.0);
                    }
                });
        });
        if let Some(index) = selected {
            self.choose(index);
        }
    }

    fn render_scene_overlay(
        &mut self,
        ui: &mut egui::Ui,
        stage_rect: egui::Rect,
        description: &str,
    ) {
        let rect = crate::player_overlay::scene_overlay_rect(stage_rect);
        ui.painter().rect_filled(
            rect,
            6.0,
            egui::Color32::from_rgba_premultiplied(8, 8, 14, 215),
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
        if player_text_panel_advance_enabled(&self.config.player_menu, &self.prefs) {
            if ui
                .interact(
                    rect,
                    egui::Id::new("standalone_player_scene_overlay"),
                    egui::Sense::click(),
                )
                .clicked()
            {
                should_advance = true;
            }
        }
        if should_advance {
            self.advance();
        }
    }

    fn render_end_overlay(&mut self, ui: &mut egui::Ui, stage_rect: egui::Rect) {
        let rect = egui::Rect::from_center_size(stage_rect.center(), egui::vec2(320.0, 150.0));
        ui.painter().rect_filled(
            rect,
            6.0,
            egui::Color32::from_rgba_premultiplied(18, 14, 24, 230),
        );
        ui.allocate_ui_at_rect(rect.shrink(18.0), |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("The End");
                ui.add_space(16.0);
                if ui.button("Play Again").clicked() {
                    if let Err(err) = self.engine.jump_to_label("start") {
                        self.last_error = Some(err.to_string());
                    } else {
                        let audio = self.engine.take_audio_commands();
                        self.apply_audio_commands(audio);
                        self.last_status = Some("Restarted".to_string());
                    }
                }
            });
        });
    }

    fn preview_visual_state(&self, event: Option<&EventCompiled>) -> VisualState {
        let mut visual = self.engine.visual_state().clone();
        match event {
            Some(EventCompiled::Scene(scene)) => visual.apply_scene(scene),
            Some(EventCompiled::Patch(patch)) => visual.apply_patch(patch),
            _ => {}
        }
        visual
    }

    fn advance_passthrough_events(&mut self, ctx: &egui::Context) {
        for _ in 0..PASSTHROUGH_EVENT_LIMIT {
            let event = match self.engine.current_event() {
                Ok(event) => event,
                Err(VnError::EndOfScript) => return,
                Err(err) => {
                    self.last_error = Some(err.to_string());
                    return;
                }
            };

            match event {
                EventCompiled::Jump { .. }
                | EventCompiled::SetFlag { .. }
                | EventCompiled::SetVar { .. }
                | EventCompiled::JumpIf { .. }
                | EventCompiled::Scene(_)
                | EventCompiled::Patch(_)
                | EventCompiled::AudioAction(_)
                | EventCompiled::SetCharacterPosition(_) => self.advance(),
                EventCompiled::ExtCall { .. } => match self.engine.resume() {
                    Ok(()) => {
                        let audio = self.engine.take_audio_commands();
                        self.apply_audio_commands(audio);
                    }
                    Err(err) => {
                        self.last_error = Some(err.to_string());
                        return;
                    }
                },
                EventCompiled::Dialogue(_)
                | EventCompiled::Choice(_)
                | EventCompiled::Transition(_) => return,
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        self.last_error = Some("Stopped auto-advancing after too many system events".to_string());
    }

    fn render_history(&self, ctx: &egui::Context) {
        if !self.show_history {
            return;
        }
        egui::Window::new("History").show(ctx, |ui| {
            for entry in &self.engine.state().history {
                ui.label(format!("{}: {}", entry.speaker, entry.text));
                ui.separator();
            }
        });
    }

    fn render_inspector(&mut self, ctx: &egui::Context) {
        if !self.show_inspector {
            return;
        }
        let event_summary = match self.engine.current_event() {
            Ok(event) => event_kind(&event),
            Err(err) => format!("Error: {err}"),
        };
        let history_bytes = history_bytes(&self.engine.state().history);
        let dt = ctx.input(|i| i.unstable_dt);
        let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        let cache_stats = self.assets.stats();
        egui::Window::new("Inspector").show(ctx, |ui| {
            ui.label(format!("IP: {}", self.engine.state().position));
            ui.label(format!("Event: {event_summary}"));
            ui.label(format!("FPS: {:.1}", fps));
            ui.label(format!("History bytes (approx): {}", history_bytes));
            ui.label(format!(
                "Texture cache: {} entries, {} MB (budget {} MB)",
                cache_stats.entries,
                cache_stats.bytes / (1024 * 1024),
                cache_stats.budget_bytes / (1024 * 1024)
            ));
            ui.label(format!(
                "Cache hits: {}, misses: {}, evictions: {}",
                cache_stats.hits, cache_stats.misses, cache_stats.evictions
            ));
            ui.separator();
            ui.label("Flags:");
            let flag_count = self.engine.flag_count();
            for flag_id in 0..flag_count {
                let mut value = self.engine.state().get_flag(flag_id);
                if ui.checkbox(&mut value, format!("flag {flag_id}")).changed() {
                    self.engine.set_flag(flag_id, value);
                }
            }
            ui.separator();
            ui.label("Jump to label:");
            ui.text_edit_singleline(&mut self.label_jump_input);
            if ui.button("Jump").clicked() {
                if let Err(err) = self.engine.jump_to_label(&self.label_jump_input) {
                    self.last_error = Some(err.to_string());
                }
            }
            ui.separator();
            ui.label("Available labels:");
            for label in self.engine.labels().keys() {
                ui.label(label);
            }
        });
    }

    fn advance(&mut self) {
        match self.engine.step() {
            Ok((audio, _change)) => self.apply_audio_commands(audio),
            Err(VnError::EndOfScript) => {}
            Err(err) => self.last_error = Some(err.to_string()),
        }
    }

    fn choose(&mut self, index: usize) {
        match self.engine.choose(index) {
            Ok(_) => {
                let audio = self.engine.take_audio_commands();
                self.apply_audio_commands(audio);
            }
            Err(VnError::EndOfScript) => {}
            Err(err) => self.last_error = Some(err.to_string()),
        }
    }

    fn apply_audio_commands(&mut self, commands: Vec<visual_novel_engine::runtime::AudioCommand>) {
        self.audio.apply_commands(commands);
    }

    fn audio_mix_from_preferences(&self) -> PlayerAudioMix {
        PlayerAudioMix {
            master: self.prefs.master_volume,
            bgm: self.prefs.bgm_volume,
            sfx: self.prefs.sfx_volume,
            voice: self.prefs.voice_volume,
            muted: self.prefs.audio_muted,
        }
    }

    fn apply_audio_preferences(&mut self) {
        self.audio.set_mix(self.audio_mix_from_preferences());
    }

    fn execute_menu_action(&mut self, action: PlayerMenuAction, ctx: Option<&egui::Context>) {
        match action {
            PlayerMenuAction::ResumeGame => self.show_menu = false,
            PlayerMenuAction::OpenMenu => self.show_menu = true,
            PlayerMenuAction::QuickSave => self.quicksave(),
            PlayerMenuAction::QuickLoad => self.quickload(),
            PlayerMenuAction::OpenSaves => self.open_menu_tab(PlayerMenuTabKind::Saves),
            PlayerMenuAction::OpenHistory => self.open_menu_tab(PlayerMenuTabKind::History),
            PlayerMenuAction::OpenRoutes => self.open_menu_tab(PlayerMenuTabKind::Routes),
            PlayerMenuAction::OpenSettings => self.open_menu_tab(PlayerMenuTabKind::Settings),
            PlayerMenuAction::OpenSystem => self.open_menu_tab(PlayerMenuTabKind::System),
            PlayerMenuAction::ToggleHistoryWindow => self.show_history = !self.show_history,
            PlayerMenuAction::ToggleFullscreen => {
                self.prefs.fullscreen = !self.prefs.fullscreen;
                self.persist_preferences();
            }
            PlayerMenuAction::RestartStory => self.restart_story(),
            PlayerMenuAction::QuitGame => {
                if let Some(ctx) = ctx {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn open_menu_tab(&mut self, tab: PlayerMenuTabKind) {
        if self.config.player_menu.tab_label(tab).is_some() {
            self.menu_tab = tab;
        }
        self.show_menu = true;
    }

    fn apply_preferences(&mut self, ctx: &egui::Context) {
        let scale = (self.config.scale_factor * self.prefs.ui_scale).max(0.5);
        if (scale - self.applied_scale).abs() > f32::EPSILON {
            ctx.set_pixels_per_point(scale);
            self.applied_scale = scale;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.prefs.fullscreen));
        self.apply_audio_preferences();
    }

    fn current_save_data(&self) -> visual_novel_engine::SaveData {
        visual_novel_engine::SaveData::new(self.script_id, self.engine.state().clone())
    }

    fn quicksave(&mut self) {
        let data = self.current_save_data();
        match self.save_store.quicksave(&data) {
            Ok(entry) => {
                self.last_error = None;
                self.last_status = Some(format!("Quick saved: {}", save_slot_summary(&entry)));
            }
            Err(err) => self.last_error = Some(format!("Quick save failed: {err}")),
        }
    }

    fn quickload(&mut self) {
        match self.save_store.quickload() {
            Ok(data) => self.apply_loaded_save(data, "Quick loaded"),
            Err(err) => self.last_error = Some(format!("Quick load failed: {err}")),
        }
    }

    fn save_slot(&mut self, slot_id: u16) {
        let data = self.current_save_data();
        match self.save_store.save_slot(slot_id, &data) {
            Ok(entry) => {
                self.last_error = None;
                self.last_status = Some(format!("Saved: {}", save_slot_summary(&entry)));
            }
            Err(err) => self.last_error = Some(format!("Save slot {slot_id} failed: {err}")),
        }
    }

    fn load_slot(&mut self, slot_id: u16) {
        match self.save_store.load_slot(slot_id) {
            Ok(data) => self.apply_loaded_save(data, &format!("Loaded slot {slot_id}")),
            Err(err) => self.last_error = Some(format!("Load slot {slot_id} failed: {err}")),
        }
    }

    fn apply_loaded_save(&mut self, data: visual_novel_engine::SaveData, status: &str) {
        if let Err(err) = data.validate_script_id(&self.script_id) {
            self.last_error = Some(format!("Save data mismatch: {err}"));
            return;
        }
        match self.engine.set_state(data.state) {
            Ok(()) => {
                self.last_error = None;
                self.last_status = Some(status.to_string());
                let audio = self.engine.take_audio_commands();
                self.apply_audio_commands(audio);
            }
            Err(err) => self.last_error = Some(format!("Failed to load state: {err}")),
        }
    }

    fn list_save_slots_for_menu(&mut self) -> Vec<SaveSlotEntry> {
        match self.save_store.list_slots() {
            Ok(entries) => entries,
            Err(err) => {
                self.last_error = Some(format!("List saves failed: {err}"));
                Vec::new()
            }
        }
    }

    fn restart_story(&mut self) {
        match self.engine.jump_to_label("start") {
            Ok(()) => {
                self.engine.clear_session_history();
                let audio = self.engine.take_audio_commands();
                self.apply_audio_commands(audio);
                self.last_error = None;
                self.last_status = Some("Story restarted".to_string());
            }
            Err(err) => self.last_error = Some(format!("Restart failed: {err}")),
        }
    }

    #[allow(dead_code)]
    fn save_state(&mut self, path: &Path) {
        let data = visual_novel_engine::SaveData::new(self.script_id, self.engine.state().clone());
        match crate::persist::save_state_to(path, &data) {
            Ok(()) => {
                self.last_error = None;
                self.last_status = Some(format!("Saved: {}", path.display()));
            }
            Err(err) => self.last_error = Some(format!("Failed to save state: {err}")),
        }
    }

    #[allow(dead_code)]
    fn load_state(&mut self, path: &Path) {
        match crate::persist::load_state_from(path) {
            Ok(data) => {
                if let Err(err) = data.validate_script_id(&self.script_id) {
                    self.last_error = Some(format!("Save data mismatch: {err}"));
                    return;
                }
                if let Err(err) = self.engine.set_state(data.state) {
                    self.last_error = Some(format!("Failed to load state: {err}"));
                } else {
                    self.last_error = None;
                    self.last_status = Some(format!("Loaded: {}", path.display()));
                }
            }
            Err(err) => self.last_error = Some(format!("Failed to load state: {err}")),
        }
    }

    fn persist_preferences(&self) {
        if let Err(err) = self.prefs.save_to(&self.prefs_path) {
            eprintln!("Failed to save GUI preferences: {err}");
        }
    }
}

fn fit_rect_to_stage(bounds: egui::Rect, stage_size: (f32, f32)) -> egui::Rect {
    let size = player_stage_viewport_size(bounds.size(), stage_size);
    egui::Rect::from_center_size(bounds.center(), size)
}

fn cover_rect(bounds: egui::Rect, source_size: egui::Vec2) -> egui::Rect {
    let source_w = source_size.x.max(1.0);
    let source_h = source_size.y.max(1.0);
    let source_aspect = source_w / source_h;
    let bounds_aspect = bounds.width().max(1.0) / bounds.height().max(1.0);
    let size = if source_aspect > bounds_aspect {
        egui::vec2(bounds.height() * source_aspect, bounds.height())
    } else {
        egui::vec2(bounds.width(), bounds.width() / source_aspect)
    };
    egui::Rect::from_center_size(bounds.center(), size)
}

fn menu_action_button_width(action: PlayerMenuAction, label: &str) -> f32 {
    match action {
        PlayerMenuAction::ResumeGame => 132.0,
        PlayerMenuAction::QuitGame => 84.0,
        PlayerMenuAction::QuickSave
        | PlayerMenuAction::QuickLoad
        | PlayerMenuAction::OpenSaves
        | PlayerMenuAction::OpenRoutes
        | PlayerMenuAction::OpenSettings
        | PlayerMenuAction::OpenSystem => 104.0,
        _ => (label.chars().count() as f32 * 8.0 + 28.0).clamp(84.0, 150.0),
    }
}

fn player_save_root_for_script(prefs_path: &Path, script_id: &ScriptId) -> PathBuf {
    let base = prefs_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("saves").join(script_id_hex(script_id))
}

fn script_id_hex(script_id: &ScriptId) -> String {
    let mut out = String::with_capacity(script_id.len() * 2);
    for byte in script_id {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn save_slot_summary(entry: &SaveSlotEntry) -> String {
    let kind = if entry.metadata.quick {
        "Quick"
    } else {
        "Manual"
    };
    let line = entry
        .metadata
        .summary_line
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("No dialogue yet");
    format!("{kind} | Progress {} | {line}", entry.metadata.position)
}
