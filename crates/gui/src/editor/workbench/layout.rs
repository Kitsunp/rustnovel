use super::LayoutOverrides;

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
    let compact = width < 900.0;
    let medium = (900.0..1400.0).contains(&width);

    let asset_min = if compact { 92.0 } else { 130.0 };
    let graph_min = if compact { 150.0 } else { 220.0 };
    let inspector_min = if compact { 150.0 } else { 210.0 };
    let central_min = if compact {
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
        layout.central_min = (total_width - min_side_width).clamp(160.0, layout.central_min);
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
mod tests {
    use super::*;

    #[test]
    fn compact_layout_uses_smaller_panel_minimums() {
        let layout = editor_panel_layout(720.0, 480.0, &LayoutOverrides::default());
        assert!(layout.asset_browser.min < 150.0);
        assert!(layout.graph.min < 240.0);
        assert!(layout.inspector.min < 220.0);
        assert!(layout.timeline.max <= 360.0);
    }

    #[test]
    fn validation_panel_can_collapse_to_toolbar_height() {
        let overrides = LayoutOverrides::default();
        let collapsed = validation_panel_layout(480.0, true, &overrides);
        let expanded = validation_panel_layout(480.0, false, &overrides);
        assert!(collapsed.default < expanded.default);
        assert!(expanded.min <= 34.0);
    }

    #[test]
    fn default_panel_maxima_leave_room_for_composer() {
        let width = 1280.0;
        let layout = editor_panel_layout(width, 720.0, &LayoutOverrides::default());
        let side_max = layout.asset_browser.max + layout.graph.max + layout.inspector.max;
        assert!(
            side_max + layout.central_min <= width,
            "side panels should not consume the composer area"
        );
    }

    #[test]
    fn validation_height_override_raises_default_height() {
        let overrides = LayoutOverrides {
            validation_height: Some(420.0),
            ..Default::default()
        };
        let layout = validation_panel_layout(720.0, false, &overrides);
        assert!(layout.default >= 400.0);
    }

    #[test]
    fn oversized_width_overrides_keep_composer_visible() {
        let overrides = LayoutOverrides {
            asset_width: Some(288.0),
            graph_width: Some(590.0),
            inspector_width: Some(242.0),
            ..Default::default()
        };
        let width = 1272.0;
        let layout = editor_panel_layout(width, 720.0, &overrides);
        let side_default =
            layout.asset_browser.default + layout.graph.default + layout.inspector.default;
        assert!(
            side_default + layout.central_min <= width,
            "stored panel sizes must not squeeze the visual composer below its minimum"
        );
        assert!(layout.graph.default < 590.0);
    }

    #[test]
    fn medium_window_uses_fresh_panel_ids_and_wider_composer_budget() {
        let layout = editor_panel_layout(1272.0, 720.0, &LayoutOverrides::default());
        assert_eq!(layout.id_suffix, "medium");
        assert!(layout.central_min >= 520.0);
        assert!(
            layout.asset_browser.max + layout.graph.max + layout.inspector.max + layout.central_min
                <= 1272.0
        );
    }

    #[test]
    fn each_panel_maximum_preserves_central_budget() {
        let width = 1272.0;
        let layout = editor_panel_layout(width, 720.0, &LayoutOverrides::default());

        assert!(
            layout.asset_browser.max + layout.graph.min + layout.inspector.min + layout.central_min
                <= width
        );
        assert!(
            layout.graph.max + layout.asset_browser.min + layout.inspector.min + layout.central_min
                <= width
        );
        assert!(
            layout.inspector.max + layout.asset_browser.min + layout.graph.min + layout.central_min
                <= width
        );
    }
}
