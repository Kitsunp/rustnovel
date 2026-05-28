use std::time::Duration;

use visual_novel_engine::{
    runtime::{
        AudioCommand, ChoiceHistoryEntry, ChoiceOptionRaw, ChoiceRaw, DialogueRaw, EventCompiled,
        EventRaw, SceneUpdateRaw, ScriptRaw,
    },
    AssetId, PlayerMenuAction, PlayerMenuActionConfig, PlayerMenuConfig, PlayerMenuLayoutConfig,
    PlayerMenuPanelAnchor, PlayerMenuQuickActionPlacement, PlayerMenuStyleConfig,
    PlayerMenuTabConfig, PlayerMenuTabKind, PlayerMenuTabsPosition,
};
use visual_novel_gui::{
    player_audio_command_label, player_event_requires_input, player_menu_action_button_size,
    player_menu_action_enabled, player_menu_content_height, player_menu_window_height,
    player_menu_window_width,
    player_overlay::{choice_overlay_layout, dialogue_overlay_rect, scene_overlay_rect},
    player_route_history_label, player_stage_available_size, player_stage_viewport_size,
    player_status_bar_height, player_text_panel_advance_enabled, sanitize_volume,
    PlayerAudioChannel, PlayerAudioMix, SecurityMode, UserPreferences, VnConfig,
};

fn assert_rect_inside(outer: egui::Rect, inner: egui::Rect) {
    assert!(
        inner.left() >= outer.left()
            && inner.right() <= outer.right()
            && inner.top() >= outer.top()
            && inner.bottom() <= outer.bottom(),
        "inner={inner:?} must fit inside outer={outer:?}"
    );
}

#[test]
fn player_menu_sizes_follow_viewport_and_style_contract() {
    let style = PlayerMenuStyleConfig {
        panel_width: 900.0,
        panel_width_fraction: 0.8,
        panel_height_fraction: 0.7,
        button_min_width: 140.0,
        button_height: 44.0,
        button_corner_radius: 10.0,
        ..PlayerMenuStyleConfig::default()
    };

    assert_eq!(player_menu_window_width(1000.0, &style), 800.0);
    assert_eq!(player_menu_window_width(360.0, &style), 288.0);
    assert_eq!(player_menu_window_height(800.0, &style), 560.0);
    assert_eq!(player_menu_content_height(800.0, &style), 428.0);

    let wide =
        player_menu_action_button_size(600.0, PlayerMenuAction::OpenSettings, "Options", &style);
    assert_eq!(wide.x, 140.0);
    assert_eq!(wide.y, 44.0);

    let narrow =
        player_menu_action_button_size(72.0, PlayerMenuAction::OpenSettings, "Options", &style);
    assert_eq!(narrow.x, 72.0);
    assert_eq!(narrow.y, 44.0);
}

#[test]
fn player_menu_quickload_is_disabled_until_quicksave_exists() {
    assert!(!player_menu_action_enabled(
        PlayerMenuAction::QuickLoad,
        false
    ));
    assert!(player_menu_action_enabled(
        PlayerMenuAction::QuickLoad,
        true
    ));
    assert!(player_menu_action_enabled(
        PlayerMenuAction::QuickSave,
        false
    ));
    assert!(player_menu_action_enabled(
        PlayerMenuAction::OpenSettings,
        false
    ));
}

#[test]
fn player_text_panel_advance_uses_author_default_until_user_overrides_it() {
    let author_button_only = PlayerMenuConfig {
        advance_on_text_panel_click: false,
        ..PlayerMenuConfig::default()
    };
    let mut prefs = UserPreferences::default();

    assert!(!player_text_panel_advance_enabled(
        &author_button_only,
        &prefs
    ));

    prefs.advance_on_text_panel_click = Some(true);
    assert!(player_text_panel_advance_enabled(
        &author_button_only,
        &prefs
    ));

    prefs.advance_on_text_panel_click = Some(false);
    assert!(!player_text_panel_advance_enabled(
        &PlayerMenuConfig::default(),
        &prefs
    ));
}

#[test]
fn player_route_history_label_is_player_facing_not_engine_debug_text() {
    let entry = ChoiceHistoryEntry {
        event_ip: 42,
        prompt: "Where should Sakura take you first?".to_string(),
        option_index: 1,
        option_text: "Find the music room".to_string(),
        target_ip: 99,
    };

    let label = player_route_history_label(0, &entry);

    assert_eq!(
        label,
        "1. Where should Sakura take you first? -> Find the music room"
    );
    assert!(!label.contains("ip "));
    assert!(!label.contains("target"));
}

#[test]
fn player_audio_command_label_clamps_invalid_and_edge_volumes() {
    let resource = AssetId::from_path("assets/audio/theme.ogg");
    let nan = player_audio_command_label(&AudioCommand::PlaySfx {
        resource,
        path: "assets/audio/theme.ogg".into(),
        volume: Some(f32::NAN),
    });
    assert!(nan.contains("volume=invalid"));

    let loud = player_audio_command_label(&AudioCommand::PlayBgm {
        resource,
        path: "assets/audio/theme.ogg".into(),
        r#loop: true,
        volume: Some(8.0),
        fade_in: Duration::from_millis(125),
    });
    assert!(loud.contains("volume=1.000"));
    assert!(loud.contains("fade_in_ms=125"));

    let quiet = player_audio_command_label(&AudioCommand::PlayVoice {
        resource,
        path: "assets/audio/theme.ogg".into(),
        volume: Some(-1.0),
    });
    assert!(quiet.contains("volume=0.000"));
}

#[test]
fn player_audio_mix_sanitizes_and_applies_channel_volumes() {
    let mix = PlayerAudioMix {
        master: 0.5,
        bgm: 0.8,
        sfx: f32::NAN,
        voice: 2.0,
        muted: false,
    };

    assert_eq!(sanitize_volume(f32::INFINITY), 1.0);
    assert!((mix.effective_volume(PlayerAudioChannel::Bgm, Some(0.5)) - 0.2).abs() < 0.0001);
    assert_eq!(
        mix.effective_volume(PlayerAudioChannel::Sfx, Some(0.5)),
        0.25
    );
    assert_eq!(
        mix.effective_volume(PlayerAudioChannel::Voice, Some(0.5)),
        0.25
    );

    let muted = PlayerAudioMix { muted: true, ..mix };
    assert_eq!(
        muted.effective_volume(PlayerAudioChannel::Bgm, Some(1.0)),
        0.0
    );
}

#[test]
fn player_config_can_require_bundle_asset_manifest() {
    let config = VnConfig {
        assets_root: Some("bundle-root".into()),
        manifest_path: Some("bundle-root/meta/assets_manifest.json".into()),
        require_manifest: Some(true),
        security_mode: SecurityMode::Trusted,
        ..VnConfig::default()
    };
    let resolved = config.resolve(None);

    assert_eq!(
        resolved.assets_root,
        std::path::PathBuf::from("bundle-root")
    );
    assert_eq!(
        resolved.manifest_path,
        Some(std::path::PathBuf::from(
            "bundle-root/meta/assets_manifest.json"
        ))
    );
    assert!(resolved.require_manifest);
}

#[test]
fn player_config_can_override_preferences_and_save_root_for_qa_or_launcher_profiles() {
    let path = std::path::PathBuf::from("profiles/session-a/prefs.json");
    let config = VnConfig {
        preferences_path: Some(path.clone()),
        ..VnConfig::default()
    };

    assert_eq!(config.preferences_path(), path);
}

#[test]
fn player_config_accepts_core_defined_menu_layout() {
    let config = VnConfig {
        player_menu: PlayerMenuConfig {
            open_on_start: false,
            title: "Pause".to_string(),
            save_slots: 4,
            quick_actions: vec![PlayerMenuActionConfig {
                action: PlayerMenuAction::ResumeGame,
                label: "Continue".to_string(),
                visible: true,
            }],
            tabs: vec![PlayerMenuTabConfig {
                kind: PlayerMenuTabKind::Routes,
                label: "Route Log".to_string(),
                visible: true,
            }],
            layout: PlayerMenuLayoutConfig {
                panel_anchor: PlayerMenuPanelAnchor::BottomRight,
                quick_action_placement: PlayerMenuQuickActionPlacement::MenuHeader,
                tabs_position: PlayerMenuTabsPosition::Left,
                initial_tab: Some(PlayerMenuTabKind::Routes),
            },
            ..PlayerMenuConfig::default()
        },
        ..VnConfig::default()
    };

    let resolved = config.resolve(None);

    assert_eq!(resolved.player_menu.save_slots, 4);
    assert!(!resolved.player_menu.open_on_start);
    assert_eq!(resolved.player_menu.title, "Pause");
    assert_eq!(
        resolved.player_menu.quick_actions[0].action,
        PlayerMenuAction::ResumeGame
    );
    assert_eq!(resolved.player_menu.quick_actions[0].label, "Continue");
    assert_eq!(
        resolved.player_menu.layout.panel_anchor,
        PlayerMenuPanelAnchor::BottomRight
    );
    assert_eq!(
        resolved.player_menu.layout.quick_action_placement,
        PlayerMenuQuickActionPlacement::MenuHeader
    );
    assert_eq!(
        resolved.player_menu.layout.tabs_position,
        PlayerMenuTabsPosition::Left
    );
    assert_eq!(
        resolved.player_menu.initial_tab(),
        PlayerMenuTabKind::Routes
    );
}

#[test]
fn player_event_input_contract_keeps_scene_updates_passthrough() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: Some("assets/bg.png".to_string()),
                music: None,
                characters: Vec::new(),
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "Visible text".to_string(),
            }),
            EventRaw::Choice(ChoiceRaw {
                prompt: "Pick one".to_string(),
                options: vec![ChoiceOptionRaw {
                    text: "A".to_string(),
                    target: "start".to_string(),
                }],
            }),
        ],
        std::collections::BTreeMap::from([("start".to_string(), 0)]),
    );
    let compiled = script.compile().expect("script compiles");
    let scene = &compiled.events[0];
    let dialogue = &compiled.events[1];
    let choice = &compiled.events[2];

    assert!(!player_event_requires_input(scene));
    assert!(matches!(dialogue, EventCompiled::Dialogue(_)));
    assert!(player_event_requires_input(dialogue));
    assert!(player_event_requires_input(choice));
}

#[test]
fn player_stage_keeps_dialogue_overlay_inside_visible_viewport() {
    let viewport = player_stage_viewport_size(egui::vec2(640.0, 320.0), (1280.0, 720.0));
    assert!(viewport.x <= 640.0);
    assert!(viewport.y <= 320.0);

    let stage = egui::Rect::from_min_size(egui::Pos2::ZERO, viewport);
    let dialogue = dialogue_overlay_rect(stage);

    assert_rect_inside(stage, dialogue);
    assert!(
        dialogue.bottom() < stage.bottom() + f32::EPSILON,
        "dialogue overlay must not be pushed below the stage"
    );
}

#[test]
fn player_stage_reserves_status_bar_space_before_sizing_viewport() {
    let reserved = player_status_bar_height(true, true);
    assert!(reserved > 0.0);

    let available = player_stage_available_size(egui::vec2(640.0, 320.0), reserved);
    assert_eq!(available.x, 640.0);
    assert!(
        available.y < 320.0,
        "status bar height must be removed before sizing the stage"
    );

    let viewport = player_stage_viewport_size(available, (1280.0, 720.0));
    assert!(viewport.y <= available.y);
    assert!(viewport.y > 0.0);
}

#[test]
fn player_stage_keeps_scene_continue_overlay_inside_visible_viewport() {
    for available in [
        egui::vec2(1280.0, 720.0),
        egui::vec2(640.0, 320.0),
        egui::vec2(260.0, 146.0),
    ] {
        let viewport = player_stage_viewport_size(available, (1280.0, 720.0));
        let stage = egui::Rect::from_min_size(egui::Pos2::ZERO, viewport);
        let scene = scene_overlay_rect(stage);

        assert_rect_inside(stage, scene);
        assert!(
            scene.bottom() <= stage.bottom() + f32::EPSILON,
            "scene Continue overlay must stay inside the stage"
        );
    }
}

#[test]
fn player_choice_overlay_handles_many_long_options_without_escaping_stage() {
    let viewport = player_stage_viewport_size(egui::vec2(480.0, 270.0), (1280.0, 720.0));
    let stage = egui::Rect::from_min_size(egui::Pos2::ZERO, viewport);
    let options = (0..12)
        .map(|idx| {
            format!(
                "Option {idx}: a very long unbrokenwordsegmentthatmustwrapwithoutmovingthepanel"
            )
        })
        .collect::<Vec<_>>();

    let layout = choice_overlay_layout(
        stage,
        "Choose a route with enough text to require wrapping",
        &options,
    );

    assert_rect_inside(stage, layout.panel);
    assert_eq!(layout.option_heights.len(), options.len());
    assert!(
        layout.options_viewport_height > 0.0,
        "choice list must leave an interactive scroll viewport"
    );
}
