//! Runtime layer for driving the engine with a winit + pixels loop.

pub mod assets;
pub mod audio;
pub mod input;
mod loader;
pub mod render;

pub use loader::{AsyncLoader, LoadRequest, LoadResult};

use std::sync::Arc;

// use pixels::{Pixels, SurfaceTexture}; // Removed unused imports
// Logic moved to software.rs
use visual_novel_engine::{
    runtime::{
        AudioCommand, Engine, EventCompiled, PrefetchMode, SceneFrame, UiState, VisualState,
    },
    RenderOutput, TextRenderer,
};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

pub use self::assets::{AssetStore, MemoryAssetStore};
pub use self::audio::{audio_duration, Audio, AudioCapabilities, RodioBackend, SilentAudio};
pub use self::input::{ConfigurableInput, Input, InputAction};
pub use self::render::RuntimeSceneFramePresenter;
use self::render::{BuiltinSoftwareDrawer, RenderBackend, SoftwareBackend, WgpuBackend};

// AssetStore and MemoryAssetStore moved to assets.rs

pub const RENDER_BACKEND_ENV: &str = "VNENGINE_RENDER_BACKEND";
pub const FORCE_WGPU_FAILURE_ENV: &str = "VNENGINE_FORCE_WGPU_FAILURE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeRenderBackendPreference {
    Auto,
    Software,
    Wgpu,
}

impl RuntimeRenderBackendPreference {
    pub fn from_env_value(value: Option<&str>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::Auto);
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Self::Auto),
            "software" | "pixels" => Ok(Self::Software),
            "wgpu" | "hardware" => Ok(Self::Wgpu),
            other => Err(format!(
                "unsupported {RENDER_BACKEND_ENV} value '{other}' (expected auto, software, or wgpu)"
            )),
        }
    }

    pub fn from_env() -> Self {
        match Self::from_env_value(std::env::var(RENDER_BACKEND_ENV).ok().as_deref()) {
            Ok(preference) => preference,
            Err(err) => {
                eprintln!("{err}; falling back to auto render backend selection");
                Self::Auto
            }
        }
    }
}

pub fn force_wgpu_failure_from_env_value(value: Option<&str>) -> bool {
    value
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub fn force_wgpu_failure_from_env() -> bool {
    force_wgpu_failure_from_env_value(std::env::var(FORCE_WGPU_FAILURE_ENV).ok().as_deref())
}

/// Runtime application wrapper. Logic controller.
pub struct RuntimeApp<I, A, S> {
    engine: Engine,
    visual: VisualState,
    input: I,
    audio: A,
    assets: S,
    ui: UiState,
    scene_frame: SceneFrame,
    last_bgm_path: Option<String>,
    prefetch_depth: usize,
}

impl<I, A, S> RuntimeApp<I, A, S>
where
    I: Input,
    A: Audio,
    S: AssetStore,
{
    const DEFAULT_PREFETCH_DEPTH: usize = 3;

    pub fn new(
        engine: Engine,
        input: I,
        audio: A,
        assets: S,
    ) -> visual_novel_engine::VnResult<Self> {
        let event = engine.current_event()?;
        let visual = Self::derive_visual(&engine, &event);
        let ui = UiState::from_event(&event, &visual);
        let scene_frame = engine.scene_frame();
        let mut app = Self {
            engine,
            visual,
            input,
            audio,
            assets,
            ui,
            scene_frame,
            last_bgm_path: None,
            prefetch_depth: Self::DEFAULT_PREFETCH_DEPTH,
        };
        let audio_commands = app.engine.take_audio_commands();
        app.apply_audio_commands(&audio_commands);
        app.prefetch_upcoming_assets();
        Ok(app)
    }

    /// Creates a new RuntimeApp trying to use RodioBackend (if available), falling back to SilentAudio.
    pub fn new_auto(
        engine: Engine,
        input: I,
        assets: Arc<S>,
    ) -> visual_novel_engine::VnResult<RuntimeApp<I, Box<dyn Audio>, Arc<S>>>
    where
        S: AssetStore + Send + Sync + 'static,
    {
        let audio: Box<dyn Audio> = match RodioBackend::new(assets.clone()) {
            Ok(backend) => {
                eprintln!("Audio: Using Rodio Backend");
                Box::new(backend)
            }
            Err(e) => {
                eprintln!(
                    "Audio: Rodio initialization failed ({}), using SilentAudio",
                    e
                );
                Box::new(SilentAudio)
            }
        };

        RuntimeApp::new(engine, input, audio, assets)
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn ui(&self) -> &UiState {
        &self.ui
    }

    pub fn scene_frame(&self) -> &SceneFrame {
        &self.scene_frame
    }

    pub fn prefetch_depth(&self) -> usize {
        self.prefetch_depth
    }

    pub fn set_prefetch_depth(&mut self, depth: usize) {
        self.prefetch_depth = depth;
        self.prefetch_upcoming_assets();
    }

    pub fn handle_action(&mut self, action: InputAction) -> visual_novel_engine::VnResult<bool> {
        match action {
            InputAction::None => {}
            InputAction::Quit => return Ok(false),
            InputAction::Advance => {
                let audio_commands = step_or_resume(&mut self.engine)?;
                self.refresh_state()?;
                self.apply_audio_commands(&audio_commands);
                self.prefetch_upcoming_assets();
            }
            InputAction::Choose(index) => {
                self.engine.choose(index)?;
                let audio_commands = self.engine.take_audio_commands();
                self.refresh_state()?;
                self.apply_audio_commands(&audio_commands);
                self.prefetch_upcoming_assets();
            }
            InputAction::Back | InputAction::Menu => {
                // Action recognized but currently non-mutating in runtime mode.
            }
        }
        Ok(true)
    }

    fn refresh_state(&mut self) -> visual_novel_engine::VnResult<()> {
        let event = self.engine.current_event()?;
        self.visual = Self::derive_visual(&self.engine, &event);
        self.ui = UiState::from_event(&event, &self.visual);
        self.scene_frame = self.engine.scene_frame();
        Ok(())
    }

    fn derive_visual(engine: &Engine, event: &EventCompiled) -> VisualState {
        let mut visual = engine.visual_state().clone();
        if let EventCompiled::Scene(scene) = event {
            visual.apply_scene(scene);
        }
        visual
    }

    fn apply_audio_commands(&mut self, commands: &[AudioCommand]) {
        for command in commands {
            match command {
                AudioCommand::PlayBgm {
                    path,
                    r#loop,
                    volume,
                    fade_in,
                    ..
                } => {
                    self.audio.play_music_with_transition(
                        path.as_ref(),
                        *r#loop,
                        *volume,
                        Some(*fade_in),
                    );
                    self.last_bgm_path = Some(path.as_ref().to_string());
                }
                AudioCommand::StopBgm { fade_out } => {
                    self.audio.stop_music_with_fade(Some(*fade_out));
                    self.last_bgm_path = None;
                }
                AudioCommand::PlaySfx { path, volume, .. } => {
                    self.audio.play_sfx_with_volume(path.as_ref(), *volume);
                }
                AudioCommand::StopSfx => {
                    self.audio.stop_sfx();
                }
                AudioCommand::PlayVoice { path, volume, .. } => {
                    self.audio.play_voice_with_volume(path.as_ref(), *volume);
                }
                AudioCommand::StopVoice => {
                    self.audio.stop_voice();
                }
            }
        }
    }

    fn prefetch_upcoming_assets(&mut self) {
        if self.prefetch_depth == 0 {
            return;
        }
        for path in self
            .engine
            .peek_next_asset_paths_with_mode(PrefetchMode::BranchAware {
                depth: self.prefetch_depth,
                max_assets: self.prefetch_depth.saturating_mul(8).max(8),
            })
        {
            if let Err(err) = self.assets.load_bytes(&path) {
                eprintln!("prefetch failed for '{path}': {err}");
            }
        }
    }

    pub fn render_text(&self) -> visual_novel_engine::VnResult<RenderOutput> {
        let renderer = TextRenderer;
        self.engine.render_current(&renderer)
    }

    pub fn assets(&self) -> &S {
        &self.assets
    }
}

fn step_or_resume(engine: &mut Engine) -> visual_novel_engine::VnResult<Vec<AudioCommand>> {
    if matches!(engine.current_event()?, EventCompiled::ExtCall { .. }) {
        engine.resume()?;
        Ok(engine.take_audio_commands())
    } else {
        let (audio_commands, _) = engine.step()?;
        Ok(audio_commands)
    }
}

/// Run the runtime loop using winit and a rendering backend (hybrid: wgpu or software).
pub fn run_winit<I, A, S>(mut app: RuntimeApp<I, A, S>) -> !
where
    I: Input + 'static,
    A: Audio + 'static,
    S: AssetStore + 'static,
{
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(err) => {
            eprintln!("failed to create event loop: {err}");
            std::process::exit(1);
        }
    };
    #[allow(deprecated)]
    let window = match WindowBuilder::new()
        .with_title("VN Runtime")
        .with_inner_size(LogicalSize::new(960.0, 540.0))
        .with_min_inner_size(LogicalSize::new(640.0, 360.0))
        .build(&event_loop)
    {
        Ok(window) => Arc::new(window),
        Err(err) => {
            eprintln!("failed to build runtime window: {err}");
            std::process::exit(1);
        }
    };

    let size = window.inner_size();

    let backend_preference = RuntimeRenderBackendPreference::from_env();
    let mut backend: Box<dyn RenderBackend> = match backend_preference {
        RuntimeRenderBackendPreference::Software => {
            eprintln!("Using Software Backend ({RENDER_BACKEND_ENV}=software)");
            let software = match SoftwareBackend::try_new(
                window.clone(),
                size.width,
                size.height,
                Box::new(BuiltinSoftwareDrawer),
            ) {
                Ok(backend) => backend,
                Err(err) => {
                    eprintln!("Software Backend initialization failed: {err}");
                    std::process::exit(1);
                }
            };
            Box::new(software)
        }
        RuntimeRenderBackendPreference::Auto | RuntimeRenderBackendPreference::Wgpu => {
            let wgpu_result = if force_wgpu_failure_from_env() {
                Err(format!("forced by {FORCE_WGPU_FAILURE_ENV}"))
            } else {
                WgpuBackend::new(window.clone(), size.width, size.height)
            };
            match wgpu_result {
                Ok(backend) => {
                    eprintln!("Using WGPU Hardware Backend");
                    Box::new(backend)
                }
                Err(err) => {
                    if backend_preference == RuntimeRenderBackendPreference::Wgpu {
                        eprintln!("WGPU Backend initialization failed: {err}");
                        std::process::exit(1);
                    }
                    eprintln!(
                        "WGPU Backend initialization failed: {}. Falling back to Software Backend.",
                        err
                    );
                    let software = match SoftwareBackend::try_new(
                        window.clone(),
                        size.width,
                        size.height,
                        Box::new(BuiltinSoftwareDrawer),
                    ) {
                        Ok(backend) => backend,
                        Err(err) => {
                            eprintln!("Software Backend initialization failed: {err}");
                            std::process::exit(1);
                        }
                    };
                    Box::new(software)
                }
            }
        }
    };

    if let Err(err) = event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    elwt.exit();
                }
                WindowEvent::Resized(size) => {
                    if let Err(e) = backend.resize(size.width, size.height) {
                        eprintln!("Resize error: {}", e);
                        elwt.exit();
                    }
                }
                WindowEvent::RedrawRequested => {
                    if let Err(e) = backend.render(app.ui()) {
                        eprintln!("Render error: {}", e);
                        elwt.exit();
                    }
                }
                _ => {
                    let action = app.input.handle_window_event(&event);
                    match app.handle_action(action) {
                        Ok(true) => {
                            window.request_redraw();
                        }
                        Ok(false) => {
                            elwt.exit();
                        }
                        Err(_) => {
                            elwt.exit();
                        }
                    }
                }
            },
            Event::AboutToWait => {
                // window.request_redraw();
            }
            _ => {}
        }
    }) {
        eprintln!("event loop error: {err}");
        std::process::exit(1);
    }

    // The run function in 0.29 may return, but we treat this as a divergent function
    std::process::exit(0);
}
