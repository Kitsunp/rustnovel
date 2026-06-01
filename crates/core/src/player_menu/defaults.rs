use super::*;

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_save_slots() -> u16 {
    DEFAULT_SAVE_SLOTS
}

pub(super) fn default_menu_title() -> String {
    "Menu".to_string()
}

pub(super) fn default_panel_anchor() -> PlayerMenuPanelAnchor {
    PlayerMenuPanelAnchor::Center
}

pub(super) fn default_quick_action_placement() -> PlayerMenuQuickActionPlacement {
    PlayerMenuQuickActionPlacement::MenuHeader
}

pub(super) fn default_tabs_position() -> PlayerMenuTabsPosition {
    PlayerMenuTabsPosition::Top
}

pub(super) fn default_panel_width() -> f32 {
    720.0
}

pub(super) fn default_panel_width_fraction() -> f32 {
    0.86
}

pub(super) fn default_panel_height_fraction() -> f32 {
    0.62
}

pub(super) fn default_button_min_width() -> f32 {
    96.0
}

pub(super) fn default_button_height() -> f32 {
    32.0
}

pub(super) fn default_button_corner_radius() -> f32 {
    4.0
}

pub(super) fn default_panel_alpha() -> u8 {
    238
}

pub(super) fn default_background_color() -> PlayerMenuColor {
    PlayerMenuColor {
        r: 20,
        g: 22,
        b: 30,
        a: 255,
    }
}

pub(super) fn default_accent_color() -> PlayerMenuColor {
    PlayerMenuColor {
        r: 130,
        g: 176,
        b: 255,
        a: 255,
    }
}

pub(super) fn default_warning_color() -> PlayerMenuColor {
    PlayerMenuColor {
        r: 245,
        g: 214,
        b: 110,
        a: 255,
    }
}

pub(super) fn default_danger_color() -> PlayerMenuColor {
    PlayerMenuColor {
        r: 245,
        g: 110,
        b: 110,
        a: 255,
    }
}

pub(super) fn default_quick_actions() -> Vec<PlayerMenuActionConfig> {
    [
        PlayerMenuAction::ResumeGame,
        PlayerMenuAction::QuickSave,
        PlayerMenuAction::QuickLoad,
        PlayerMenuAction::OpenSaves,
        PlayerMenuAction::OpenRoutes,
        PlayerMenuAction::OpenSettings,
    ]
    .into_iter()
    .map(|action| PlayerMenuActionConfig {
        action,
        label: default_action_label(action).to_string(),
        visible: true,
    })
    .collect()
}

pub(super) fn default_tabs() -> Vec<PlayerMenuTabConfig> {
    [
        PlayerMenuTabKind::Saves,
        PlayerMenuTabKind::History,
        PlayerMenuTabKind::Routes,
        PlayerMenuTabKind::Settings,
        PlayerMenuTabKind::System,
    ]
    .into_iter()
    .map(|kind| PlayerMenuTabConfig {
        kind,
        label: default_tab_label(kind).to_string(),
        visible: true,
    })
    .collect()
}

pub(super) fn default_action_label(action: PlayerMenuAction) -> &'static str {
    match action {
        PlayerMenuAction::ResumeGame => "Start",
        PlayerMenuAction::OpenMenu => "Menu",
        PlayerMenuAction::QuickSave => "Quick Save",
        PlayerMenuAction::QuickLoad => "Quick Load",
        PlayerMenuAction::OpenSaves => "Saves",
        PlayerMenuAction::OpenHistory => "History",
        PlayerMenuAction::OpenRoutes => "Routes",
        PlayerMenuAction::OpenSettings => "Settings",
        PlayerMenuAction::OpenSystem => "System",
        PlayerMenuAction::ToggleHistoryWindow => "History Window",
        PlayerMenuAction::ToggleFullscreen => "Fullscreen",
        PlayerMenuAction::RestartStory => "Restart",
        PlayerMenuAction::QuitGame => "Quit",
    }
}

pub(super) fn default_tab_label(kind: PlayerMenuTabKind) -> &'static str {
    match kind {
        PlayerMenuTabKind::Saves => "Saves",
        PlayerMenuTabKind::History => "History",
        PlayerMenuTabKind::Routes => "Routes",
        PlayerMenuTabKind::Settings => "Settings",
        PlayerMenuTabKind::System => "System",
    }
}
