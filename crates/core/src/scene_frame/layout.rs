use super::{
    DisplayProfile, LayoutPolicy, LayoutRect, LayoutResolution, StageFitPolicy, StageProfile,
    UiTheme, UiThemeValidationReport,
};

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
        if let Err(err) = super::parse_theme_color_code(value) {
            report.errors.push(format!(
                "color token '{key}' must be #RRGGBB or #RRGGBBAA: {err}"
            ));
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
