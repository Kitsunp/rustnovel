use super::LayoutOverrides;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const WORKSPACE_LAYOUT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum WorkspacePanelId {
    AssetBrowser,
    Graph,
    Composer,
    Inspector,
    Validation,
    Timeline,
    NodeEditor,
}

impl WorkspacePanelId {
    pub(crate) const ALL: [Self; 7] = [
        Self::AssetBrowser,
        Self::Graph,
        Self::Composer,
        Self::Inspector,
        Self::Validation,
        Self::Timeline,
        Self::NodeEditor,
    ];

    fn default_placement(self) -> WorkspacePanelPlacement {
        match self {
            Self::AssetBrowser | Self::Graph => WorkspacePanelPlacement::Left,
            Self::Composer => WorkspacePanelPlacement::Center,
            Self::Inspector => WorkspacePanelPlacement::Right,
            Self::Validation | Self::Timeline => WorkspacePanelPlacement::Bottom,
            Self::NodeEditor => WorkspacePanelPlacement::Floating,
        }
    }

    fn visible_by_default(self) -> bool {
        !matches!(self, Self::Validation | Self::NodeEditor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum WorkspacePanelPlacement {
    Left,
    Center,
    Right,
    Bottom,
    Floating,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct WorkspacePanelRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct WorkspacePanelState {
    pub id: WorkspacePanelId,
    pub placement: WorkspacePanelPlacement,
    pub visible: bool,
    pub collapsed: bool,
    pub floating_rect: Option<WorkspacePanelRect>,
    pub split_ratio: f32,
    pub min_size: f32,
    pub max_size: f32,
    pub last_size: Option<f32>,
    pub z_order: i32,
}

impl WorkspacePanelState {
    fn default_for(id: WorkspacePanelId, z_order: i32) -> Self {
        let placement = id.default_placement();
        let (split_ratio, min_size, max_size) = match id {
            WorkspacePanelId::AssetBrowser => (0.12, 92.0, 420.0),
            WorkspacePanelId::Graph => (0.30, 150.0, 760.0),
            WorkspacePanelId::Composer => (0.58, 320.0, 4096.0),
            WorkspacePanelId::Inspector => (0.20, 150.0, 520.0),
            WorkspacePanelId::Validation => (0.30, 34.0, 720.0),
            WorkspacePanelId::Timeline => (0.26, 72.0, 420.0),
            WorkspacePanelId::NodeEditor => (0.50, 320.0, 4096.0),
        };
        Self {
            id,
            placement,
            visible: id.visible_by_default(),
            collapsed: false,
            floating_rect: None,
            split_ratio,
            min_size,
            max_size,
            last_size: None,
            z_order,
        }
    }
}

fn workspace_layout_schema_version() -> u32 {
    WORKSPACE_LAYOUT_SCHEMA_VERSION
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct WorkspaceLayout {
    #[serde(default = "workspace_layout_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub panels: BTreeMap<WorkspacePanelId, WorkspacePanelState>,
}

impl Default for WorkspaceLayout {
    fn default() -> Self {
        let mut panels = BTreeMap::new();
        for (idx, id) in WorkspacePanelId::ALL.into_iter().enumerate() {
            panels.insert(id, WorkspacePanelState::default_for(id, idx as i32));
        }
        Self {
            schema_version: WORKSPACE_LAYOUT_SCHEMA_VERSION,
            panels,
        }
    }
}

impl WorkspaceLayout {
    pub(crate) fn normalize(&mut self) {
        self.schema_version = WORKSPACE_LAYOUT_SCHEMA_VERSION;
        for (idx, id) in WorkspacePanelId::ALL.into_iter().enumerate() {
            self.panels
                .entry(id)
                .or_insert_with(|| WorkspacePanelState::default_for(id, idx as i32));
        }
    }

    pub(crate) fn panel(&self, id: WorkspacePanelId) -> WorkspacePanelState {
        self.panels
            .get(&id)
            .cloned()
            .unwrap_or_else(|| WorkspacePanelState::default_for(id, id as i32))
    }

    pub(crate) fn set_visible(&mut self, id: WorkspacePanelId, visible: bool) {
        self.normalize();
        if let Some(panel) = self.panels.get_mut(&id) {
            panel.visible = visible;
        }
    }

    pub(crate) fn set_collapsed(&mut self, id: WorkspacePanelId, collapsed: bool) {
        self.normalize();
        if let Some(panel) = self.panels.get_mut(&id) {
            panel.collapsed = collapsed;
        }
    }

    pub(crate) fn set_floating_rect(&mut self, id: WorkspacePanelId, rect: WorkspacePanelRect) {
        self.normalize();
        if let Some(panel) = self.panels.get_mut(&id) {
            panel.placement = WorkspacePanelPlacement::Floating;
            panel.floating_rect = Some(rect);
            panel.visible = true;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PanelSize {
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct EditorPanelLayout {
    pub asset_browser: PanelSize,
    pub inspector: PanelSize,
    pub graph: PanelSize,
    pub timeline: PanelSize,
    pub central_min: f32,
    pub id_suffix: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ValidationPanelLayout {
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

pub(super) fn editor_panel_layout(
    available_width: f32,
    available_height: f32,
    overrides: &LayoutOverrides,
) -> EditorPanelLayout {
    let width = available_width.max(360.0);
    let height = available_height.max(280.0);
    let narrow = width < 560.0;
    let compact = width < 900.0;
    let medium = (900.0..1400.0).contains(&width);

    let asset_min = if narrow {
        (width * 0.15).clamp(48.0, 72.0)
    } else if compact {
        92.0
    } else {
        130.0
    };
    let graph_min = if narrow {
        (width * 0.24).clamp(84.0, 120.0)
    } else if compact {
        150.0
    } else {
        220.0
    };
    let inspector_min = if narrow {
        (width * 0.24).clamp(84.0, 120.0)
    } else if compact {
        150.0
    } else {
        210.0
    };
    let central_min = if narrow {
        (width - asset_min - graph_min - inspector_min).clamp(96.0, 220.0)
    } else if compact {
        260.0
    } else if medium {
        520.0
    } else {
        560.0
    };
    let id_suffix = if compact {
        "compact"
    } else if medium {
        "medium"
    } else {
        "wide"
    };

    let mut layout = EditorPanelLayout {
        asset_browser: apply_width_override(
            panel_size(width, asset_min, 0.11, 0.14),
            overrides.asset_width,
            width,
        ),
        graph: apply_width_override(
            panel_size(width, graph_min, 0.27, 0.32),
            overrides.graph_width,
            width,
        ),
        inspector: apply_width_override(
            panel_size(width, inspector_min, 0.19, 0.24),
            overrides.inspector_width,
            width,
        ),
        timeline: apply_height_override(
            PanelSize {
                min: if compact { 72.0 } else { 96.0 },
                default: (height * 0.26).clamp(120.0, 220.0),
                max: (height * 0.50).clamp(160.0, 360.0),
            },
            overrides.timeline_height,
            height,
        ),
        central_min,
        id_suffix,
    };
    fit_side_panels_to_central_budget(&mut layout, width);
    layout
}

pub(super) fn validation_panel_layout(
    available_height: f32,
    collapsed: bool,
    overrides: &LayoutOverrides,
) -> ValidationPanelLayout {
    let height = available_height.max(220.0);
    if collapsed {
        return ValidationPanelLayout {
            min: 28.0,
            default: 34.0,
            max: 48.0,
        };
    }

    let default = overrides
        .validation_height
        .unwrap_or_else(|| (height * 0.30).clamp(110.0, 240.0))
        .clamp(80.0, height * 0.82);
    ValidationPanelLayout {
        min: 34.0,
        default,
        max: (height * 0.82).clamp(180.0, 720.0),
    }
}

pub(super) fn timeline_panel_layout(base: PanelSize, stacked_with_validation: bool) -> PanelSize {
    if !stacked_with_validation {
        return base;
    }
    PanelSize {
        min: base.min.min(56.0),
        default: base.default.min(96.0).max(base.min.min(56.0)),
        max: base.max.min(160.0).max(base.min.min(56.0)),
    }
}

pub(super) fn dragged_panel_override(
    current: Option<f32>,
    measured: f32,
    min: f32,
    max: f32,
    dragged: bool,
) -> Option<f32> {
    if !dragged || !measured.is_finite() {
        return current;
    }
    let next = measured.clamp(min, max);
    if current.is_some_and(|value| (value - next).abs() < 1.0) {
        current
    } else {
        Some(next)
    }
}

fn panel_size(width: f32, min: f32, default_ratio: f32, max_ratio: f32) -> PanelSize {
    let max = (width * max_ratio).max(min);
    PanelSize {
        min,
        default: (width * default_ratio).clamp(min, max),
        max,
    }
}

fn apply_width_override(
    mut size: PanelSize,
    override_width: Option<f32>,
    total_width: f32,
) -> PanelSize {
    if let Some(width) = override_width {
        let width = width.clamp(size.min, total_width * 0.48);
        size.default = width;
        size.max = size.max.max(width).min(total_width * 0.52);
    }
    size
}

fn apply_height_override(
    mut size: PanelSize,
    override_height: Option<f32>,
    total_height: f32,
) -> PanelSize {
    if let Some(height) = override_height {
        let height = height.clamp(size.min, total_height * 0.70);
        size.default = height;
        size.max = size.max.max(height).min(total_height * 0.78);
    }
    size
}

fn fit_side_panels_to_central_budget(layout: &mut EditorPanelLayout, total_width: f32) {
    let min_side_width = layout.asset_browser.min + layout.graph.min + layout.inspector.min;
    if min_side_width + layout.central_min > total_width {
        let central_floor = if total_width < 560.0 { 96.0 } else { 160.0 };
        layout.central_min =
            (total_width - min_side_width).clamp(central_floor, layout.central_min);
    }

    let side_budget = (total_width - layout.central_min).max(min_side_width);
    shrink_defaults_to_budget(layout, side_budget);
    clamp_maxima_to_budget(layout, side_budget);
}

fn shrink_defaults_to_budget(layout: &mut EditorPanelLayout, side_budget: f32) {
    let total_default =
        layout.asset_browser.default + layout.graph.default + layout.inspector.default;
    let mut overflow = (total_default - side_budget).max(0.0);
    overflow = shrink_panel_default(&mut layout.graph, overflow);
    overflow = shrink_panel_default(&mut layout.asset_browser, overflow);
    let _ = shrink_panel_default(&mut layout.inspector, overflow);
}

fn shrink_panel_default(panel: &mut PanelSize, overflow: f32) -> f32 {
    if overflow <= 0.0 {
        return 0.0;
    }
    let removable = (panel.default - panel.min).max(0.0);
    let removed = removable.min(overflow);
    panel.default -= removed;
    overflow - removed
}

fn clamp_maxima_to_budget(layout: &mut EditorPanelLayout, side_budget: f32) {
    layout.asset_browser.max = panel_max_with_other_minimums(
        side_budget,
        layout.asset_browser.min,
        layout.graph.min + layout.inspector.min,
    )
    .min(layout.asset_browser.max);
    layout.graph.max = panel_max_with_other_minimums(
        side_budget,
        layout.graph.min,
        layout.asset_browser.min + layout.inspector.min,
    )
    .min(layout.graph.max);
    layout.inspector.max = panel_max_with_other_minimums(
        side_budget,
        layout.inspector.min,
        layout.asset_browser.min + layout.graph.min,
    )
    .min(layout.inspector.max);

    layout.asset_browser.default = layout
        .asset_browser
        .default
        .clamp(layout.asset_browser.min, layout.asset_browser.max);
    layout.graph.default = layout
        .graph
        .default
        .clamp(layout.graph.min, layout.graph.max);
    layout.inspector.default = layout
        .inspector
        .default
        .clamp(layout.inspector.min, layout.inspector.max);
    shrink_maxima_to_budget(layout, side_budget);
}

fn panel_max_with_other_minimums(side_budget: f32, own_min: f32, other_mins: f32) -> f32 {
    (side_budget - other_mins).max(own_min)
}

fn shrink_maxima_to_budget(layout: &mut EditorPanelLayout, side_budget: f32) {
    let total_max = layout.asset_browser.max + layout.graph.max + layout.inspector.max;
    let mut overflow = (total_max - side_budget).max(0.0);
    overflow = shrink_panel_max(&mut layout.graph, overflow);
    overflow = shrink_panel_max(&mut layout.inspector, overflow);
    let _ = shrink_panel_max(&mut layout.asset_browser, overflow);

    layout.asset_browser.default = layout
        .asset_browser
        .default
        .clamp(layout.asset_browser.min, layout.asset_browser.max);
    layout.graph.default = layout
        .graph
        .default
        .clamp(layout.graph.min, layout.graph.max);
    layout.inspector.default = layout
        .inspector
        .default
        .clamp(layout.inspector.min, layout.inspector.max);
}

fn shrink_panel_max(panel: &mut PanelSize, overflow: f32) -> f32 {
    if overflow <= 0.0 {
        return 0.0;
    }
    let removable = (panel.max - panel.min).max(0.0);
    let removed = removable.min(overflow);
    panel.max -= removed;
    overflow - removed
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
