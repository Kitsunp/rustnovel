use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const DEFAULT_SAVE_SLOTS: u16 = 6;
const MAX_SAVE_SLOTS: u16 = 99;
const MIN_PANEL_WIDTH: f32 = 320.0;
const MIN_PANEL_WIDTH_FRACTION: f32 = 0.25;
const MAX_PANEL_WIDTH_FRACTION: f32 = 1.0;
const MIN_PANEL_HEIGHT_FRACTION: f32 = 0.25;
const MAX_PANEL_HEIGHT_FRACTION: f32 = 0.95;
const MIN_BUTTON_WIDTH: f32 = 48.0;
const MIN_BUTTON_HEIGHT: f32 = 24.0;
const MAX_BUTTON_CORNER_RADIUS: f32 = 32.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlayerMenuConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub open_on_start: bool,
    #[serde(default = "default_true")]
    pub advance_on_text_panel_click: bool,
    #[serde(default = "default_menu_title")]
    pub title: String,
    #[serde(default = "default_save_slots")]
    pub save_slots: u16,
    #[serde(default = "default_quick_actions")]
    pub quick_actions: Vec<PlayerMenuActionConfig>,
    #[serde(default = "default_tabs")]
    pub tabs: Vec<PlayerMenuTabConfig>,
    #[serde(default)]
    pub layout: PlayerMenuLayoutConfig,
    #[serde(default)]
    pub style: PlayerMenuStyleConfig,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlayerMenuActionConfig {
    pub action: PlayerMenuAction,
    #[serde(default)]
    pub label: String,
    #[serde(default = "default_true")]
    pub visible: bool,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PlayerMenuAction {
    ResumeGame,
    OpenMenu,
    QuickSave,
    QuickLoad,
    OpenSaves,
    OpenHistory,
    OpenRoutes,
    OpenSettings,
    OpenSystem,
    ToggleHistoryWindow,
    ToggleFullscreen,
    RestartStory,
    QuitGame,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlayerMenuTabConfig {
    pub kind: PlayerMenuTabKind,
    #[serde(default)]
    pub label: String,
    #[serde(default = "default_true")]
    pub visible: bool,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PlayerMenuTabKind {
    Saves,
    History,
    Routes,
    Settings,
    System,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlayerMenuLayoutConfig {
    #[serde(default = "default_panel_anchor")]
    pub panel_anchor: PlayerMenuPanelAnchor,
    #[serde(default = "default_quick_action_placement")]
    pub quick_action_placement: PlayerMenuQuickActionPlacement,
    #[serde(default = "default_tabs_position")]
    pub tabs_position: PlayerMenuTabsPosition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_tab: Option<PlayerMenuTabKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlayerMenuPanelAnchor {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlayerMenuQuickActionPlacement {
    Toolbar,
    MenuHeader,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlayerMenuTabsPosition {
    Top,
    Left,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlayerMenuStyleConfig {
    #[serde(default = "default_panel_width")]
    pub panel_width: f32,
    #[serde(default = "default_panel_width_fraction")]
    pub panel_width_fraction: f32,
    #[serde(default = "default_panel_height_fraction")]
    pub panel_height_fraction: f32,
    #[serde(default = "default_button_min_width")]
    pub button_min_width: f32,
    #[serde(default = "default_button_height")]
    pub button_height: f32,
    #[serde(default = "default_button_corner_radius")]
    pub button_corner_radius: f32,
    #[serde(default = "default_panel_alpha")]
    pub panel_alpha: u8,
    #[serde(default = "default_background_color")]
    pub background: PlayerMenuColor,
    #[serde(default = "default_accent_color")]
    pub accent: PlayerMenuColor,
    #[serde(default = "default_warning_color")]
    pub warning: PlayerMenuColor,
    #[serde(default = "default_danger_color")]
    pub danger: PlayerMenuColor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlayerMenuColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlayerMenuNormalizationReport {
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerMenuConfigError {
    SaveSlotsOutOfRange { value: u16, max: u16 },
    NoVisibleTabs,
    EmptyMenuTitle,
    EmptyActionLabel { action: PlayerMenuAction },
    EmptyTabLabel { tab: PlayerMenuTabKind },
    DuplicateTab { tab: PlayerMenuTabKind },
    InvalidInitialTab { tab: PlayerMenuTabKind },
    InvalidPanelWidth,
    InvalidPanelWidthFraction,
    InvalidPanelHeightFraction,
    InvalidButtonSize,
    InvalidButtonCornerRadius,
}

impl std::fmt::Display for PlayerMenuConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SaveSlotsOutOfRange { value, max } => {
                write!(f, "save_slots must be between 1 and {max}, got {value}")
            }
            Self::NoVisibleTabs => write!(f, "player menu must have at least one visible tab"),
            Self::EmptyMenuTitle => write!(f, "player menu title must not be empty"),
            Self::EmptyActionLabel { action } => {
                write!(f, "player menu action {action:?} has an empty label")
            }
            Self::EmptyTabLabel { tab } => {
                write!(f, "player menu tab {tab:?} has an empty label")
            }
            Self::DuplicateTab { tab } => write!(f, "player menu tab {tab:?} is duplicated"),
            Self::InvalidInitialTab { tab } => write!(
                f,
                "player menu initial_tab {tab:?} must reference a visible tab"
            ),
            Self::InvalidPanelWidth => {
                write!(
                    f,
                    "player menu panel_width must be finite and >= {MIN_PANEL_WIDTH}"
                )
            }
            Self::InvalidPanelWidthFraction => {
                write!(
                    f,
                    "player menu panel_width_fraction must be finite and between {MIN_PANEL_WIDTH_FRACTION} and {MAX_PANEL_WIDTH_FRACTION}"
                )
            }
            Self::InvalidPanelHeightFraction => {
                write!(
                    f,
                    "player menu panel_height_fraction must be finite and between {MIN_PANEL_HEIGHT_FRACTION} and {MAX_PANEL_HEIGHT_FRACTION}"
                )
            }
            Self::InvalidButtonSize => {
                write!(
                    f,
                    "player menu button_min_width/button_height must be finite and large enough to render"
                )
            }
            Self::InvalidButtonCornerRadius => {
                write!(
                    f,
                    "player menu button_corner_radius must be finite and between 0 and {MAX_BUTTON_CORNER_RADIUS}"
                )
            }
        }
    }
}

impl std::error::Error for PlayerMenuConfigError {}

impl Default for PlayerMenuConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            open_on_start: true,
            advance_on_text_panel_click: true,
            title: default_menu_title(),
            save_slots: DEFAULT_SAVE_SLOTS,
            quick_actions: default_quick_actions(),
            tabs: default_tabs(),
            layout: PlayerMenuLayoutConfig::default(),
            style: PlayerMenuStyleConfig::default(),
        }
    }
}

impl PlayerMenuConfig {
    pub fn validate_strict(&self) -> Result<(), PlayerMenuConfigError> {
        self.validate()
    }

    pub fn validate(&self) -> Result<(), PlayerMenuConfigError> {
        if self.enabled && self.title.trim().is_empty() {
            return Err(PlayerMenuConfigError::EmptyMenuTitle);
        }
        if self.save_slots == 0 || self.save_slots > MAX_SAVE_SLOTS {
            return Err(PlayerMenuConfigError::SaveSlotsOutOfRange {
                value: self.save_slots,
                max: MAX_SAVE_SLOTS,
            });
        }
        if self
            .quick_actions
            .iter()
            .any(|action| action.visible && action.label.trim().is_empty())
        {
            let Some(action) = self
                .quick_actions
                .iter()
                .find(|action| action.visible && action.label.trim().is_empty())
                .map(|action| action.action)
            else {
                return Err(PlayerMenuConfigError::EmptyMenuTitle);
            };
            return Err(PlayerMenuConfigError::EmptyActionLabel { action });
        }

        let mut seen_tabs = BTreeSet::new();
        let mut visible_tabs = 0usize;
        for tab in &self.tabs {
            if !seen_tabs.insert(tab.kind) {
                return Err(PlayerMenuConfigError::DuplicateTab { tab: tab.kind });
            }
            if tab.visible {
                visible_tabs += 1;
                if tab.label.trim().is_empty() {
                    return Err(PlayerMenuConfigError::EmptyTabLabel { tab: tab.kind });
                }
            }
        }
        if visible_tabs == 0 {
            return Err(PlayerMenuConfigError::NoVisibleTabs);
        }
        if let Some(initial_tab) = self.layout.initial_tab {
            if !self
                .tabs
                .iter()
                .any(|tab| tab.kind == initial_tab && tab.visible)
            {
                return Err(PlayerMenuConfigError::InvalidInitialTab { tab: initial_tab });
            }
        }
        if !self.style.panel_width.is_finite() || self.style.panel_width < MIN_PANEL_WIDTH {
            return Err(PlayerMenuConfigError::InvalidPanelWidth);
        }
        if !self.style.panel_width_fraction.is_finite()
            || !(MIN_PANEL_WIDTH_FRACTION..=MAX_PANEL_WIDTH_FRACTION)
                .contains(&self.style.panel_width_fraction)
        {
            return Err(PlayerMenuConfigError::InvalidPanelWidthFraction);
        }
        if !self.style.panel_height_fraction.is_finite()
            || !(MIN_PANEL_HEIGHT_FRACTION..=MAX_PANEL_HEIGHT_FRACTION)
                .contains(&self.style.panel_height_fraction)
        {
            return Err(PlayerMenuConfigError::InvalidPanelHeightFraction);
        }
        if !self.style.button_min_width.is_finite()
            || self.style.button_min_width < MIN_BUTTON_WIDTH
            || !self.style.button_height.is_finite()
            || self.style.button_height < MIN_BUTTON_HEIGHT
        {
            return Err(PlayerMenuConfigError::InvalidButtonSize);
        }
        if !self.style.button_corner_radius.is_finite()
            || !(0.0..=MAX_BUTTON_CORNER_RADIUS).contains(&self.style.button_corner_radius)
        {
            return Err(PlayerMenuConfigError::InvalidButtonCornerRadius);
        }
        Ok(())
    }

    pub fn normalized(&self) -> Self {
        self.normalize_with_warnings().0
    }

    pub fn normalize_with_warnings(&self) -> (Self, PlayerMenuNormalizationReport) {
        let mut normalized = self.clone();
        let mut warnings = Vec::new();
        if normalized.title.trim().is_empty() {
            normalized.title = default_menu_title();
            warnings.push("menu title was empty and was replaced with default".to_string());
        }
        normalized.save_slots = normalized.save_slots.clamp(1, MAX_SAVE_SLOTS);
        if !normalized.style.panel_width.is_finite()
            || normalized.style.panel_width < MIN_PANEL_WIDTH
        {
            normalized.style.panel_width = default_panel_width();
            warnings.push("panel_width was invalid and was replaced with default".to_string());
        }
        if !normalized.style.panel_width_fraction.is_finite() {
            normalized.style.panel_width_fraction = default_panel_width_fraction();
            warnings.push(
                "panel_width_fraction was non-finite and was replaced with default".to_string(),
            );
        }
        normalized.style.panel_width_fraction = normalized
            .style
            .panel_width_fraction
            .clamp(MIN_PANEL_WIDTH_FRACTION, MAX_PANEL_WIDTH_FRACTION);
        if !normalized.style.panel_height_fraction.is_finite() {
            normalized.style.panel_height_fraction = default_panel_height_fraction();
            warnings.push(
                "panel_height_fraction was non-finite and was replaced with default".to_string(),
            );
        }
        normalized.style.panel_height_fraction = normalized
            .style
            .panel_height_fraction
            .clamp(MIN_PANEL_HEIGHT_FRACTION, MAX_PANEL_HEIGHT_FRACTION);
        if !normalized.style.button_min_width.is_finite()
            || normalized.style.button_min_width < MIN_BUTTON_WIDTH
        {
            normalized.style.button_min_width = default_button_min_width();
            warnings.push("button_min_width was invalid and was replaced with default".to_string());
        }
        if !normalized.style.button_height.is_finite()
            || normalized.style.button_height < MIN_BUTTON_HEIGHT
        {
            normalized.style.button_height = default_button_height();
            warnings.push("button_height was invalid and was replaced with default".to_string());
        }
        if !normalized.style.button_corner_radius.is_finite() {
            normalized.style.button_corner_radius = default_button_corner_radius();
            warnings.push(
                "button_corner_radius was non-finite and was replaced with default".to_string(),
            );
        }
        normalized.style.button_corner_radius = normalized
            .style
            .button_corner_radius
            .clamp(0.0, MAX_BUTTON_CORNER_RADIUS);
        if normalized.quick_actions.is_empty() {
            normalized.quick_actions = default_quick_actions();
            warnings.push("quick_actions was empty and defaults were inserted".to_string());
        }
        if normalized.tabs.is_empty() {
            normalized.tabs = default_tabs();
            warnings.push("tabs was empty and defaults were inserted".to_string());
        }

        let original_tab_count = normalized.tabs.len();
        let mut deduped_tabs: Vec<PlayerMenuTabConfig> = Vec::new();
        for tab in normalized.tabs {
            if let Some(existing) = deduped_tabs
                .iter_mut()
                .find(|existing| existing.kind == tab.kind)
            {
                if !existing.visible && tab.visible {
                    *existing = tab;
                }
            } else {
                deduped_tabs.push(tab);
            }
        }
        if deduped_tabs.len() != original_tab_count {
            warnings.push("duplicate tabs were deduplicated".to_string());
        }
        normalized.tabs = deduped_tabs;
        if !normalized.tabs.iter().any(|tab| tab.visible) {
            normalized.tabs = default_tabs();
            warnings.push("no visible tabs remained and defaults were inserted".to_string());
        }
        for action in &mut normalized.quick_actions {
            if action.label.trim().is_empty() {
                action.label = default_action_label(action.action).to_string();
                warnings.push(format!(
                    "empty label for action {:?} was replaced with default",
                    action.action
                ));
            }
        }
        for tab in &mut normalized.tabs {
            if tab.label.trim().is_empty() {
                tab.label = default_tab_label(tab.kind).to_string();
                warnings.push(format!(
                    "empty label for tab {:?} was replaced with default",
                    tab.kind
                ));
            }
        }
        if normalized
            .layout
            .initial_tab
            .is_some_and(|tab| normalized.tab_label(tab).is_none())
        {
            normalized.layout.initial_tab = Some(normalized.first_visible_tab());
            warnings.push("invalid initial_tab was replaced with first visible tab".to_string());
        }
        (normalized, PlayerMenuNormalizationReport { warnings })
    }

    pub fn first_visible_tab(&self) -> PlayerMenuTabKind {
        self.tabs
            .iter()
            .find(|tab| tab.visible)
            .map(|tab| tab.kind)
            .unwrap_or(PlayerMenuTabKind::Saves)
    }

    pub fn tab_label(&self, kind: PlayerMenuTabKind) -> Option<&str> {
        self.tabs
            .iter()
            .find(|tab| tab.kind == kind && tab.visible)
            .map(|tab| tab.label.as_str())
    }

    pub fn initial_tab(&self) -> PlayerMenuTabKind {
        self.layout
            .initial_tab
            .filter(|tab| self.tab_label(*tab).is_some())
            .unwrap_or_else(|| self.first_visible_tab())
    }
}

impl Default for PlayerMenuLayoutConfig {
    fn default() -> Self {
        Self {
            panel_anchor: default_panel_anchor(),
            quick_action_placement: default_quick_action_placement(),
            tabs_position: default_tabs_position(),
            initial_tab: None,
        }
    }
}

impl Default for PlayerMenuStyleConfig {
    fn default() -> Self {
        Self {
            panel_width: default_panel_width(),
            panel_width_fraction: default_panel_width_fraction(),
            panel_height_fraction: default_panel_height_fraction(),
            button_min_width: default_button_min_width(),
            button_height: default_button_height(),
            button_corner_radius: default_button_corner_radius(),
            panel_alpha: default_panel_alpha(),
            background: default_background_color(),
            accent: default_accent_color(),
            warning: default_warning_color(),
            danger: default_danger_color(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_save_slots() -> u16 {
    DEFAULT_SAVE_SLOTS
}

fn default_menu_title() -> String {
    "Menu".to_string()
}

fn default_panel_anchor() -> PlayerMenuPanelAnchor {
    PlayerMenuPanelAnchor::Center
}

fn default_quick_action_placement() -> PlayerMenuQuickActionPlacement {
    PlayerMenuQuickActionPlacement::MenuHeader
}

fn default_tabs_position() -> PlayerMenuTabsPosition {
    PlayerMenuTabsPosition::Top
}

fn default_panel_width() -> f32 {
    720.0
}

fn default_panel_width_fraction() -> f32 {
    0.86
}

fn default_panel_height_fraction() -> f32 {
    0.62
}

fn default_button_min_width() -> f32 {
    96.0
}

fn default_button_height() -> f32 {
    32.0
}

fn default_button_corner_radius() -> f32 {
    4.0
}

fn default_panel_alpha() -> u8 {
    238
}

fn default_background_color() -> PlayerMenuColor {
    PlayerMenuColor {
        r: 20,
        g: 22,
        b: 30,
        a: 255,
    }
}

fn default_accent_color() -> PlayerMenuColor {
    PlayerMenuColor {
        r: 130,
        g: 176,
        b: 255,
        a: 255,
    }
}

fn default_warning_color() -> PlayerMenuColor {
    PlayerMenuColor {
        r: 245,
        g: 214,
        b: 110,
        a: 255,
    }
}

fn default_danger_color() -> PlayerMenuColor {
    PlayerMenuColor {
        r: 245,
        g: 110,
        b: 110,
        a: 255,
    }
}

fn default_quick_actions() -> Vec<PlayerMenuActionConfig> {
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

fn default_tabs() -> Vec<PlayerMenuTabConfig> {
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

fn default_action_label(action: PlayerMenuAction) -> &'static str {
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

fn default_tab_label(kind: PlayerMenuTabKind) -> &'static str {
    match kind {
        PlayerMenuTabKind::Saves => "Saves",
        PlayerMenuTabKind::History => "History",
        PlayerMenuTabKind::Routes => "Routes",
        PlayerMenuTabKind::Settings => "Settings",
        PlayerMenuTabKind::System => "System",
    }
}
