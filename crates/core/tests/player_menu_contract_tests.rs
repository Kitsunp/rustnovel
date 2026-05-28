use visual_novel_engine::{
    manifest::ProjectSettings, PlayerMenuAction, PlayerMenuActionConfig, PlayerMenuConfig,
    PlayerMenuConfigError, PlayerMenuLayoutConfig, PlayerMenuPanelAnchor,
    PlayerMenuQuickActionPlacement, PlayerMenuTabConfig, PlayerMenuTabKind, PlayerMenuTabsPosition,
    ProjectManifest,
};

#[test]
fn default_player_menu_is_valid_and_exposes_game_runtime_surfaces() {
    let menu = PlayerMenuConfig::default();

    menu.validate().expect("default menu must be valid");
    assert!(menu.open_on_start);
    assert!(menu.advance_on_text_panel_click);
    assert_eq!(menu.title, "Menu");
    assert_eq!(menu.save_slots, 6);
    assert_eq!(menu.first_visible_tab(), PlayerMenuTabKind::Saves);
    assert_eq!(menu.initial_tab(), PlayerMenuTabKind::Saves);
    assert_eq!(
        menu.layout.quick_action_placement,
        PlayerMenuQuickActionPlacement::MenuHeader
    );
    assert_eq!(menu.quick_actions[0].action, PlayerMenuAction::ResumeGame);
    assert_eq!(menu.quick_actions[0].label, "Start");
    assert!(menu.style.panel_width_fraction > 0.0);
    assert!(menu.style.panel_height_fraction > 0.0);
    assert!(menu.style.button_min_width >= 48.0);
    assert!(menu.style.button_height >= 24.0);
    assert!(menu.tab_label(PlayerMenuTabKind::Routes).is_some());
    assert!(menu
        .quick_actions
        .iter()
        .any(|action| action.action == PlayerMenuAction::QuickSave));
    assert!(menu
        .quick_actions
        .iter()
        .any(|action| action.action == PlayerMenuAction::QuickLoad));
}

#[test]
fn player_menu_normalization_recovers_authoring_edge_cases() {
    let menu = PlayerMenuConfig {
        title: String::new(),
        save_slots: 0,
        quick_actions: vec![PlayerMenuActionConfig {
            action: PlayerMenuAction::OpenRoutes,
            label: String::new(),
            visible: true,
        }],
        tabs: vec![
            PlayerMenuTabConfig {
                kind: PlayerMenuTabKind::Routes,
                label: String::new(),
                visible: false,
            },
            PlayerMenuTabConfig {
                kind: PlayerMenuTabKind::Routes,
                label: "Duplicated".to_string(),
                visible: true,
            },
        ],
        layout: PlayerMenuLayoutConfig {
            initial_tab: Some(PlayerMenuTabKind::History),
            ..PlayerMenuLayoutConfig::default()
        },
        style: visual_novel_engine::PlayerMenuStyleConfig {
            panel_width: f32::NAN,
            panel_width_fraction: f32::INFINITY,
            panel_height_fraction: f32::NEG_INFINITY,
            button_min_width: -1.0,
            button_height: 0.0,
            button_corner_radius: f32::NAN,
            ..visual_novel_engine::PlayerMenuStyleConfig::default()
        },
        ..PlayerMenuConfig::default()
    };

    let normalized = menu.normalized();

    normalized
        .validate()
        .expect("normalized menu must be valid");
    assert_eq!(normalized.save_slots, 1);
    assert_eq!(normalized.title, "Menu");
    assert_eq!(
        normalized.style.panel_width,
        PlayerMenuConfig::default().style.panel_width
    );
    assert_eq!(
        normalized.style.panel_width_fraction,
        PlayerMenuConfig::default().style.panel_width_fraction
    );
    assert_eq!(
        normalized.style.panel_height_fraction,
        PlayerMenuConfig::default().style.panel_height_fraction
    );
    assert_eq!(
        normalized.style.button_min_width,
        PlayerMenuConfig::default().style.button_min_width
    );
    assert_eq!(normalized.initial_tab(), PlayerMenuTabKind::Routes);
    assert_eq!(
        normalized.quick_actions[0].label, "Routes",
        "empty action labels should fall back to core defaults"
    );
    assert_eq!(
        normalized.tabs[0].label, "Duplicated",
        "a visible duplicate tab with a real label should replace an earlier hidden duplicate"
    );
}

#[test]
fn player_menu_validation_rejects_unrenderable_design_contracts() {
    let missing_title = PlayerMenuConfig {
        title: "   ".to_string(),
        ..PlayerMenuConfig::default()
    };
    assert!(matches!(
        missing_title.validate(),
        Err(PlayerMenuConfigError::EmptyMenuTitle)
    ));

    let hidden_initial_tab = PlayerMenuConfig {
        tabs: vec![
            PlayerMenuTabConfig {
                kind: PlayerMenuTabKind::Saves,
                label: "Saves".to_string(),
                visible: true,
            },
            PlayerMenuTabConfig {
                kind: PlayerMenuTabKind::Routes,
                label: "Routes".to_string(),
                visible: false,
            },
        ],
        layout: PlayerMenuLayoutConfig {
            initial_tab: Some(PlayerMenuTabKind::Routes),
            ..PlayerMenuLayoutConfig::default()
        },
        ..PlayerMenuConfig::default()
    };
    assert!(matches!(
        hidden_initial_tab.validate(),
        Err(PlayerMenuConfigError::InvalidInitialTab {
            tab: PlayerMenuTabKind::Routes
        })
    ));

    let invalid_responsive_style = PlayerMenuConfig {
        style: visual_novel_engine::PlayerMenuStyleConfig {
            panel_width_fraction: 1.5,
            ..visual_novel_engine::PlayerMenuStyleConfig::default()
        },
        ..PlayerMenuConfig::default()
    };
    assert!(matches!(
        invalid_responsive_style.validate(),
        Err(PlayerMenuConfigError::InvalidPanelWidthFraction)
    ));

    let invalid_height_fraction = PlayerMenuConfig {
        style: visual_novel_engine::PlayerMenuStyleConfig {
            panel_height_fraction: 0.1,
            ..visual_novel_engine::PlayerMenuStyleConfig::default()
        },
        ..PlayerMenuConfig::default()
    };
    assert!(matches!(
        invalid_height_fraction.validate(),
        Err(PlayerMenuConfigError::InvalidPanelHeightFraction)
    ));

    let invalid_button_shape = PlayerMenuConfig {
        style: visual_novel_engine::PlayerMenuStyleConfig {
            button_height: 12.0,
            ..visual_novel_engine::PlayerMenuStyleConfig::default()
        },
        ..PlayerMenuConfig::default()
    };
    assert!(matches!(
        invalid_button_shape.validate(),
        Err(PlayerMenuConfigError::InvalidButtonSize)
    ));
}

#[test]
fn project_manifest_roundtrips_player_menu_configuration() {
    let toml = r#"
manifest_schema_version = "1.0"

[metadata]
name = "menu-fixture"
author = "qa"
version = "0.1.0"

[settings]
resolution = [1280, 720]
entry_point = "main.json"

[settings.player_menu]
open_on_start = false
advance_on_text_panel_click = false
title = "Pause"
save_slots = 3

[settings.player_menu.layout]
panel_anchor = "top_right"
quick_action_placement = "menu_header"
tabs_position = "left"
initial_tab = "routes"

[settings.player_menu.style]
panel_width = 640.0
panel_width_fraction = 0.74
panel_height_fraction = 0.66
button_min_width = 112.0
button_height = 36.0
button_corner_radius = 6.0

[settings.player_menu.style.background]
r = 8
g = 12
b = 24
a = 250

[[settings.player_menu.quick_actions]]
action = "open_menu"
label = "Pause"

[[settings.player_menu.quick_actions]]
action = "quick_save"
label = "QS"

[[settings.player_menu.tabs]]
kind = "routes"
label = "Route Log"

[[settings.player_menu.tabs]]
kind = "settings"
label = "Options"

[assets]
"#;

    let (manifest, _) = ProjectManifest::from_toml_with_migration(toml).expect("manifest");
    assert_eq!(manifest.settings.player_menu.title, "Pause");
    assert!(!manifest.settings.player_menu.open_on_start);
    assert!(!manifest.settings.player_menu.advance_on_text_panel_click);
    assert_eq!(manifest.settings.player_menu.save_slots, 3);
    assert_eq!(
        manifest.settings.player_menu.layout.panel_anchor,
        PlayerMenuPanelAnchor::TopRight
    );
    assert_eq!(
        manifest.settings.player_menu.layout.quick_action_placement,
        PlayerMenuQuickActionPlacement::MenuHeader
    );
    assert_eq!(
        manifest.settings.player_menu.layout.tabs_position,
        PlayerMenuTabsPosition::Left
    );
    assert_eq!(
        manifest.settings.player_menu.initial_tab(),
        PlayerMenuTabKind::Routes
    );
    assert_eq!(manifest.settings.player_menu.style.panel_width, 640.0);
    assert_eq!(
        manifest.settings.player_menu.style.panel_width_fraction,
        0.74
    );
    assert_eq!(
        manifest.settings.player_menu.style.panel_height_fraction,
        0.66
    );
    assert_eq!(manifest.settings.player_menu.style.button_min_width, 112.0);
    assert_eq!(manifest.settings.player_menu.style.button_height, 36.0);
    assert_eq!(
        manifest.settings.player_menu.style.button_corner_radius,
        6.0
    );
    assert_eq!(manifest.settings.player_menu.style.background.r, 8);
    assert_eq!(
        manifest.settings.player_menu.quick_actions[0].action,
        PlayerMenuAction::OpenMenu
    );
    assert_eq!(
        manifest.settings.player_menu.tabs[0].kind,
        PlayerMenuTabKind::Routes
    );

    let serialized = toml::to_string_pretty(&manifest).expect("serialize manifest");
    let (roundtrip, _) =
        ProjectManifest::from_toml_with_migration(&serialized).expect("roundtrip manifest");
    assert_eq!(
        roundtrip.settings.player_menu,
        manifest.settings.player_menu
    );
}

#[test]
fn old_manifest_without_player_menu_uses_default_runtime_menu() {
    let toml = r#"
manifest_schema_version = "1.0"

[metadata]
name = "legacy"
author = "qa"
version = "0.1.0"

[settings]
resolution = [1280, 720]
entry_point = "main.json"

[assets]
"#;

    let (manifest, _) = ProjectManifest::from_toml_with_migration(toml).expect("manifest");
    assert_eq!(
        manifest.settings.player_menu,
        ProjectSettings {
            resolution: (1280, 720),
            default_language: "en".to_string(),
            supported_languages: vec!["en".to_string()],
            entry_point: "main.json".to_string(),
            player_menu: PlayerMenuConfig::default(),
        }
        .player_menu
    );
}
