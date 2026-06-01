use super::LayoutOverrides;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[path = "layout/sizing.rs"]
mod sizing;
#[path = "layout/workspace.rs"]
mod workspace;

use sizing::*;
pub use workspace::*;

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
