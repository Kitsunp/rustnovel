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
        AudioCommand, Engine, ExternalCallOutcome, PrefetchMode, SceneFrame, UiState, UiView,
    },
    LayoutRect, RenderCommand, RenderOutput, TextRenderer,
};
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

pub use self::assets::{AssetStore, MemoryAssetStore};
pub use self::audio::{
    audio_duration, audio_duration_from_bytes, Audio, AudioCapabilities, AudioResult,
    MusicPlayback, RodioBackend, SilentAudio,
};
pub use self::input::{pointer_action_for_scene_frame, ConfigurableInput, Input, InputAction};
pub use self::render::RuntimeSceneFramePresenter;
use self::render::{
    BuiltinSoftwareDrawer, RenderBackend, RenderFrame, SoftwareBackend, WgpuBackend,
};

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
        Self::from_env_with_warning().0
    }

    fn from_env_with_warning() -> (Self, Option<String>) {
        let preference = match std::env::var(RENDER_BACKEND_ENV) {
            Ok(value) => Self::from_env_value(Some(&value)),
            Err(std::env::VarError::NotPresent) => Ok(Self::Auto),
            Err(std::env::VarError::NotUnicode(_)) => {
                Err(format!("{RENDER_BACKEND_ENV} must be valid UTF-8"))
            }
        };
        match preference {
            Ok(preference) => (preference, None),
            Err(error) => {
                let warning = format!("{error}. Using automatic render backend selection.");
                eprintln!("{warning}");
                (Self::Auto, Some(warning))
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

/// A failed prefetch from the latest prefetch pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefetchDiagnostic {
    pub path: String,
    pub error: String,
}

/// Runtime application wrapper. Logic controller.
pub struct RuntimeApp<I, A, S> {
    engine: Engine,
    input: I,
    audio: A,
    assets: S,
    ui: UiState,
    scene_frame: SceneFrame,
    last_bgm_path: Option<String>,
    prefetch_depth: usize,
    audio_fallback_reason: Option<String>,
    prefetch_diagnostics: Vec<PrefetchDiagnostic>,
    prefetch_diagnostics_omitted: usize,
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
        Self::new_with_audio_fallback(engine, input, audio, assets, None)
    }

    fn new_with_audio_fallback(
        engine: Engine,
        input: I,
        audio: A,
        assets: S,
        audio_fallback_reason: Option<String>,
    ) -> visual_novel_engine::VnResult<Self> {
        let event = engine.current_event_ref()?;
        let scene_frame = engine.scene_frame();
        let ui = UiState::from_scene_frame(event, &scene_frame);
        let mut app = Self {
            engine,
            input,
            audio,
            assets,
            ui,
            scene_frame,
            last_bgm_path: None,
            prefetch_depth: Self::DEFAULT_PREFETCH_DEPTH,
            audio_fallback_reason,
            prefetch_diagnostics: Vec::new(),
            prefetch_diagnostics_omitted: 0,
        };
        let audio_commands = app.engine.take_audio_commands();
        app.apply_audio_commands(&audio_commands)?;
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
        Self::new_auto_with_factory(engine, input, assets, |assets| {
            RodioBackend::new(assets).map(|backend| Box::new(backend) as Box<dyn Audio>)
        })
    }

    fn new_auto_with_factory<F>(
        engine: Engine,
        input: I,
        assets: Arc<S>,
        factory: F,
    ) -> visual_novel_engine::VnResult<RuntimeApp<I, Box<dyn Audio>, Arc<S>>>
    where
        S: AssetStore + Send + Sync + 'static,
        F: FnOnce(Arc<S>) -> Result<Box<dyn Audio>, String>,
    {
        let (audio, fallback): (Box<dyn Audio>, Option<String>) = match factory(assets.clone()) {
            Ok(backend) => {
                eprintln!("Audio: Using Rodio Backend");
                (backend, None)
            }
            Err(e) => {
                eprintln!(
                    "Audio: Rodio initialization failed ({}), using SilentAudio",
                    e
                );
                (Box::new(SilentAudio), Some(e))
            }
        };

        RuntimeApp::new_with_audio_fallback(engine, input, audio, assets, fallback)
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

    /// The initialization error when automatic audio selection fell back to silence.
    pub fn audio_fallback_reason(&self) -> Option<&str> {
        self.audio_fallback_reason.as_deref()
    }

    /// Up to 32 failed paths from the latest prefetch pass. Long fields are truncated.
    pub fn prefetch_diagnostics(&self) -> &[PrefetchDiagnostic] {
        &self.prefetch_diagnostics
    }

    /// Number of additional failed paths omitted from the latest pass.
    pub fn prefetch_diagnostics_omitted(&self) -> usize {
        self.prefetch_diagnostics_omitted
    }

    pub fn set_prefetch_depth(&mut self, depth: usize) {
        self.prefetch_depth = depth;
        self.prefetch_upcoming_assets();
    }

    pub fn handle_action(&mut self, action: InputAction) -> visual_novel_engine::VnResult<bool> {
        match action {
            InputAction::None => {}
            InputAction::InvalidSceneAction => {
                return Err(visual_novel_engine::VnError::InvalidScript(
                    "scene-frame interaction action is unsupported or malformed".to_string(),
                ));
            }
            InputAction::Quit => return Ok(false),
            InputAction::Advance => {
                let audio_commands = match step_or_resume(&mut self.engine) {
                    Ok(commands) => commands,
                    Err(visual_novel_engine::VnError::EndOfScript) => return Ok(false),
                    Err(error) => return Err(error),
                };
                self.finish_transition(&audio_commands)?;
            }
            InputAction::Choose(index) => {
                self.engine.choose(index)?;
                let audio_commands = self.engine.take_audio_commands();
                self.finish_transition(&audio_commands)?;
            }
            InputAction::Back => {
                return Err(visual_novel_engine::VnError::InvalidScript(
                    "input action 'back' is recognized but not implemented in runtime mode"
                        .to_string(),
                ));
            }
            InputAction::Menu => {
                return Err(visual_novel_engine::VnError::InvalidScript(
                    "input action 'menu' is recognized but not implemented in runtime mode"
                        .to_string(),
                ));
            }
        }
        Ok(true)
    }

    pub fn complete_external_call(
        &mut self,
        outcome: ExternalCallOutcome,
    ) -> visual_novel_engine::VnResult<()> {
        self.engine.complete_external_call(outcome)?;
        let audio_commands = self.engine.take_audio_commands();
        self.finish_transition(&audio_commands)
    }

    fn finish_transition(
        &mut self,
        audio_commands: &[AudioCommand],
    ) -> visual_novel_engine::VnResult<()> {
        let scene_frame = self.engine.scene_frame();
        self.ui = match self.engine.current_event_ref() {
            Ok(event) => UiState::from_scene_frame(event, &scene_frame),
            Err(visual_novel_engine::VnError::EndOfScript) => UiState {
                view: UiView::System {
                    message: "End of script".to_string(),
                },
                pending_transition: None,
            },
            Err(err) => return Err(err),
        };
        self.scene_frame = scene_frame;
        self.apply_audio_commands(audio_commands)?;
        self.prefetch_upcoming_assets();
        Ok(())
    }

    fn apply_audio_commands(
        &mut self,
        commands: &[AudioCommand],
    ) -> visual_novel_engine::VnResult<()> {
        for command in commands {
            match command {
                AudioCommand::PlayBgm {
                    path,
                    r#loop,
                    volume,
                    fade_in,
                    ..
                } => {
                    self.audio
                        .play_music_with_transition(path.as_ref(), *r#loop, *volume, Some(*fade_in))
                        .map_err(|err| {
                            audio_command_error(command, err, self.audio_fallback_reason.as_deref())
                        })?;
                    self.last_bgm_path = Some(path.as_ref().to_string());
                }
                AudioCommand::StopBgm { fade_out } => {
                    self.audio
                        .stop_music_with_fade(Some(*fade_out))
                        .map_err(|err| {
                            audio_command_error(command, err, self.audio_fallback_reason.as_deref())
                        })?;
                    self.last_bgm_path = None;
                }
                AudioCommand::PlaySfx { path, volume, .. } => {
                    self.audio
                        .play_sfx_with_volume(path.as_ref(), *volume)
                        .map_err(|err| {
                            audio_command_error(command, err, self.audio_fallback_reason.as_deref())
                        })?;
                }
                AudioCommand::StopSfx => {
                    self.audio.stop_sfx().map_err(|err| {
                        audio_command_error(command, err, self.audio_fallback_reason.as_deref())
                    })?;
                }
                AudioCommand::PlayVoice { path, volume, .. } => {
                    self.audio
                        .play_voice_with_volume(path.as_ref(), *volume)
                        .map_err(|err| {
                            audio_command_error(command, err, self.audio_fallback_reason.as_deref())
                        })?;
                }
                AudioCommand::StopVoice => {
                    self.audio.stop_voice().map_err(|err| {
                        audio_command_error(command, err, self.audio_fallback_reason.as_deref())
                    })?;
                }
            }
        }
        Ok(())
    }

    fn prefetch_upcoming_assets(&mut self) {
        const MAX_DIAGNOSTICS: usize = 32;
        let mut diagnostics = Vec::new();
        let mut omitted = 0usize;
        if self.prefetch_depth == 0 {
            self.prefetch_diagnostics.clear();
            self.prefetch_diagnostics_omitted = 0;
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
                if diagnostics.len() < MAX_DIAGNOSTICS {
                    diagnostics.push(PrefetchDiagnostic {
                        path: bounded_diagnostic_text(&path),
                        error: bounded_diagnostic_text(&err),
                    });
                } else {
                    omitted = omitted.saturating_add(1);
                }
            }
        }
        if diagnostics != self.prefetch_diagnostics || omitted != self.prefetch_diagnostics_omitted
        {
            for diagnostic in &diagnostics {
                if !self.prefetch_diagnostics.contains(diagnostic) {
                    eprintln!(
                        "prefetch failed for '{}': {}",
                        diagnostic.path, diagnostic.error
                    );
                }
            }
            if omitted > 0 && omitted != self.prefetch_diagnostics_omitted {
                eprintln!("{omitted} additional prefetch failures omitted");
            }
        }
        self.prefetch_diagnostics = diagnostics;
        self.prefetch_diagnostics_omitted = omitted;
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
    if let Some(request) = engine.pending_external_call()? {
        return Err(visual_novel_engine::VnError::external_call_pending(
            request.event_ip,
            request.command,
        ));
    }
    let (audio_commands, _) = engine.step()?;
    Ok(audio_commands)
}

fn bounded_diagnostic_text(value: &str) -> String {
    const LIMIT: usize = 1024;
    if value.len() <= LIMIT {
        return value.to_owned();
    }
    let mut end = LIMIT;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

fn audio_command_error(
    command: &AudioCommand,
    err: String,
    fallback: Option<&str>,
) -> visual_novel_engine::VnError {
    let fallback = fallback
        .map(|reason| format!("; Rodio initialization failed: {reason}"))
        .unwrap_or_default();
    visual_novel_engine::VnError::invalid_script(format!(
        "audio command '{}' failed: {err}{fallback}",
        audio_command_trace(command)
    ))
}

fn audio_command_trace(command: &AudioCommand) -> String {
    match command {
        AudioCommand::PlayBgm {
            path,
            r#loop,
            volume,
            fade_in,
            ..
        } => format!(
            "play_bgm path={} loop={} volume={:?} fade_in_ms={}",
            path.as_ref(),
            r#loop,
            volume,
            fade_in.as_millis()
        ),
        AudioCommand::StopBgm { fade_out } => {
            format!("stop_bgm fade_out_ms={}", fade_out.as_millis())
        }
        AudioCommand::PlaySfx { path, volume, .. } => {
            format!("play_sfx path={} volume={:?}", path.as_ref(), volume)
        }
        AudioCommand::StopSfx => "stop_sfx".to_string(),
        AudioCommand::PlayVoice { path, volume, .. } => {
            format!("play_voice path={} volume={:?}", path.as_ref(), volume)
        }
        AudioCommand::StopVoice => "stop_voice".to_string(),
    }
}

/// Runs the runtime loop as a process entry point.
pub fn run_winit<I, A, S>(app: RuntimeApp<I, A, S>) -> !
where
    I: Input + 'static,
    A: Audio + 'static,
    S: AssetStore + 'static,
{
    let status = match try_run_winit(app) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    };
    std::process::exit(status);
}

/// Runs the runtime loop and returns the cause of any startup or loop failure.
pub fn try_run_winit<I, A, S>(mut app: RuntimeApp<I, A, S>) -> Result<(), String>
where
    I: Input + 'static,
    A: Audio + 'static,
    S: AssetStore + 'static,
{
    let event_loop =
        EventLoop::new().map_err(|err| format!("failed to create event loop: {err}"))?;
    #[allow(deprecated)]
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("VN Runtime")
            .with_inner_size(LogicalSize::new(960.0, 540.0))
            .with_min_inner_size(LogicalSize::new(640.0, 360.0))
            .build(&event_loop)
            .map_err(|err| format!("failed to build runtime window: {err}"))?,
    );
    let (preference, config_warning) = RuntimeRenderBackendPreference::from_env_with_warning();
    let (mut backend, backend_warning) = create_winit_backend(&window, preference)?;
    let startup_warnings = [config_warning, backend_warning];
    let mut frame_with_warnings = frame_with_runtime_warnings(&app, &startup_warnings);

    let mut failure = None;
    let mut cursor_position: Option<(f64, f64)> = None;

    let loop_result = event_loop.run(|event, elwt| {
        if failure.is_some() {
            return;
        }
        let result = match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => Ok(false),
                WindowEvent::Resized(size) => backend
                    .resize(size.width, size.height)
                    .map(|()| true)
                    .map_err(|err| format!("resize error: {err}")),
                WindowEvent::RedrawRequested => backend
                    .render(RenderFrame {
                        ui: app.ui(),
                        scene_frame: frame_with_warnings
                            .as_ref()
                            .unwrap_or_else(|| app.scene_frame()),
                        assets: app.assets(),
                    })
                    .map(|()| true)
                    .map_err(|err| format!("render error: {err}")),
                _ => {
                    let action = match &event {
                        WindowEvent::CursorMoved { position, .. } => {
                            cursor_position = Some((position.x, position.y));
                            InputAction::None
                        }
                        WindowEvent::MouseInput {
                            state: ElementState::Pressed,
                            button: MouseButton::Left,
                            ..
                        } => {
                            let size = window.inner_size();
                            cursor_position
                                .map(|(x, y)| {
                                    pointer_action_for_scene_frame(
                                        app.scene_frame(),
                                        (size.width, size.height),
                                        x,
                                        y,
                                    )
                                })
                                .filter(|action| *action != InputAction::None)
                                .unwrap_or_else(|| app.input.handle_window_event(&event))
                        }
                        _ => app.input.handle_window_event(&event),
                    };
                    let changes_frame = !matches!(action, InputAction::None);
                    app.handle_action(action)
                        .inspect(|&keep_running| {
                            if keep_running {
                                if changes_frame {
                                    frame_with_warnings =
                                        frame_with_runtime_warnings(&app, &startup_warnings);
                                }
                                window.request_redraw();
                            }
                        })
                        .map_err(|err| format!("input action error: {err}"))
                }
            },
            Event::AboutToWait => {
                window.request_redraw();
                Ok(true)
            }
            _ => Ok(true),
        };
        match result {
            Ok(true) => {}
            Ok(false) => elwt.exit(),
            Err(error) => {
                failure = Some(error);
                elwt.exit();
            }
        }
    });
    match (failure, loop_result) {
        (None, Ok(())) => Ok(()),
        (Some(error), Ok(())) => Err(error),
        (None, Err(error)) => Err(format!("event loop error: {error}")),
        (Some(error), Err(loop_error)) => Err(format!("{error}. Event loop error: {loop_error}")),
    }
}

fn frame_with_runtime_warnings<I, A, S>(
    app: &RuntimeApp<I, A, S>,
    startup_warnings: &[Option<String>],
) -> Option<SceneFrame>
where
    I: Input,
    A: Audio,
    S: AssetStore,
{
    let mut warnings = startup_warnings
        .iter()
        .flatten()
        .map(|warning| bounded_diagnostic_text(warning))
        .collect::<Vec<_>>();
    if let Some(reason) = app.audio_fallback_reason() {
        warnings.push(format!("Audio unavailable; running silent: {reason}"));
    }
    for diagnostic in app.prefetch_diagnostics().iter().take(3) {
        warnings.push(format!(
            "Prefetch failed for '{}': {}",
            diagnostic.path, diagnostic.error
        ));
    }
    let remaining = app
        .prefetch_diagnostics()
        .len()
        .saturating_sub(3)
        .saturating_add(app.prefetch_diagnostics_omitted());
    if remaining > 0 {
        warnings.push(format!("{remaining} additional prefetch failures"));
    }
    if warnings.is_empty() {
        return None;
    }
    let mut frame = app.scene_frame().clone();
    let first_warning_y = frame
        .commands
        .iter()
        .filter_map(|command| match command {
            RenderCommand::Text { style, rect, .. } if style == "system.warning" => {
                Some(rect.y + rect.height)
            }
            _ => None,
        })
        .fold(32.0, f32::max);
    for (index, text) in warnings.into_iter().enumerate() {
        frame.commands.push(RenderCommand::Text {
            text,
            style: "system.warning".to_string(),
            rect: LayoutRect {
                x: 32.0,
                y: first_warning_y + index as f32 * 48.0,
                width: 1216.0,
                height: 48.0,
            },
        });
    }
    Some(frame)
}

fn create_winit_backend(
    window: &Arc<winit::window::Window>,
    preference: RuntimeRenderBackendPreference,
) -> Result<(Box<dyn RenderBackend>, Option<String>), String> {
    let size = window.inner_size();
    let hardware_error = if preference == RuntimeRenderBackendPreference::Software {
        eprintln!("Using Software Backend ({RENDER_BACKEND_ENV}=software)");
        None
    } else {
        let hardware = if force_wgpu_failure_from_env() {
            Err(format!("forced by {FORCE_WGPU_FAILURE_ENV}"))
        } else {
            WgpuBackend::new(window.clone(), size.width, size.height)
        };
        match hardware {
            Ok(backend) => {
                eprintln!("Using WGPU Hardware Backend");
                return Ok((Box::new(backend), None));
            }
            Err(error) if preference == RuntimeRenderBackendPreference::Wgpu => {
                return Err(format!("WGPU Backend initialization failed: {error}"));
            }
            Err(error) => Some(error),
        }
    };

    let software = SoftwareBackend::try_new(
        window.clone(),
        size.width,
        size.height,
        Box::new(BuiltinSoftwareDrawer::new()),
    )
    .map_err(|error| match &hardware_error {
        Some(hardware) => format!(
            "WGPU Backend initialization failed: {hardware}. Software Backend initialization failed: {error}"
        ),
        None => format!("Software Backend initialization failed: {error}"),
    })?;
    let warning = hardware_error.map(|error| {
        let warning =
            format!("WGPU Backend initialization failed: {error}. Using Software Backend.");
        eprintln!("{warning}");
        warning
    });
    Ok((Box::new(software), warning))
}

#[cfg(test)]
mod diagnostics_tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use visual_novel_engine::runtime::{DialogueRaw, EventRaw, SceneUpdateRaw, ScriptRaw};
    use visual_novel_engine::{AssetId, ResourceLimiter, SecurityPolicy};

    struct NoInput;
    impl Input for NoInput {
        fn handle_window_event(&mut self, _: &WindowEvent) -> InputAction {
            InputAction::None
        }
    }

    struct ToggleAssets(Arc<AtomicBool>);
    impl AssetStore for ToggleAssets {
        fn load_bytes(&self, _: &str) -> Result<Vec<u8>, String> {
            if self.0.load(Ordering::Relaxed) {
                Ok(vec![1])
            } else {
                Err("storage offline".into())
            }
        }
    }

    fn engine(events: Vec<EventRaw>) -> Engine {
        Engine::new(
            ScriptRaw::new(events, BTreeMap::from([("start".into(), 0)])),
            SecurityPolicy::default(),
            ResourceLimiter::default(),
        )
        .unwrap()
    }

    fn dialogue() -> EventRaw {
        EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".into(),
            text: "hello".into(),
        })
    }

    #[test]
    fn auto_audio_exposes_cause_and_keeps_initial_command_failure() {
        let assets = Arc::new(MemoryAssetStore::default());
        let mut app = RuntimeApp::<NoInput, SilentAudio, MemoryAssetStore>::new_auto_with_factory(
            engine(vec![dialogue()]),
            NoInput,
            assets.clone(),
            |_| Err("device missing".into()),
        )
        .expect("silent app");
        assert_eq!(app.audio_fallback_reason(), Some("device missing"));
        app.scene_frame.commands.push(RenderCommand::Text {
            text: "Engine warning".to_string(),
            style: "system.warning".to_string(),
            rect: LayoutRect {
                x: 32.0,
                y: 32.0,
                width: 1216.0,
                height: 32.0,
            },
        });
        let frame = frame_with_runtime_warnings(&app, &[Some("Software backend fallback".into())])
            .expect("frame with diagnostics");
        assert!(frame.commands.iter().any(|command| matches!(command, RenderCommand::Text { text, rect, .. } if text.contains("device missing") && rect.y >= 64.0)));
        assert!(frame.commands.iter().any(|command| matches!(command, RenderCommand::Text { text, rect, .. } if text.contains("Software backend fallback") && rect.y >= 64.0)));
        assert!(!app.scene_frame().commands.iter().any(|command| matches!(command, RenderCommand::Text { text, .. } if text.contains("device missing"))));
        let mut initial = engine(vec![dialogue()]);
        initial.queue_audio_command(AudioCommand::PlaySfx {
            resource: AssetId::from_path("intro.ogg"),
            path: "intro.ogg".into(),
            volume: None,
        });
        let err = match RuntimeApp::<NoInput, SilentAudio, MemoryAssetStore>::new_auto_with_factory(
            initial,
            NoInput,
            assets,
            |_| Err("device missing".into()),
        ) {
            Ok(_) => panic!("initial command must fail"),
            Err(err) => err.to_string(),
        };
        assert!(
            err.contains("intro.ogg")
                && err.contains("audio backend unavailable")
                && err.contains("device missing"),
            "{err}"
        );
    }

    #[test]
    fn prefetch_diagnostics_are_bounded_and_clear_after_recovery_or_disable() {
        let working = Arc::new(AtomicBool::new(false));
        let events = (0..40)
            .map(|index| {
                EventRaw::Scene(SceneUpdateRaw {
                    background: Some(format!("bg/{index}.png")),
                    music: None,
                    characters: Vec::new(),
                })
            })
            .collect();
        let mut app = RuntimeApp::new(
            engine(events),
            NoInput,
            SilentAudio,
            ToggleAssets(working.clone()),
        )
        .unwrap();
        app.set_prefetch_depth(64);
        assert_eq!(app.prefetch_diagnostics().len(), 32);
        assert_eq!(app.prefetch_diagnostics_omitted(), 8);
        let previous = app.prefetch_diagnostics().to_vec();
        app.set_prefetch_depth(64);
        assert_eq!(app.prefetch_diagnostics(), previous);
        working.store(true, Ordering::Relaxed);
        app.set_prefetch_depth(64);
        assert!(app.prefetch_diagnostics().is_empty());
        assert_eq!(app.prefetch_diagnostics_omitted(), 0);
        working.store(false, Ordering::Relaxed);
        app.set_prefetch_depth(1);
        assert_eq!(app.prefetch_diagnostics().len(), 1);
        app.set_prefetch_depth(0);
        assert!(app.prefetch_diagnostics().is_empty());
    }
}
