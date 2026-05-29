use super::LayoutOverrides;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const WORKSPACE_LAYOUT_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_DOCK_REFERENCE_WIDTH: f32 = 1280.0;
pub const TOOLBAR_EXPANDED_MIN_WIDTH: f32 = 2400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorToolbarMode {
    Grouped,
    Expanded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationReportPlacement {
    Inspector,
    FloatingWindow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WorkspacePanelId {
    AssetBrowser,
    Graph,
    Composer,
    Inspector,
    Validation,
    Timeline,
    NodeEditor,
}

impl WorkspacePanelId {
    pub const ALL: [Self; 7] = [
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
pub enum WorkspacePanelPlacement {
    Left,
    Center,
    Right,
    Bottom,
    Floating,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspacePanelRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspacePanelState {
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

pub fn editor_toolbar_mode(available_width: f32) -> EditorToolbarMode {
    if available_width >= TOOLBAR_EXPANDED_MIN_WIDTH {
        EditorToolbarMode::Expanded
    } else {
        EditorToolbarMode::Grouped
    }
}

pub fn validation_report_placement(inspector_visible: bool) -> ValidationReportPlacement {
    if inspector_visible {
        ValidationReportPlacement::Inspector
    } else {
        ValidationReportPlacement::FloatingWindow
    }
}

pub fn validation_report_body_height(available_height: f32, issue_count: usize) -> f32 {
    if issue_count == 0 {
        return 28.0;
    }
    (available_height * 0.42).clamp(120.0, 360.0)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceLayout {
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
    pub fn normalize(&mut self) {
        self.schema_version = WORKSPACE_LAYOUT_SCHEMA_VERSION;
        for (idx, id) in WorkspacePanelId::ALL.into_iter().enumerate() {
            self.panels
                .entry(id)
                .or_insert_with(|| WorkspacePanelState::default_for(id, idx as i32));
        }
    }

    pub fn panel(&self, id: WorkspacePanelId) -> WorkspacePanelState {
        self.panels
            .get(&id)
            .cloned()
            .unwrap_or_else(|| WorkspacePanelState::default_for(id, id as i32))
    }

    pub fn set_visible(&mut self, id: WorkspacePanelId, visible: bool) {
        self.normalize();
        if let Some(panel) = self.panels.get_mut(&id) {
            panel.visible = visible;
        }
    }

    pub fn set_collapsed(&mut self, id: WorkspacePanelId, collapsed: bool) {
        self.normalize();
        if let Some(panel) = self.panels.get_mut(&id) {
            panel.collapsed = collapsed;
        }
    }

    pub fn set_floating_rect(&mut self, id: WorkspacePanelId, rect: WorkspacePanelRect) {
        self.normalize();
        if let Some(panel) = self.panels.get_mut(&id) {
            panel.placement = WorkspacePanelPlacement::Floating;
            panel.floating_rect = Some(rect);
            panel.visible = true;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelSize {
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorPanelLayout {
    pub asset_browser: PanelSize,
    pub inspector: PanelSize,
    pub graph: PanelSize,
    pub timeline: PanelSize,
    pub central_min: f32,
    pub id_suffix: &'static str,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EditorDockVisibility {
    pub asset_browser: bool,
    pub graph: bool,
    pub inspector: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorDockLayout {
    pub asset_browser: Option<WorkspacePanelRect>,
    pub graph: Option<WorkspacePanelRect>,
    pub composer: WorkspacePanelRect,
    pub inspector: Option<WorkspacePanelRect>,
    pub asset_splitter: Option<WorkspacePanelRect>,
    pub graph_splitter: Option<WorkspacePanelRect>,
    pub inspector_splitter: Option<WorkspacePanelRect>,
    pub panel_sizes: EditorPanelLayout,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValidationPanelLayout {
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

pub fn editor_panel_layout(
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
        (width * 0.34).clamp(400.0, 520.0)
    } else {
        (width * 0.34).clamp(480.0, 1440.0)
    };
    let id_suffix = if compact {
        "compact"
    } else if medium {
        "medium"
    } else {
        "wide"
    };

    let dock_reference_width = dock_reference_width(overrides);
    let mut layout = EditorPanelLayout {
        asset_browser: apply_width_override(
            panel_size(width, asset_min, 0.11, 0.14),
            overrides.asset_width,
            width,
            dock_reference_width,
        ),
        graph: apply_width_override(
            panel_size(width, graph_min, 0.27, 0.32),
            overrides.graph_width,
            width,
            dock_reference_width,
        ),
        inspector: apply_width_override(
            panel_size(width, inspector_min, 0.19, 0.24),
            overrides.inspector_width,
            width,
            dock_reference_width,
        ),
        timeline: apply_height_override(
            PanelSize {
                min: if compact { 72.0 } else { 96.0 },
                default: (height * 0.22).clamp(96.0, 180.0),
                max: (height * 0.42).clamp(140.0, 320.0),
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

pub fn resolve_editor_dock_layout(
    available_width: f32,
    available_height: f32,
    overrides: &LayoutOverrides,
    visibility: EditorDockVisibility,
) -> EditorDockLayout {
    const SPLITTER: f32 = 10.0;

    let width = available_width.max(1.0);
    let height = available_height.max(1.0);
    let splitter_count = [
        visibility.asset_browser,
        visibility.graph,
        visibility.inspector,
    ]
    .into_iter()
    .filter(|visible| *visible)
    .count() as f32;
    let content_width = (width - splitter_count * SPLITTER).max(1.0);
    let panel_sizes = editor_panel_layout(content_width, height, overrides);

    let asset_min = if visibility.asset_browser {
        panel_sizes.asset_browser.min
    } else {
        0.0
    };
    let graph_min = if visibility.graph {
        panel_sizes.graph.min
    } else {
        0.0
    };
    let inspector_min = if visibility.inspector {
        panel_sizes.inspector.min
    } else {
        0.0
    };
    let min_side = asset_min + graph_min + inspector_min;
    let central_min = panel_sizes
        .central_min
        .min((content_width - min_side).max(96.0))
        .max(0.0);
    let side_budget = (content_width - central_min).max(0.0);

    let mut asset_w = if visibility.asset_browser {
        panel_sizes.asset_browser.default
    } else {
        0.0
    };
    let mut graph_w = if visibility.graph {
        panel_sizes.graph.default
    } else {
        0.0
    };
    let mut inspector_w = if visibility.inspector {
        panel_sizes.inspector.default
    } else {
        0.0
    };

    let mut side_widths = [asset_w, graph_w, inspector_w];
    shrink_widths_proportionally(
        &mut side_widths,
        [asset_min, graph_min, inspector_min],
        side_budget,
    );
    asset_w = side_widths[0];
    graph_w = side_widths[1];
    inspector_w = side_widths[2];

    let composer_w = (content_width - asset_w - graph_w - inspector_w).max(1.0);
    let mut x = 0.0;
    let asset_browser = visibility.asset_browser.then(|| {
        let rect = WorkspacePanelRect {
            x,
            y: 0.0,
            w: asset_w,
            h: height,
        };
        x += asset_w;
        rect
    });
    let asset_splitter = visibility.asset_browser.then(|| {
        let rect = WorkspacePanelRect {
            x,
            y: 0.0,
            w: SPLITTER,
            h: height,
        };
        x += SPLITTER;
        rect
    });
    let graph = visibility.graph.then(|| {
        let rect = WorkspacePanelRect {
            x,
            y: 0.0,
            w: graph_w,
            h: height,
        };
        x += graph_w;
        rect
    });
    let graph_splitter = visibility.graph.then(|| {
        let rect = WorkspacePanelRect {
            x,
            y: 0.0,
            w: SPLITTER,
            h: height,
        };
        x += SPLITTER;
        rect
    });
    let composer = WorkspacePanelRect {
        x,
        y: 0.0,
        w: composer_w,
        h: height,
    };
    x += composer_w;
    let inspector_splitter = visibility.inspector.then(|| {
        let rect = WorkspacePanelRect {
            x,
            y: 0.0,
            w: SPLITTER,
            h: height,
        };
        x += SPLITTER;
        rect
    });
    let inspector = visibility.inspector.then_some(WorkspacePanelRect {
        x,
        y: 0.0,
        w: inspector_w,
        h: height,
    });

    EditorDockLayout {
        asset_browser,
        graph,
        composer,
        inspector,
        asset_splitter,
        graph_splitter,
        inspector_splitter,
        panel_sizes,
    }
}

pub fn validation_panel_layout(
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

    let automatic_default = (height * 0.14).clamp(84.0, 140.0);
    let max = (height * 0.58).clamp(180.0, 720.0);
    let default = overrides
        .validation_height
        .unwrap_or(automatic_default)
        .clamp(80.0, max);
    ValidationPanelLayout {
        min: 34.0,
        default,
        max,
    }
}

pub fn timeline_panel_layout(base: PanelSize, _validation_visible: bool) -> PanelSize {
    base
}

pub fn dragged_panel_override(
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

pub fn observed_panel_override(
    current: Option<f32>,
    measured: f32,
    adaptive_default: f32,
    min: f32,
    max: f32,
) -> Option<f32> {
    if !measured.is_finite() {
        return current;
    }
    let next = measured.clamp(min, max);
    if let Some(value) = current {
        if (value - next).abs() < 1.0 {
            current
        } else {
            Some(next)
        }
    } else if (adaptive_default - next).abs() >= 1.0 {
        Some(next)
    } else {
        None
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
    reference_width: f32,
) -> PanelSize {
    if let Some(width) = override_width {
        let width = scaled_width_override(width, total_width, reference_width)
            .clamp(size.min, total_width * 0.48);
        size.default = width;
        size.max = size.max.max(width).min(total_width * 0.52);
    }
    size
}

fn dock_reference_width(overrides: &LayoutOverrides) -> f32 {
    overrides
        .dock_reference_width
        .filter(|width| width.is_finite() && *width >= 360.0)
        .unwrap_or(DEFAULT_DOCK_REFERENCE_WIDTH)
}

fn scaled_width_override(width: f32, total_width: f32, reference_width: f32) -> f32 {
    if !width.is_finite() {
        return width;
    }
    let reference_width = reference_width.max(360.0);
    width * (total_width.max(1.0) / reference_width)
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
    let mut widths = [
        layout.asset_browser.default,
        layout.graph.default,
        layout.inspector.default,
    ];
    shrink_widths_proportionally(
        &mut widths,
        [
            layout.asset_browser.min,
            layout.graph.min,
            layout.inspector.min,
        ],
        side_budget,
    );
    layout.asset_browser.default = widths[0];
    layout.graph.default = widths[1];
    layout.inspector.default = widths[2];
}

fn shrink_widths_proportionally(widths: &mut [f32; 3], minimums: [f32; 3], budget: f32) {
    let mut overflow = (widths.iter().sum::<f32>() - budget).max(0.0);
    while overflow > 0.1 {
        let removable = [
            (widths[0] - minimums[0]).max(0.0),
            (widths[1] - minimums[1]).max(0.0),
            (widths[2] - minimums[2]).max(0.0),
        ];
        let removable_total = removable.iter().sum::<f32>();
        if removable_total <= 0.1 {
            break;
        }

        let mut removed_total = 0.0;
        for index in 0..widths.len() {
            if removable[index] <= 0.0 {
                continue;
            }
            let share = overflow * (removable[index] / removable_total);
            let removed = share.min(removable[index]);
            widths[index] -= removed;
            removed_total += removed;
        }
        if removed_total <= 0.1 {
            break;
        }
        overflow -= removed_total;
    }
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
    let mut maxima = [
        layout.asset_browser.max,
        layout.graph.max,
        layout.inspector.max,
    ];
    shrink_widths_proportionally(
        &mut maxima,
        [
            layout.asset_browser.min,
            layout.graph.min,
            layout.inspector.min,
        ],
        side_budget,
    );
    layout.asset_browser.max = maxima[0];
    layout.graph.max = maxima[1];
    layout.inspector.max = maxima[2];

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
