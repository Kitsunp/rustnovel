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
    bgm_fades: Vec<Option<Duration>>,
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

    fn play_music_with_transition(
        &mut self,
        id: &str,
        loop_playback: bool,
        volume: Option<f32>,
        fade_in: Option<Duration>,
    ) -> Result<(), String> {
        self.state.borrow_mut().bgm_fades.push(fade_in);
        self.play_music_with_options(id, loop_playback, volume)
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

#[test]
fn composer_previews_inherited_music_and_exposes_audio_failures() {
    use crate::editor::{visual_composer::VisualComposerAction, StoryNode};
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    workbench.composer_preview_mode = crate::editor::ComposerPreviewMode::SelectedRoute;
    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::Pos2::ZERO);
    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: Some("music/theme.ogg".into()),
            characters: Vec::new(),
        },
        egui::pos2(0.0, 100.0),
    );
    let line = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".into(),
            text: "Music inherited".into(),
        },
        egui::pos2(0.0, 200.0),
    );
    workbench.node_graph.connect(start, scene);
    workbench.node_graph.connect(scene, line);
    workbench.node_graph.set_single_selection(Some(line));
    workbench.selected_node = Some(line);
    let state = Rc::new(RefCell::new(AudioProbeState::default()));
    workbench.player_audio_backend = Some(Box::new(AudioProbe {
        state: state.clone(),
    }));
    workbench.player_state.bgm_volume = 0.5;
    workbench.start_composer_runtime_preview_from_selection();
    assert_eq!(
        state.borrow().bgm,
        vec![("music/theme.ogg".into(), true, Some(0.5))]
    );
    workbench.handle_composer_actions(vec![VisualComposerAction::StopAudio], Some(line));
    assert_eq!(state.borrow().stop_bgm, 1);
    workbench.player_audio_backend = Some(Box::new(FailingAudioProbe));
    workbench.handle_composer_actions(vec![VisualComposerAction::PreviewAudio], Some(line));
    assert!(workbench
        .player_state
        .last_audio_error
        .as_deref()
        .unwrap()
        .contains("backend rejected"));
    assert!(workbench
        .toast
        .as_ref()
        .unwrap()
        .message
        .contains("backend rejected"));
}

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

    let snapshot = state.borrow();
    assert_eq!(snapshot.sfx.len(), 1);
    assert_eq!(snapshot.sfx[0], ("audio/click.ogg".to_string(), Some(0.0)));
    drop(snapshot);
    workbench.apply_player_audio_commands(vec![
        visual_novel_engine::runtime::AudioCommand::PlayBgm {
            resource: visual_novel_engine::AssetId::from_path("audio/load.ogg"),
            path: "audio/load.ogg".into(),
            r#loop: false,
            volume: Some(0.6),
            fade_in: Duration::from_millis(750),
        },
    ]);
    let state = state.borrow();
    assert_eq!(
        state.bgm.last().unwrap(),
        &("audio/load.ogg".to_string(), false, Some(0.3))
    );
    assert_eq!(
        state.bgm_fades.last(),
        Some(&Some(Duration::from_millis(750)))
    );
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
    workbench.handle_asset_browser_actions(vec![crate::editor::AssetBrowserAction::PreviewAudio {
        path: "audio/theme.ogg".into(),
        offset_ms: 0,
    }]);

    let state = state.borrow();
    assert_eq!(
        state.bgm_with_offset,
        vec![
            ("audio/theme.ogg".to_string(), false, Some(0.4), 1250),
            ("audio/theme.ogg".to_string(), true, Some(0.5), 0)
        ]
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
