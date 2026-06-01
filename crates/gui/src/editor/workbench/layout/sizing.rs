use super::*;

pub(super) fn panel_size(width: f32, min: f32, default_ratio: f32, max_ratio: f32) -> PanelSize {
    let max = (width * max_ratio).max(min);
    PanelSize {
        min,
        default: (width * default_ratio).clamp(min, max),
        max,
    }
}

pub(super) fn apply_width_override(
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

pub(super) fn dock_reference_width(overrides: &LayoutOverrides) -> f32 {
    overrides
        .dock_reference_width
        .filter(|width| width.is_finite() && *width >= 360.0)
        .unwrap_or(DEFAULT_DOCK_REFERENCE_WIDTH)
}

pub(super) fn scaled_width_override(width: f32, total_width: f32, reference_width: f32) -> f32 {
    if !width.is_finite() {
        return width;
    }
    let reference_width = reference_width.max(360.0);
    width * (total_width.max(1.0) / reference_width)
}

pub(super) fn apply_height_override(
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

pub(super) fn fit_side_panels_to_central_budget(layout: &mut EditorPanelLayout, total_width: f32) {
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

pub(super) fn shrink_defaults_to_budget(layout: &mut EditorPanelLayout, side_budget: f32) {
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

pub(super) fn shrink_widths_proportionally(widths: &mut [f32; 3], minimums: [f32; 3], budget: f32) {
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

pub(super) fn clamp_maxima_to_budget(layout: &mut EditorPanelLayout, side_budget: f32) {
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

pub(super) fn panel_max_with_other_minimums(
    side_budget: f32,
    own_min: f32,
    other_mins: f32,
) -> f32 {
    (side_budget - other_mins).max(own_min)
}

pub(super) fn shrink_maxima_to_budget(layout: &mut EditorPanelLayout, side_budget: f32) {
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
