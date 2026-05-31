use std::time::Duration;

use crate::assets::AssetId;
use crate::audio::AudioCommand;
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
