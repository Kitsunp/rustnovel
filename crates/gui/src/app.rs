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
#[path = "app/player_helpers.rs"]
mod player_helpers;
#[path = "app/player_render.rs"]
mod player_render;
#[path = "app/player_state.rs"]
mod player_state;
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
    let preferences = load_user_preferences_for_run(&preferences_path)?;
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

fn load_user_preferences_for_run(path: &Path) -> Result<UserPreferences, GuiError> {
    UserPreferences::load_from(path).map_err(|err| {
        std::io::Error::new(
            err.kind(),
            format!("load preferences '{}': {err}", path.display()),
        )
        .into()
    })
}

use player_helpers::{
    cover_rect, fit_rect_to_stage, player_save_root_for_script, save_slot_summary,
};
pub use player_helpers::{
    player_event_requires_input, player_menu_action_button_size, player_menu_action_enabled,
    player_menu_action_text, player_menu_color, player_menu_content_height,
    player_menu_reference_viewport, player_menu_text_button_size, player_menu_window_height,
    player_menu_window_width, player_route_history_label, player_stage_available_size,
    player_stage_viewport_size, player_status_bar_height, player_text_panel_advance_enabled,
};

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
}

#[cfg(test)]
#[path = "app/tests.rs"]
mod tests;
