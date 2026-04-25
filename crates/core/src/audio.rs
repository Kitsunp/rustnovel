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
