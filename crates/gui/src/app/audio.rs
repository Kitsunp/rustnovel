use std::path::PathBuf;
use std::sync::Arc;

use visual_novel_engine::runtime::AudioCommand;
use visual_novel_runtime::{Audio, AudioCapabilities};

use crate::assets::{AssetStore, SecurityMode};

pub struct PlayerAudioController {
    backend: Box<dyn Audio>,
    assets: Option<Arc<dyn visual_novel_runtime::AssetStore + Send + Sync>>,
    mix: PlayerAudioMix,
    bgm: PlayerAudioChannelState,
    sfx: PlayerAudioChannelState,
    voice: PlayerAudioChannelState,
    last_event: Option<String>,
    last_warning: Option<String>,
    capabilities: AudioCapabilities,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerAudioChannel {
    Bgm,
    Sfx,
    Voice,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerAudioMix {
    pub master: f32,
    pub bgm: f32,
    pub sfx: f32,
    pub voice: f32,
    pub muted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerAudioChannelState {
    pub active: bool,
    pub path: Option<String>,
    pub command_volume: f32,
    pub effective_volume: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerAudioSnapshot {
    pub mix: PlayerAudioMix,
    pub bgm: PlayerAudioChannelState,
    pub sfx: PlayerAudioChannelState,
    pub voice: PlayerAudioChannelState,
    pub backend_silent: bool,
    pub last_event: Option<String>,
    pub last_warning: Option<String>,
}

impl Default for PlayerAudioMix {
    fn default() -> Self {
        Self {
            master: 1.0,
            bgm: 1.0,
            sfx: 1.0,
            voice: 1.0,
            muted: false,
        }
    }
}

impl PlayerAudioMix {
    pub fn sanitized(self) -> Self {
        Self {
            master: sanitize_volume(self.master),
            bgm: sanitize_volume(self.bgm),
            sfx: sanitize_volume(self.sfx),
            voice: sanitize_volume(self.voice),
            muted: self.muted,
        }
    }

    pub fn effective_volume(self, channel: PlayerAudioChannel, command_volume: Option<f32>) -> f32 {
        let mix = self.sanitized();
        if mix.muted {
            return 0.0;
        }
        let channel_volume = match channel {
            PlayerAudioChannel::Bgm => mix.bgm,
            PlayerAudioChannel::Sfx => mix.sfx,
            PlayerAudioChannel::Voice => mix.voice,
        };
        (mix.master * channel_volume * sanitize_command_volume(command_volume)).clamp(0.0, 1.0)
    }
}

impl Default for PlayerAudioChannelState {
    fn default() -> Self {
        Self {
            active: false,
            path: None,
            command_volume: 1.0,
            effective_volume: 0.0,
        }
    }
}

impl PlayerAudioController {
    pub fn new(
        assets_root: PathBuf,
        security_mode: SecurityMode,
        manifest_path: Option<PathBuf>,
        require_manifest: bool,
    ) -> Self {
        let asset_store =
            match AssetStore::new(assets_root, security_mode, manifest_path, require_manifest) {
                Ok(store) => store,
                Err(err) => {
                    return Self {
                        backend: Box::new(visual_novel_runtime::SilentAudio),
                        assets: None,
                        mix: PlayerAudioMix::default(),
                        bgm: PlayerAudioChannelState::default(),
                        sfx: PlayerAudioChannelState::default(),
                        voice: PlayerAudioChannelState::default(),
                        last_event: None,
                        last_warning: Some(format!(
                            "Audio assets unavailable; running silent: {err}"
                        )),
                        capabilities: AudioCapabilities::SILENT,
                    };
                }
            };
        let assets: Arc<dyn visual_novel_runtime::AssetStore + Send + Sync> = Arc::new(asset_store);
        let (backend, capabilities, last_warning): (
            Box<dyn Audio>,
            AudioCapabilities,
            Option<String>,
        ) = match visual_novel_runtime::RodioBackend::new(assets.clone()) {
            Ok(backend) => (Box::new(backend), AudioCapabilities::RODIO, None),
            Err(err) => (
                Box::new(visual_novel_runtime::SilentAudio),
                AudioCapabilities::SILENT,
                Some(format!(
                    "Audio output unavailable; commands are tracked but playback is silent: {err}"
                )),
            ),
        };
        Self {
            backend,
            assets: Some(assets),
            mix: PlayerAudioMix::default(),
            bgm: PlayerAudioChannelState::default(),
            sfx: PlayerAudioChannelState::default(),
            voice: PlayerAudioChannelState::default(),
            last_event: None,
            last_warning,
            capabilities,
        }
    }

    pub fn apply_commands(&mut self, commands: Vec<AudioCommand>) {
        for command in commands {
            self.apply_command(command);
        }
    }

    pub fn last_event(&self) -> Option<&str> {
        self.last_event.as_deref()
    }

    pub fn last_warning(&self) -> Option<&str> {
        self.last_warning.as_deref()
    }

    pub fn set_mix(&mut self, mix: PlayerAudioMix) {
        let mix = mix.sanitized();
        if self.mix == mix {
            return;
        }
        self.mix = mix;
        self.reapply_channel_volumes();
    }

    pub fn snapshot(&self) -> PlayerAudioSnapshot {
        PlayerAudioSnapshot {
            mix: self.mix,
            bgm: self.bgm.clone(),
            sfx: self.sfx.clone(),
            voice: self.voice.clone(),
            backend_silent: self.capabilities.no_op,
            last_event: self.last_event.clone(),
            last_warning: self.last_warning.clone(),
        }
    }

    fn apply_command(&mut self, command: AudioCommand) {
        self.last_event = Some(audio_command_label(&command));
        match command {
            AudioCommand::PlayBgm {
                path,
                r#loop,
                volume,
                fade_in,
                ..
            } => {
                let path = path.as_ref();
                if self.audio_asset_is_ready("BGM", path) {
                    let command_volume = sanitize_command_volume(volume);
                    let effective_volume = self
                        .mix
                        .effective_volume(PlayerAudioChannel::Bgm, Some(command_volume));
                    self.backend.play_music_with_transition(
                        path,
                        r#loop,
                        Some(effective_volume),
                        Some(fade_in),
                    );
                    self.bgm = PlayerAudioChannelState {
                        active: true,
                        path: Some(path.to_string()),
                        command_volume,
                        effective_volume,
                    };
                }
            }
            AudioCommand::StopBgm { fade_out } => {
                self.backend.stop_music_with_fade(Some(fade_out));
                self.bgm = PlayerAudioChannelState::default();
            }
            AudioCommand::PlaySfx { path, volume, .. } => {
                let path = path.as_ref();
                if self.audio_asset_is_ready("SFX", path) {
                    let command_volume = sanitize_command_volume(volume);
                    let effective_volume = self
                        .mix
                        .effective_volume(PlayerAudioChannel::Sfx, Some(command_volume));
                    self.backend
                        .play_sfx_with_volume(path, Some(effective_volume));
                    self.sfx = PlayerAudioChannelState {
                        active: true,
                        path: Some(path.to_string()),
                        command_volume,
                        effective_volume,
                    };
                }
            }
            AudioCommand::StopSfx => {
                self.backend.stop_sfx();
                self.sfx = PlayerAudioChannelState::default();
            }
            AudioCommand::PlayVoice { path, volume, .. } => {
                let path = path.as_ref();
                if self.audio_asset_is_ready("Voice", path) {
                    let command_volume = sanitize_command_volume(volume);
                    let effective_volume = self
                        .mix
                        .effective_volume(PlayerAudioChannel::Voice, Some(command_volume));
                    self.backend
                        .play_voice_with_volume(path, Some(effective_volume));
                    self.voice = PlayerAudioChannelState {
                        active: true,
                        path: Some(path.to_string()),
                        command_volume,
                        effective_volume,
                    };
                }
            }
            AudioCommand::StopVoice => {
                self.backend.stop_voice();
                self.voice = PlayerAudioChannelState::default();
            }
        }
    }

    fn reapply_channel_volumes(&mut self) {
        let bgm_volume = self
            .mix
            .effective_volume(PlayerAudioChannel::Bgm, Some(self.bgm.command_volume));
        let sfx_volume = self
            .mix
            .effective_volume(PlayerAudioChannel::Sfx, Some(self.sfx.command_volume));
        let voice_volume = self
            .mix
            .effective_volume(PlayerAudioChannel::Voice, Some(self.voice.command_volume));
        self.backend.set_music_volume(bgm_volume);
        self.backend.set_sfx_volume(sfx_volume);
        self.backend.set_voice_volume(voice_volume);
        self.bgm.effective_volume = if self.bgm.active { bgm_volume } else { 0.0 };
        self.sfx.effective_volume = if self.sfx.active { sfx_volume } else { 0.0 };
        self.voice.effective_volume = if self.voice.active { voice_volume } else { 0.0 };
    }

    fn audio_asset_is_ready(&mut self, channel: &str, path: &str) -> bool {
        let Some(assets) = &self.assets else {
            return false;
        };
        match visual_novel_runtime::audio_duration(assets.as_ref(), path) {
            Ok(_) => true,
            Err(err) => {
                self.last_warning = Some(format!(
                    "{channel} audio '{path}' could not be loaded: {err}"
                ));
                false
            }
        }
    }
}

pub fn sanitize_volume(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn sanitize_command_volume(volume: Option<f32>) -> f32 {
    volume.map(sanitize_volume).unwrap_or(1.0)
}

pub fn audio_command_label(command: &AudioCommand) -> String {
    match command {
        AudioCommand::PlayBgm {
            path,
            r#loop,
            volume,
            fade_in,
            ..
        } => format!(
            "play_bgm path={} loop={} volume={} fade_in_ms={}",
            path.as_ref(),
            r#loop,
            format_volume(*volume),
            fade_in.as_millis()
        ),
        AudioCommand::StopBgm { fade_out } => {
            format!("stop_bgm fade_out_ms={}", fade_out.as_millis())
        }
        AudioCommand::PlaySfx { path, volume, .. } => {
            format!(
                "play_sfx path={} volume={}",
                path.as_ref(),
                format_volume(*volume)
            )
        }
        AudioCommand::StopSfx => "stop_sfx".to_string(),
        AudioCommand::PlayVoice { path, volume, .. } => {
            format!(
                "play_voice path={} volume={}",
                path.as_ref(),
                format_volume(*volume)
            )
        }
        AudioCommand::StopVoice => "stop_voice".to_string(),
    }
}

fn format_volume(volume: Option<f32>) -> String {
    match volume {
        Some(value) if value.is_finite() => format!("{:.3}", value.clamp(0.0, 1.0)),
        Some(_) => "invalid".to_string(),
        None => "default".to_string(),
    }
}
