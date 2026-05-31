use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use super::super::*;

#[derive(Default, Debug)]
struct AudioProbeState {
    bgm: Vec<(String, bool, Option<f32>)>,
    bgm_with_offset: Vec<(String, bool, Option<f32>, u128)>,
    sfx: Vec<(String, Option<f32>)>,
    stop_bgm: usize,
}

#[derive(Clone)]
struct AudioProbe {
    state: Rc<RefCell<AudioProbeState>>,
}

impl visual_novel_runtime::Audio for AudioProbe {
    fn play_music(&mut self, id: &str) -> Result<(), String> {
        self.play_music_with_options(id, true, None)
    }

    fn play_music_with_options(
        &mut self,
        id: &str,
        loop_playback: bool,
        volume: Option<f32>,
    ) -> Result<(), String> {
        self.state
            .borrow_mut()
            .bgm
            .push((id.to_string(), loop_playback, volume));
        Ok(())
    }

    fn play_music_with_options_at(
        &mut self,
        id: &str,
        loop_playback: bool,
        volume: Option<f32>,
        start_at: Duration,
    ) -> Result<(), String> {
        self.state.borrow_mut().bgm_with_offset.push((
            id.to_string(),
            loop_playback,
            volume,
            start_at.as_millis(),
        ));
        Ok(())
    }

    fn stop_music(&mut self) -> Result<(), String> {
        self.state.borrow_mut().stop_bgm += 1;
        Ok(())
    }

    fn stop_music_with_fade(&mut self, _fade_out: Option<Duration>) -> Result<(), String> {
        self.stop_music()
    }

    fn play_sfx(&mut self, id: &str) -> Result<(), String> {
        self.play_sfx_with_volume(id, None)
    }

    fn play_sfx_with_volume(&mut self, id: &str, volume: Option<f32>) -> Result<(), String> {
        self.state.borrow_mut().sfx.push((id.to_string(), volume));
        Ok(())
    }
}

struct FailingAudioProbe;

impl visual_novel_runtime::Audio for FailingAudioProbe {
    fn play_music(&mut self, id: &str) -> Result<(), String> {
        Err(format!("backend rejected {id}"))
    }

    fn stop_music(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn play_sfx(&mut self, _id: &str) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn editor_audio_preview_uses_backend_and_mix_controls() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let state = Rc::new(RefCell::new(AudioProbeState::default()));
    workbench.player_audio_backend = Some(Box::new(AudioProbe {
        state: state.clone(),
    }));
    workbench.player_state.bgm_volume = 0.5;

    workbench.play_editor_audio_preview("bgm", "audio/theme.ogg", Some(0.8), true);

    {
        let state = state.borrow();
        assert_eq!(state.bgm.len(), 1);
        assert_eq!(
            state.bgm[0],
            ("audio/theme.ogg".to_string(), true, Some(0.4))
        );
    }

    workbench.player_state.sfx_muted = true;
    workbench.play_editor_audio_preview("sfx", "audio/click.ogg", None, false);

    let state = state.borrow();
    assert_eq!(state.sfx.len(), 1);
    assert_eq!(state.sfx[0], ("audio/click.ogg".to_string(), Some(0.0)));
}

#[test]
fn editor_audio_preview_reports_backend_failures() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    workbench.player_audio_backend = Some(Box::new(FailingAudioProbe));

    workbench.play_editor_audio_preview("bgm", "audio/theme.ogg", Some(0.8), true);

    let err = workbench
        .player_state
        .last_audio_error
        .as_deref()
        .expect("backend error should be visible");
    assert!(err.contains("Audio preview command failed"));
    assert!(err.contains("backend rejected audio/theme.ogg"));
}

#[test]
fn editor_audio_preview_from_offset_routes_seekable_bgm_without_affecting_sfx() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let state = Rc::new(RefCell::new(AudioProbeState::default()));
    workbench.player_audio_backend = Some(Box::new(AudioProbe {
        state: state.clone(),
    }));
    workbench.player_state.bgm_volume = 0.5;

    workbench.play_editor_audio_preview_from_offset(
        "bgm",
        "audio/theme.ogg",
        Some(0.8),
        false,
        Duration::from_millis(1250),
    );
    workbench.play_editor_audio_preview_from_offset(
        "sfx",
        "audio/click.ogg",
        None,
        false,
        Duration::from_millis(500),
    );

    let state = state.borrow();
    assert_eq!(
        state.bgm_with_offset,
        vec![("audio/theme.ogg".to_string(), false, Some(0.4), 1250)]
    );
    assert_eq!(state.sfx, vec![("audio/click.ogg".to_string(), Some(1.0))]);
}

#[test]
fn editor_audio_preview_stop_routes_to_backend() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let state = Rc::new(RefCell::new(AudioProbeState::default()));
    workbench.player_audio_backend = Some(Box::new(AudioProbe {
        state: state.clone(),
    }));

    workbench.stop_editor_audio_preview("bgm");

    assert_eq!(state.borrow().stop_bgm, 1);
}
