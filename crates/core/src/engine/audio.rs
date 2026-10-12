use crate::audio::{scene_music_command, AudioCommand};
use crate::state::EngineState;

pub(crate) fn initial_audio_commands(state: &EngineState) -> Vec<AudioCommand> {
    let mut commands = Vec::new();
    if let Some(music) = &state.visual.music {
        commands.push(scene_music_command(Some(music)));
    }
    commands
}
