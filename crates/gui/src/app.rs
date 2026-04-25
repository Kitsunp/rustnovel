use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use eframe::egui;
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use visual_novel_engine::{
    compute_script_id, Engine, ResourceLimiter, ScriptId, ScriptRaw, SecurityPolicy, UiState,
    UiView, VnError,
};

use crate::assets::{AssetManager, AssetStore, SecurityMode};
use crate::persist::{load_state_from, save_state_to, PersistError, UserPreferences};
use crate::widgets::{event_kind, history_bytes};

#[path = "app/eframe_impl.rs"]
mod eframe_impl;

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
        }
    }

    pub fn preferences_path(&self) -> PathBuf {
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
    show_settings: bool,
    show_history: bool,
    show_inspector: bool,
    last_error: Option<String>,
    assets: AssetManager,
    applied_scale: f32,
    label_jump_input: String,
    script_id: ScriptId,
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
        let mut app = Self {
            engine,
            config,
            prefs,
            prefs_path,
            show_settings: false,
            show_history: false,
            show_inspector: false,
            last_error: None,
            assets,
            applied_scale: 0.0,
            label_jump_input: String::new(),
            script_id,
        };
        let scale = app.config.scale_factor * app.prefs.ui_scale;
        cc.egui_ctx.set_pixels_per_point(scale.max(0.5));
        app.applied_scale = scale;
        app
    }

    fn render_scene(&mut self, ui: &mut egui::Ui) {
        let visual = self.engine.visual_state();
        ui.group(|ui| {
            ui.heading("Scene");
            if let Some(background) = visual.background.as_deref() {
                match self.assets.texture_for_asset(ui.ctx(), background) {
                    Ok(Some(texture)) => {
                        let size = ui.available_width();
                        let ratio = texture.size()[1] as f32 / texture.size()[0].max(1) as f32;
                        ui.add(
                            egui::Image::from_texture(&texture)
                                .fit_to_exact_size(egui::Vec2::new(size, size * ratio)),
                        );
                    }
                    Ok(None) => {}
                    Err(err) => self.last_error = Some(format!("Asset error: {err}")),
                }
            }
            ui.label(format!(
                "Music: {}",
                visual
                    .music
                    .as_deref()
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "None".to_string())
            ));
            ui.separator();
            for character in &visual.characters {
                ui.label(format!(
                    "Character: {} ({:?})",
                    character.name,
                    character.expression.as_deref()
                ));
            }
        });
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        let view = self
            .engine
            .current_event()
            .map(|event| UiState::from_event(&event, self.engine.visual_state()).view);
        let view = match view {
            Ok(view) => view,
            Err(err) => {
                ui.label(format!("Error: {err}"));
                return;
            }
        };
        ui.group(|ui| match view {
            UiView::Dialogue { speaker, text } => {
                ui.heading(speaker);
                ui.label(text);
                if ui.button("History").clicked() {
                    self.show_history = !self.show_history;
                }
                if ui.button("Continue").clicked() {
                    self.advance();
                }
            }
            UiView::Choice { prompt, options } => {
                ui.heading(prompt);
                for (idx, option) in options.into_iter().enumerate() {
                    if ui.button(option).clicked() {
                        self.choose(idx);
                    }
                }
                if ui.button("History").clicked() {
                    self.show_history = !self.show_history;
                }
            }
            UiView::Scene { description } => {
                ui.label(description);
                if ui.button("History").clicked() {
                    self.show_history = !self.show_history;
                }
                if ui.button("Continue").clicked() {
                    self.advance();
                }
            }
            UiView::System { message } => {
                ui.label(message);
                if ui.button("History").clicked() {
                    self.show_history = !self.show_history;
                }
                if ui.button("Continue").clicked() {
                    self.advance();
                }
            }
        });
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
            Ok((_audio, _change)) => {}
            Err(VnError::EndOfScript) => {}
            Err(err) => self.last_error = Some(err.to_string()),
        }
    }

    fn choose(&mut self, index: usize) {
        match self.engine.choose(index) {
            Ok(_) => {}
            Err(VnError::EndOfScript) => {}
            Err(err) => self.last_error = Some(err.to_string()),
        }
    }

    fn apply_preferences(&mut self, ctx: &egui::Context) {
        let scale = (self.config.scale_factor * self.prefs.ui_scale).max(0.5);
        if (scale - self.applied_scale).abs() > f32::EPSILON {
            ctx.set_pixels_per_point(scale);
            self.applied_scale = scale;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.prefs.fullscreen));
    }

    fn save_state(&mut self, path: &Path) {
        let data = visual_novel_engine::SaveData::new(self.script_id, self.engine.state().clone());
        if let Err(err) = save_state_to(path, &data) {
            self.last_error = Some(format!("Failed to save state: {err}"));
        }
    }

    fn load_state(&mut self, path: &Path) {
        match load_state_from(path) {
            Ok(data) => {
                if let Err(err) = data.validate_script_id(&self.script_id) {
                    self.last_error = Some(format!("Save data mismatch: {err}"));
                    return;
                }
                if let Err(err) = self.engine.set_state(data.state) {
                    self.last_error = Some(format!("Failed to load state: {err}"));
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
