use std::time::Duration;

use crate::assets::AssetId;
use crate::event::SharedStr;

/// Audio commands emitted by the engine.
/// Each command includes both AssetId (for caching) and path (for playback).
#[derive(Clone, Debug, PartialEq)]
pub enum AudioCommand {
    PlayBgm {
        resource: AssetId,
        path: SharedStr,
        r#loop: bool,
        volume: Option<f32>,
        fade_in: Duration,
    },
    StopBgm {
        fade_out: Duration,
    },
    PlaySfx {
        resource: AssetId,
        path: SharedStr,
        volume: Option<f32>,
    },
    StopSfx,
    PlayVoice {
        resource: AssetId,
        path: SharedStr,
        volume: Option<f32>,
    },
    StopVoice,
}

pub(crate) const DEFAULT_AUDIO_FADE_MS: u64 = 500;

/// Builds the canonical command for music represented by a scene snapshot.
pub(crate) fn scene_music_command(music: Option<&SharedStr>) -> AudioCommand {
    let fade = Duration::from_millis(DEFAULT_AUDIO_FADE_MS);
    match music {
        Some(path) => AudioCommand::PlayBgm {
            resource: AssetId::from_path(path.as_ref()),
            path: path.clone(),
            r#loop: true,
            volume: None,
            fade_in: fade,
        },
        None => AudioCommand::StopBgm { fade_out: fade },
    }
}
