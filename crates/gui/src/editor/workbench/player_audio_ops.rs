use super::*;
use std::sync::Arc;
use std::time::Duration;

impl EditorWorkbench {
    pub(super) fn ensure_player_audio_backend(&mut self) {
        let project_root = self.project_root.clone();
        if self.player_audio_backend.is_some() && self.player_audio_root == project_root {
            return;
        }

        match super::audio_preview_store::GuiAudioAssetStore::new(project_root.clone()) {
            Ok(store) => {
                let store = Arc::new(store);
                match visual_novel_runtime::RodioBackend::new(store) {
                    Ok(backend) => {
                        let no_project_root = project_root.is_none();
                        self.player_audio_backend = Some(Box::new(backend));
                        self.player_audio_root = project_root;
                        self.player_state.last_audio_error = if no_project_root {
                            Some(
                                "Project root not set; only absolute audio paths can preview"
                                    .to_string(),
                            )
                        } else {
                            None
                        };
                    }
                    Err(err) => {
                        self.player_audio_backend =
                            Some(Box::new(visual_novel_runtime::SilentAudio));
                        self.player_audio_root = project_root;
                        self.player_state.last_audio_error = Some(format!(
                            "Audio output unavailable; running silent preview: {err}"
                        ));
                    }
                }
            }
            Err(err) => {
                self.player_audio_backend = Some(Box::new(visual_novel_runtime::SilentAudio));
                self.player_audio_root = project_root;
                self.player_state.last_audio_error =
                    Some(format!("Asset store unavailable for audio preview: {err}"));
            }
        }
    }

    pub(super) fn apply_player_audio_commands(
        &mut self,
        commands: Vec<visual_novel_engine::AudioCommand>,
    ) {
        if commands.is_empty() {
            return;
        }
        self.ensure_player_audio_backend();
        if self.player_audio_backend.is_none() {
            return;
        }
        for command in commands {
            match command {
                visual_novel_engine::AudioCommand::PlayBgm {
                    path,
                    r#loop,
                    volume,
                    ..
                } => {
                    let playback_path = self.resolve_preview_audio_path("BGM", path.as_ref());
                    let output_volume = self.mix_volume(volume, AudioPreviewChannel::Bgm);
                    self.player_state.last_audio_event = Some(format!(
                        "play_bgm path={} loop={} volume={:?}",
                        playback_path, r#loop, output_volume
                    ));
                    if let Some(audio_backend) = self.player_audio_backend.as_mut() {
                        audio_backend.play_music_with_options(
                            playback_path.as_str(),
                            r#loop,
                            output_volume,
                        );
                    }
                }
                visual_novel_engine::AudioCommand::StopBgm { fade_out } => {
                    self.player_state.last_audio_event =
                        Some(format!("stop_bgm fade_out_ms={}", fade_out.as_millis()));
                    if let Some(audio_backend) = self.player_audio_backend.as_mut() {
                        audio_backend.stop_music_with_fade(Some(fade_out));
                    }
                }
                visual_novel_engine::AudioCommand::PlaySfx { path, volume, .. } => {
                    let playback_path = self.resolve_preview_audio_path("SFX", path.as_ref());
                    let output_volume = self.mix_volume(volume, AudioPreviewChannel::Sfx);
                    self.player_state.last_audio_event = Some(format!(
                        "play_sfx path={} volume={:?}",
                        playback_path, output_volume
                    ));
                    if let Some(audio_backend) = self.player_audio_backend.as_mut() {
                        audio_backend.play_sfx_with_volume(playback_path.as_str(), output_volume);
                    }
                }
                visual_novel_engine::AudioCommand::StopSfx => {
                    self.player_state.last_audio_event = Some("stop_sfx".to_string());
                    if let Some(audio_backend) = self.player_audio_backend.as_mut() {
                        audio_backend.stop_sfx();
                    }
                }
                visual_novel_engine::AudioCommand::PlayVoice { path, volume, .. } => {
                    let playback_path = self.resolve_preview_audio_path("Voice", path.as_ref());
                    let output_volume = self.mix_volume(volume, AudioPreviewChannel::Voice);
                    self.player_state.last_audio_event = Some(format!(
                        "play_voice path={} volume={:?}",
                        playback_path, output_volume
                    ));
                    if let Some(audio_backend) = self.player_audio_backend.as_mut() {
                        audio_backend.play_voice_with_volume(playback_path.as_str(), output_volume);
                    }
                }
                visual_novel_engine::AudioCommand::StopVoice => {
                    self.player_state.last_audio_event = Some("stop_voice".to_string());
                    if let Some(audio_backend) = self.player_audio_backend.as_mut() {
                        audio_backend.stop_voice();
                    }
                }
            }
        }
    }

    pub(crate) fn play_editor_audio_preview(
        &mut self,
        channel: &str,
        path: &str,
        volume: Option<f32>,
        loop_playback: bool,
    ) {
        self.play_editor_audio_preview_from_offset(
            channel,
            path,
            volume,
            loop_playback,
            Duration::ZERO,
        );
    }

    pub(crate) fn play_editor_audio_preview_from_offset(
        &mut self,
        channel: &str,
        path: &str,
        volume: Option<f32>,
        loop_playback: bool,
        start_at: Duration,
    ) {
        if path.trim().is_empty() {
            self.toast = Some(ToastState::warning("Audio preview requires an asset path"));
            return;
        }

        let command = match normalize_audio_channel(channel).as_str() {
            "bgm" => visual_novel_engine::AudioCommand::PlayBgm {
                resource: visual_novel_engine::AssetId::from_path(path),
                path: path.into(),
                r#loop: loop_playback,
                volume,
                fade_in: Duration::from_millis(0),
            },
            "sfx" => visual_novel_engine::AudioCommand::PlaySfx {
                resource: visual_novel_engine::AssetId::from_path(path),
                path: path.into(),
                volume,
            },
            "voice" => visual_novel_engine::AudioCommand::PlayVoice {
                resource: visual_novel_engine::AssetId::from_path(path),
                path: path.into(),
                volume,
            },
            _ => {
                self.toast = Some(ToastState::warning(format!(
                    "Unknown audio channel '{}'",
                    channel
                )));
                return;
            }
        };
        if !start_at.is_zero() && normalize_audio_channel(channel) == "bgm" {
            self.apply_editor_bgm_preview_command_from_offset(
                path,
                volume,
                loop_playback,
                start_at,
            );
        } else {
            self.apply_player_audio_commands(vec![command]);
        }
    }

    fn apply_editor_bgm_preview_command_from_offset(
        &mut self,
        path: &str,
        volume: Option<f32>,
        loop_playback: bool,
        start_at: Duration,
    ) {
        self.ensure_player_audio_backend();
        let playback_path = self.resolve_preview_audio_path("BGM", path);
        let output_volume = self.mix_volume(volume, AudioPreviewChannel::Bgm);
        self.player_state.last_audio_event = Some(format!(
            "play_bgm path={} loop={} volume={:?} offset_ms={}",
            playback_path,
            loop_playback,
            output_volume,
            start_at.as_millis()
        ));
        if let Some(audio_backend) = self.player_audio_backend.as_mut() {
            audio_backend.play_music_with_options_at(
                playback_path.as_str(),
                loop_playback,
                output_volume,
                start_at,
            );
        }
    }

    pub(crate) fn stop_editor_audio_preview(&mut self, channel: &str) {
        let command = match normalize_audio_channel(channel).as_str() {
            "bgm" => visual_novel_engine::AudioCommand::StopBgm {
                fade_out: Duration::from_millis(0),
            },
            "sfx" => visual_novel_engine::AudioCommand::StopSfx,
            "voice" => visual_novel_engine::AudioCommand::StopVoice,
            _ => visual_novel_engine::AudioCommand::StopBgm {
                fade_out: Duration::from_millis(0),
            },
        };
        self.apply_player_audio_commands(vec![command]);
    }

    fn mix_volume(&self, command_volume: Option<f32>, channel: AudioPreviewChannel) -> Option<f32> {
        let (master, muted) = match channel {
            AudioPreviewChannel::Bgm => (self.player_state.bgm_volume, self.player_state.bgm_muted),
            AudioPreviewChannel::Sfx => (self.player_state.sfx_volume, self.player_state.sfx_muted),
            AudioPreviewChannel::Voice => (
                self.player_state.voice_volume,
                self.player_state.voice_muted,
            ),
        };
        let output = if muted {
            0.0
        } else {
            command_volume.unwrap_or(1.0) * master
        };
        Some(output.clamp(0.0, 1.0))
    }

    fn resolve_preview_audio_path(&mut self, kind: &str, raw_path: &str) -> String {
        let resolved_path = super::player_audio_path::resolve_player_audio_asset_path(
            self.project_root.as_deref(),
            raw_path,
        );
        let unresolved = resolved_path.is_none();
        let playback_path = resolved_path.unwrap_or_else(|| raw_path.to_string());
        self.player_state.last_audio_error = if unresolved && self.project_root.is_some() {
            Some(format!(
                "Audio preview could not resolve {} path '{}'; checked canonical candidates in project root",
                kind, raw_path
            ))
        } else {
            None
        };
        playback_path
    }
}

enum AudioPreviewChannel {
    Bgm,
    Sfx,
    Voice,
}

fn normalize_audio_channel(channel: &str) -> String {
    channel.trim().to_ascii_lowercase()
}
