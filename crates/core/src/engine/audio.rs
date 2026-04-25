use std::time::Duration;

use crate::assets::AssetId;
use crate::audio::AudioCommand;
use crate::event::{AudioActionCompiled, SharedStr};
use crate::state::EngineState;

pub(crate) const DEFAULT_FADE_MS: u64 = 500;

pub(crate) fn initial_audio_commands(state: &EngineState) -> Vec<AudioCommand> {
    let mut commands = Vec::new();
    if let Some(music) = &state.visual.music {
        commands.push(AudioCommand::PlayBgm {
            resource: AssetId::from_path(music.as_ref()),
            path: music.clone(),
            r#loop: true,
            volume: None,
            fade_in: Duration::from_millis(DEFAULT_FADE_MS),
        });
    }
    commands
}

pub(crate) fn append_music_delta(
    before: Option<SharedStr>,
    after: &Option<SharedStr>,
    audio_commands: &mut Vec<AudioCommand>,
) {
    if before.as_deref() == after.as_deref() {
        return;
    }
    match after {
        Some(music) => audio_commands.push(AudioCommand::PlayBgm {
            resource: AssetId::from_path(music.as_ref()),
            path: music.clone(),
            r#loop: true,
            volume: None,
            fade_in: Duration::from_millis(DEFAULT_FADE_MS),
        }),
        None => audio_commands.push(AudioCommand::StopBgm {
            fade_out: Duration::from_millis(DEFAULT_FADE_MS),
        }),
    }
}

pub(crate) fn audio_command_from_action(action: &AudioActionCompiled) -> Option<AudioCommand> {
    match action.action {
        0 => audio_play_command(action),
        1 | 2 => audio_stop_command(action),
        _ => None,
    }
}

fn audio_play_command(action: &AudioActionCompiled) -> Option<AudioCommand> {
    let path = action.asset.as_ref()?;
    match action.channel {
        0 => Some(AudioCommand::PlayBgm {
            resource: AssetId::from_path(path.as_ref()),
            path: path.clone(),
            r#loop: action.loop_playback.unwrap_or(true),
            volume: action.volume,
            fade_in: Duration::from_millis(action.fade_duration_ms.unwrap_or(DEFAULT_FADE_MS)),
        }),
        1 => Some(AudioCommand::PlaySfx {
            resource: AssetId::from_path(path.as_ref()),
            path: path.clone(),
            volume: action.volume,
        }),
        2 => Some(AudioCommand::PlayVoice {
            resource: AssetId::from_path(path.as_ref()),
            path: path.clone(),
            volume: action.volume,
        }),
        _ => None,
    }
}

fn audio_stop_command(action: &AudioActionCompiled) -> Option<AudioCommand> {
    match action.channel {
        0 => Some(AudioCommand::StopBgm {
            fade_out: Duration::from_millis(action.fade_duration_ms.unwrap_or(DEFAULT_FADE_MS)),
        }),
        1 => Some(AudioCommand::StopSfx),
        2 => Some(AudioCommand::StopVoice),
        _ => None,
    }
}
