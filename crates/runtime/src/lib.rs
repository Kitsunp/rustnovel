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
    AudioCommand, Engine, EventCompiled, RenderOutput, TextRenderer, UiState, VisualState,
};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

pub use self::assets::{AssetStore, MemoryAssetStore};
pub use self::audio::{Audio, RodioBackend, SilentAudio};
pub use self::input::{ConfigurableInput, Input, InputAction};
use self::render::{BuiltinSoftwareDrawer, RenderBackend, SoftwareBackend, WgpuBackend};

// AssetStore and MemoryAssetStore moved to assets.rs

/// Runtime application wrapper. Logic controller.
pub struct RuntimeApp<I, A, S> {
    engine: Engine,
    visual: VisualState,
    input: I,
    audio: A,
    assets: S,
    ui: UiState,
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
        let mut app = Self {
            engine,
            visual,
            input,
            audio,
            assets,
            ui,
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
                let _ = self.engine.choose(index)?;
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
                    ..
                } => {
                    self.audio
                        .play_music_with_options(path.as_ref(), *r#loop, *volume);
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
        for path in self.engine.peek_next_asset_paths(self.prefetch_depth) {
            let _ = self.assets.load_bytes(&path);
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
    let event_loop = EventLoop::new().expect("failed to create event loop");
    #[allow(deprecated)]
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("VN Runtime")
            .with_inner_size(LogicalSize::new(960.0, 540.0))
            .with_min_inner_size(LogicalSize::new(640.0, 360.0))
            .build(&event_loop)
            .expect("failed to build runtime window"),
    );

    let size = window.inner_size();

    // Initialize Backend with Fallback
    let mut backend: Box<dyn RenderBackend> =
        match WgpuBackend::new(window.clone(), size.width, size.height) {
            Ok(backend) => {
                eprintln!("Using WGPU Hardware Backend");
                Box::new(backend)
            }
            Err(err) => {
                eprintln!(
                    "WGPU Backend initialization failed: {}. Falling back to Software Backend.",
                    err
                );
                Box::new(SoftwareBackend::new(
                    window.clone(),
                    size.width,
                    size.height,
                    Box::new(BuiltinSoftwareDrawer),
                ))
            }
        };

    event_loop
        .run(move |event, elwt| {
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        elwt.exit();
                    }
                    WindowEvent::Resized(size) => {
                        backend.resize(size.width, size.height);
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
        })
        .expect("event loop error");

    // The run function in 0.29 may return, but we treat this as a divergent function
    std::process::exit(0);
}
