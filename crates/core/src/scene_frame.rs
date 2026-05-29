use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::route_tree::RouteTree;
use crate::visual::VisualState;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DisplayProfile {
    pub logical_size: [f32; 2],
    pub physical_size: [u32; 2],
    pub dpi: Option<f32>,
    pub ppi: Option<f32>,
    pub tpi: Option<f32>,
    pub scale_factor: f32,
    pub user_scale: f32,
    pub safe_area: SafeAreaInsets,
    pub window_mode: WindowMode,
    pub orientation: DisplayOrientation,
}

impl DisplayProfile {
    pub fn new(logical_width: f32, logical_height: f32) -> Self {
        Self {
            logical_size: [logical_width.max(1.0), logical_height.max(1.0)],
            physical_size: [
                logical_width.max(1.0) as u32,
                logical_height.max(1.0) as u32,
            ],
            dpi: None,
            ppi: None,
            tpi: None,
            scale_factor: 1.0,
            user_scale: 1.0,
            safe_area: SafeAreaInsets::default(),
            window_mode: WindowMode::Windowed,
            orientation: if logical_width >= logical_height {
                DisplayOrientation::Landscape
            } else {
                DisplayOrientation::Portrait
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SafeAreaInsets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowMode {
    Windowed,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DisplayOrientation {
    Landscape,
    Portrait,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StageProfile {
    pub design_size: [f32; 2],
    pub policy: StageFitPolicy,
    pub safe_area_mode: SafeAreaMode,
}

impl Default for StageProfile {
    fn default() -> Self {
        Self {
            design_size: [1280.0, 720.0],
            policy: StageFitPolicy::Contain,
            safe_area_mode: SafeAreaMode::Respect,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StageFitPolicy {
    Contain,
    Cover,
    Stretch,
    PixelPerfect,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SafeAreaMode {
    Ignore,
    Respect,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LayoutPolicy {
    pub breakpoints: Vec<LayoutBreakpoint>,
    pub density: UiDensity,
    pub min_touch_target: f32,
}

impl Default for LayoutPolicy {
    fn default() -> Self {
        Self {
            breakpoints: vec![
                LayoutBreakpoint::new("compact", 0.0, 640.0),
                LayoutBreakpoint::new("normal", 640.0, 1920.0),
                LayoutBreakpoint::new("ultrawide", 1920.0, f32::MAX),
            ],
            density: UiDensity::Comfortable,
            min_touch_target: 44.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LayoutBreakpoint {
    pub id: String,
    pub min_width: f32,
    pub max_width: f32,
}

impl LayoutBreakpoint {
    pub fn new(id: impl Into<String>, min_width: f32, max_width: f32) -> Self {
        Self {
            id: id.into(),
            min_width,
            max_width,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiDensity {
    Compact,
    Comfortable,
    Spacious,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct UiTheme {
    pub id: String,
    pub locale: Option<String>,
    pub colors: BTreeMap<String, String>,
    pub typography: BTreeMap<String, TypographyToken>,
    pub spacing: BTreeMap<String, f32>,
    pub radii: BTreeMap<String, f32>,
    pub alpha: BTreeMap<String, f32>,
    pub action_text: BTreeMap<String, String>,
    pub components: ComponentRegistry,
}

impl Default for UiTheme {
    fn default() -> Self {
        let mut colors = BTreeMap::new();
        colors.insert("stage.background".to_string(), "#101218".to_string());
        colors.insert("dialogue.background".to_string(), "#08080EDC".to_string());
        colors.insert("dialogue.text".to_string(), "#FFFFFF".to_string());
        colors.insert("dialogue.speaker".to_string(), "#9ED8FF".to_string());
        colors.insert("choice.background".to_string(), "#101827E6".to_string());
        colors.insert("choice.prompt".to_string(), "#FFFFFF".to_string());
        colors.insert("button.primary".to_string(), "#2D6CDF".to_string());
        colors.insert("button.choice".to_string(), "#263449".to_string());
        colors.insert("system.text".to_string(), "#F4D35E".to_string());
        let mut typography = BTreeMap::new();
        typography.insert(
            "dialogue".to_string(),
            TypographyToken {
                font_family: "sans".to_string(),
                size: 18.0,
                weight: 400,
                line_height: 1.3,
            },
        );
        typography.insert(
            "choice".to_string(),
            TypographyToken {
                font_family: "sans".to_string(),
                size: 16.0,
                weight: 500,
                line_height: 1.25,
            },
        );
        let mut spacing = BTreeMap::new();
        spacing.insert("panel.padding".to_string(), 24.0);
        spacing.insert("choice.gap".to_string(), 12.0);
        let mut radii = BTreeMap::new();
        radii.insert("panel".to_string(), 8.0);
        radii.insert("button".to_string(), 6.0);
        let mut alpha = BTreeMap::new();
        alpha.insert("overlay".to_string(), 0.88);
        let mut action_text = BTreeMap::new();
        action_text.insert("continue".to_string(), "Continue".to_string());
        action_text.insert("show_full".to_string(), "Show full".to_string());
        action_text.insert("resume".to_string(), "Resume".to_string());
        action_text.insert("open_routes".to_string(), "Routes".to_string());
        let mut components = ComponentRegistry::default();
        components.insert_default_game_components();
        Self {
            id: "default".to_string(),
            locale: None,
            colors,
            typography,
            spacing,
            radii,
            alpha,
            action_text,
            components,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TypographyToken {
    pub font_family: String,
    pub size: f32,
    pub weight: u16,
    pub line_height: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComponentRegistry {
    pub components: BTreeMap<String, ComponentStyle>,
}

impl ComponentRegistry {
    pub const REQUIRED_GAME_COMPONENTS: &'static [&'static str] = &[
        "dialogue_box",
        "choice_list",
        "button.primary",
        "button.choice",
        "player_menu",
        "save_slots",
        "history_panel",
        "settings_panel",
        "route_tree",
        "toast",
        "loading_overlay",
        "status_bar",
        "scene_frame_inspector",
    ];

    pub fn insert_default_game_components(&mut self) {
        self.components.insert(
            "dialogue_box".to_string(),
            ComponentStyle {
                background: Some("dialogue.background".to_string()),
                typography: Some("dialogue".to_string()),
                radius: Some("panel".to_string()),
                padding: Some([24.0, 24.0, 24.0, 24.0]),
                ..Default::default()
            },
        );
        self.components.insert(
            "choice_list".to_string(),
            ComponentStyle {
                background: Some("choice.background".to_string()),
                typography: Some("choice".to_string()),
                radius: Some("panel".to_string()),
                padding: Some([24.0, 24.0, 24.0, 24.0]),
                ..Default::default()
            },
        );
        self.components.insert(
            "button.primary".to_string(),
            ComponentStyle {
                background: Some("button.primary".to_string()),
                typography: Some("dialogue".to_string()),
                radius: Some("button".to_string()),
                padding: Some([12.0, 16.0, 12.0, 16.0]),
                ..Default::default()
            },
        );
        self.components.insert(
            "button.choice".to_string(),
            ComponentStyle {
                background: Some("button.choice".to_string()),
                typography: Some("choice".to_string()),
                radius: Some("button".to_string()),
                padding: Some([10.0, 14.0, 10.0, 14.0]),
                ..Default::default()
            },
        );
        for id in [
            "player_menu",
            "save_slots",
            "history_panel",
            "settings_panel",
            "route_tree",
            "toast",
            "loading_overlay",
            "status_bar",
            "scene_frame_inspector",
        ] {
            self.components
                .entry(id.to_string())
                .or_insert_with(|| ComponentStyle {
                    background: Some("dialogue.background".to_string()),
                    typography: Some("dialogue".to_string()),
                    radius: Some("panel".to_string()),
                    padding: Some([16.0, 16.0, 16.0, 16.0]),
                    ..Default::default()
                });
        }
    }

    pub fn missing_required_components(&self) -> Vec<String> {
        Self::REQUIRED_GAME_COMPONENTS
            .iter()
            .filter(|id| !self.components.contains_key(**id))
            .map(|id| (*id).to_string())
            .collect()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComponentStyle {
    pub layout: Option<LayoutSpec>,
    pub typography: Option<String>,
    pub background: Option<String>,
    pub border: Option<String>,
    pub radius: Option<String>,
    pub padding: Option<[f32; 4]>,
    pub states: BTreeMap<String, ComponentStyleOverride>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComponentStyleOverride {
    pub background: Option<String>,
    pub border: Option<String>,
    pub typography: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LayoutSpec {
    pub anchor: Anchor,
    pub rect: LayoutRect,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    #[default]
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LayoutResolution {
    pub display: DisplayProfile,
    pub stage: StageProfile,
    pub breakpoint: String,
    pub stage_rect: LayoutRect,
    pub scale: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiThemeValidationReport {
    pub valid: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SceneFrame {
    pub frame_schema: String,
    pub visual: VisualState,
    pub commands: Vec<RenderCommand>,
    pub interactions: Vec<InteractionSpec>,
    pub route: Option<RouteTree>,
    pub theme_id: Option<String>,
    pub layout: Option<LayoutResolution>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RenderCommand {
    Clear {
        color: String,
    },
    Image {
        asset: String,
        rect: LayoutRect,
        fit: ImageFit,
        z: i32,
    },
    Text {
        text: String,
        style: String,
        rect: LayoutRect,
    },
    Panel {
        style: String,
        rect: LayoutRect,
    },
    Button {
        id: String,
        label: String,
        style: String,
        rect: LayoutRect,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageFit {
    Contain,
    Cover,
    Stretch,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InteractionSpec {
    pub id: String,
    pub label: String,
    pub action: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiResponse {
    pub activated_actions: Vec<String>,
    pub diagnostics: Vec<String>,
}

/// Renderer-agnostic presentation contract shared by GUI, exported players and tests.
pub trait SceneFramePresenter {
    fn present(
        &mut self,
        frame: &SceneFrame,
        display: &DisplayProfile,
        theme: &UiTheme,
    ) -> UiResponse;
}

#[derive(Clone, Debug, Default)]
pub struct HeadlessSceneFramePresenter {
    pub last_layout: Option<LayoutResolution>,
}

impl SceneFramePresenter for HeadlessSceneFramePresenter {
    fn present(
        &mut self,
        frame: &SceneFrame,
        display: &DisplayProfile,
        theme: &UiTheme,
    ) -> UiResponse {
        let report = validate_ui_theme(theme);
        let mut diagnostics = report.warnings;
        diagnostics.extend(report.errors);
        self.last_layout = frame.layout.clone().or_else(|| {
            Some(resolve_layout(
                display.clone(),
                StageProfile::default(),
                LayoutPolicy::default(),
            ))
        });
        UiResponse {
            activated_actions: Vec::new(),
            diagnostics,
        }
    }
}

pub fn validate_ui_theme(theme: &UiTheme) -> UiThemeValidationReport {
    let mut report = UiThemeValidationReport {
        valid: true,
        warnings: Vec::new(),
        errors: Vec::new(),
    };
    if theme.id.trim().is_empty() {
        report.errors.push("theme id must not be empty".to_string());
    }
    for (key, value) in &theme.colors {
        if !value.starts_with('#') || !(value.len() == 7 || value.len() == 9) {
            report
                .errors
                .push(format!("color token '{key}' must be #RRGGBB or #RRGGBBAA"));
        }
    }
    for (key, value) in &theme.spacing {
        if !value.is_finite() || *value < 0.0 {
            report.errors.push(format!(
                "spacing token '{key}' must be finite and non-negative"
            ));
        }
    }
    for id in theme.components.missing_required_components() {
        report.warnings.push(format!(
            "component registry missing required component '{id}'"
        ));
    }
    report.valid = report.errors.is_empty();
    report
}

pub fn resolve_layout(
    display: DisplayProfile,
    stage: StageProfile,
    policy: LayoutPolicy,
) -> LayoutResolution {
    let breakpoint = policy
        .breakpoints
        .iter()
        .find(|bp| {
            display.logical_size[0] >= bp.min_width && display.logical_size[0] < bp.max_width
        })
        .map(|bp| bp.id.clone())
        .unwrap_or_else(|| "default".to_string());
    let available_width =
        (display.logical_size[0] - display.safe_area.left - display.safe_area.right).max(1.0);
    let available_height =
        (display.logical_size[1] - display.safe_area.top - display.safe_area.bottom).max(1.0);
    let sx = available_width / stage.design_size[0].max(1.0);
    let sy = available_height / stage.design_size[1].max(1.0);
    let scale = match stage.policy {
        StageFitPolicy::Contain | StageFitPolicy::PixelPerfect => sx.min(sy),
        StageFitPolicy::Cover => sx.max(sy),
        StageFitPolicy::Stretch => 1.0,
    } * display.user_scale.max(0.1);
    let width = match stage.policy {
        StageFitPolicy::Stretch => available_width,
        _ => stage.design_size[0] * scale,
    };
    let height = match stage.policy {
        StageFitPolicy::Stretch => available_height,
        _ => stage.design_size[1] * scale,
    };
    let stage_rect = LayoutRect {
        x: display.safe_area.left + (available_width - width) * 0.5,
        y: display.safe_area.top + (available_height - height) * 0.5,
        width,
        height,
    };
    LayoutResolution {
        display,
        stage,
        breakpoint,
        stage_rect,
        scale,
    }
}
