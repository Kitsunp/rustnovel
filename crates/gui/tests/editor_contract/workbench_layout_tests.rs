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
fn compact_window_minimums_never_exceed_available_width() {
    for width in [360.0, 420.0, 520.0, 720.0] {
        let layout = editor_panel_layout(width, 480.0, &LayoutOverrides::default());
        let required_min =
            layout.asset_browser.min + layout.graph.min + layout.inspector.min + layout.central_min;
        assert!(
            required_min <= width,
            "layout minimums require {required_min}px in a {width}px window"
        );
    }
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
fn timeline_keeps_its_bottom_budget_when_validation_report_is_visible() {
    let base = editor_panel_layout(1280.0, 720.0, &LayoutOverrides::default()).timeline;
    let with_validation = timeline_panel_layout(base, true);
    assert_eq!(with_validation, base);
    assert_eq!(timeline_panel_layout(base, false), base);
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
fn dragged_panel_override_records_only_real_resize_gestures() {
    assert_eq!(
        dragged_panel_override(None, 260.0, 100.0, 400.0, false),
        None,
        "passive layout should remain adaptive until the user drags a splitter"
    );
    assert_eq!(
        dragged_panel_override(None, 620.0, 100.0, 400.0, true),
        Some(400.0),
        "dragged sizes are clamped to the panel budget"
    );
    assert_eq!(
        dragged_panel_override(Some(220.25), 220.75, 100.0, 400.0, true),
        Some(220.25),
        "sub-pixel jitter should not dirty persisted layout prefs"
    );
}

#[test]
fn observed_panel_override_records_resizes_without_panel_drag_flag() {
    assert_eq!(
        observed_panel_override(None, 260.0, 260.25, 100.0, 400.0),
        None,
        "adaptive defaults should stay adaptive when the observed panel size matches the resolver"
    );
    assert_eq!(
        observed_panel_override(None, 340.0, 260.0, 100.0, 400.0),
        Some(340.0),
        "egui splitter state must be captured even when the panel body response is not dragged"
    );
    assert_eq!(
        observed_panel_override(Some(340.0), 520.0, 260.0, 100.0, 400.0),
        Some(400.0),
        "manual panel sizes must still be clamped to the dynamic layout budget"
    );
}

#[test]
fn screenshot_viewports_keep_composer_budget_after_manual_panel_sizes() {
    let overrides = LayoutOverrides {
        dock_reference_width: Some(1275.0),
        asset_width: Some(420.0),
        graph_width: Some(760.0),
        inspector_width: Some(520.0),
        ..Default::default()
    };
    for (width, height) in [(1275.0, 746.0), (1919.0, 1079.0)] {
        let layout = resolve_editor_dock_layout(
            width,
            height,
            &overrides,
            EditorDockVisibility {
                asset_browser: true,
                graph: true,
                inspector: true,
            },
        );
        let side_default = layout.asset_browser.unwrap().w
            + layout.graph.unwrap().w
            + layout.inspector.unwrap().w;
        assert!(
            side_default + layout.composer.w <= width,
            "resolved docked panels must fit viewport {width}x{height}"
        );
        assert!(
            layout.composer.w + 1.0 >= layout.panel_sizes.central_min.min(width),
            "composer should receive a positive negotiated budget"
        );
        assert!(layout.asset_browser.unwrap().x < layout.graph.unwrap().x);
        assert!(layout.graph.unwrap().x < layout.composer.x);
        assert!(layout.composer.x < layout.inspector.unwrap().x);
        assert!(layout.inspector.unwrap().x + layout.inspector.unwrap().w <= width);
    }
}

#[test]
fn default_window_and_fullscreen_keep_similar_dock_proportions() {
    let window = resolve_editor_dock_layout(
        1275.0,
        746.0,
        &LayoutOverrides::default(),
        EditorDockVisibility {
            asset_browser: true,
            graph: true,
            inspector: true,
        },
    );
    let fullscreen = resolve_editor_dock_layout(
        1919.0,
        1079.0,
        &LayoutOverrides::default(),
        EditorDockVisibility {
            asset_browser: true,
            graph: true,
            inspector: true,
        },
    );

    assert_dock_ratios_close(window, fullscreen, 0.025);
}

#[test]
fn manual_window_splitter_sizes_scale_to_fullscreen_proportionally() {
    let overrides = LayoutOverrides {
        dock_reference_width: Some(1275.0),
        asset_width: Some(132.0),
        graph_width: Some(260.0),
        inspector_width: Some(242.0),
        ..Default::default()
    };
    let window = resolve_editor_dock_layout(
        1275.0,
        746.0,
        &overrides,
        EditorDockVisibility {
            asset_browser: true,
            graph: true,
            inspector: true,
        },
    );
    let fullscreen = resolve_editor_dock_layout(
        1919.0,
        1079.0,
        &overrides,
        EditorDockVisibility {
            asset_browser: true,
            graph: true,
            inspector: true,
        },
    );

    assert_dock_ratios_close(window, fullscreen, 0.035);
    assert!(fullscreen.graph.unwrap().w > window.graph.unwrap().w);
    assert!(fullscreen.inspector.unwrap().w > window.inspector.unwrap().w);
}

#[test]
fn legacy_absolute_splitter_sizes_are_interpreted_as_reference_widths() {
    let overrides = LayoutOverrides {
        asset_width: Some(132.0),
        graph_width: Some(260.0),
        inspector_width: Some(242.0),
        ..Default::default()
    };
    let window = resolve_editor_dock_layout(
        1275.0,
        746.0,
        &overrides,
        EditorDockVisibility {
            asset_browser: true,
            graph: true,
            inspector: true,
        },
    );
    let fullscreen = resolve_editor_dock_layout(
        1919.0,
        1079.0,
        &overrides,
        EditorDockVisibility {
            asset_browser: true,
            graph: true,
            inspector: true,
        },
    );

    assert_dock_ratios_close(window, fullscreen, 0.035);
}

#[test]
fn validation_report_is_not_a_stacked_bottom_panel() {
    assert_eq!(
        validation_report_placement(true),
        ValidationReportPlacement::Inspector
    );
    assert_eq!(
        validation_report_placement(false),
        ValidationReportPlacement::FloatingWindow
    );
    for height in [746.0, 1079.0] {
        let base = editor_panel_layout(1275.0, height, &LayoutOverrides::default()).timeline;
        let timeline = timeline_panel_layout(base, true);
        assert!(
            timeline.default <= height * 0.25,
            "timeline consumes {}px in {height}px viewport",
            timeline.default
        );
    }
}

#[test]
fn validation_report_uses_compact_body_when_clean() {
    assert_eq!(validation_report_body_height(520.0, 0), 28.0);
    let with_issues = validation_report_body_height(520.0, 4);
    assert!(with_issues >= 120.0);
    assert!(with_issues <= 360.0);
    assert!(with_issues < 520.0);
}

#[test]
fn capture_widths_use_grouped_toolbar_to_avoid_overflow() {
    assert_eq!(editor_toolbar_mode(1275.0), EditorToolbarMode::Grouped);
    assert_eq!(editor_toolbar_mode(1919.0), EditorToolbarMode::Grouped);
    assert_eq!(
        editor_toolbar_mode(TOOLBAR_EXPANDED_MIN_WIDTH + 1.0),
        EditorToolbarMode::Expanded
    );
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
    assert!(layout.central_min >= 400.0);
    assert!(layout.central_min <= 520.0);
    assert!(
        layout.asset_browser.max + layout.graph.max + layout.inspector.max + layout.central_min
            <= 1272.0
    );
}

#[test]
fn screenshot_width_docks_never_overlap_composer_and_inspector() {
    for width in [920.0, 1040.0, 1264.0, 1275.0] {
        let dock = resolve_editor_dock_layout(
            width,
            560.0,
            &LayoutOverrides::default(),
            EditorDockVisibility {
                asset_browser: true,
                graph: true,
                inspector: true,
            },
        );
        let inspector = dock.inspector.expect("inspector visible");
        let splitter = dock.inspector_splitter.expect("inspector splitter visible");

        assert!(
            dock.composer.x + dock.composer.w <= splitter.x + 0.1,
            "composer body must end before inspector splitter at width {width}"
        );
        assert!(
            splitter.x + splitter.w <= inspector.x + 0.1,
            "splitter must end before inspector body at width {width}"
        );
        assert!(
            inspector.x + inspector.w <= width + 0.1,
            "inspector must remain inside viewport at width {width}"
        );
    }
}

#[test]
fn manual_splitters_can_make_composer_narrow_without_negative_geometry() {
    let overrides = LayoutOverrides {
        dock_reference_width: Some(1275.0),
        asset_width: Some(150.0),
        graph_width: Some(360.0),
        inspector_width: Some(300.0),
        ..Default::default()
    };
    let dock = resolve_editor_dock_layout(
        1000.0,
        560.0,
        &overrides,
        EditorDockVisibility {
            asset_browser: true,
            graph: true,
            inspector: true,
        },
    );

    assert!(dock.composer.w >= dock.panel_sizes.central_min - 0.1);
    assert!(
        dock.panel_sizes.central_min <= 430.0,
        "composer minimum should allow the responsive toolbar to adapt instead of blocking resize"
    );
    assert!(dock.composer.w > 0.0);
    assert!(dock.inspector.unwrap().x > dock.composer.x);
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

#[test]
fn workspace_layout_roundtrips_panel_visibility_and_rect() {
    let mut layout = WorkspaceLayout::default();
    layout.set_visible(WorkspacePanelId::Graph, false);
    layout.set_collapsed(WorkspacePanelId::Validation, true);
    layout.set_floating_rect(
        WorkspacePanelId::NodeEditor,
        WorkspacePanelRect {
            x: 20.0,
            y: 30.0,
            w: 900.0,
            h: 620.0,
        },
    );

    let payload = serde_json::to_string(&layout).expect("serialize workspace layout");
    let restored: WorkspaceLayout =
        serde_json::from_str(&payload).expect("deserialize workspace layout");

    assert!(!restored.panel(WorkspacePanelId::Graph).visible);
    assert!(restored.panel(WorkspacePanelId::Validation).collapsed);
    assert_eq!(
        restored.panel(WorkspacePanelId::NodeEditor).floating_rect,
        Some(WorkspacePanelRect {
            x: 20.0,
            y: 30.0,
            w: 900.0,
            h: 620.0,
        })
    );
}

#[test]
fn workspace_layout_normalize_restores_missing_panel_state() {
    let mut layout = WorkspaceLayout {
        schema_version: 0,
        panels: std::collections::BTreeMap::new(),
    };

    layout.normalize();

    assert_eq!(layout.schema_version, WORKSPACE_LAYOUT_SCHEMA_VERSION);
    for id in WorkspacePanelId::ALL {
        assert!(
            layout.panels.contains_key(&id),
            "missing normalized panel: {id:?}"
        );
    }
    assert!(!layout.panel(WorkspacePanelId::Validation).visible);
}

fn assert_dock_ratios_close(
    first: EditorDockLayout,
    second: EditorDockLayout,
    tolerance: f32,
) {
    for (label, first_ratio, second_ratio) in [
        (
            "asset",
            first.asset_browser.unwrap().w / first_width(&first),
            second.asset_browser.unwrap().w / first_width(&second),
        ),
        (
            "graph",
            first.graph.unwrap().w / first_width(&first),
            second.graph.unwrap().w / first_width(&second),
        ),
        (
            "composer",
            first.composer.w / first_width(&first),
            second.composer.w / first_width(&second),
        ),
        (
            "inspector",
            first.inspector.unwrap().w / first_width(&first),
            second.inspector.unwrap().w / first_width(&second),
        ),
    ] {
        assert!(
            (first_ratio - second_ratio).abs() <= tolerance,
            "{label} ratio drifted from {first_ratio:.3} to {second_ratio:.3}"
        );
    }
}

fn first_width(layout: &EditorDockLayout) -> f32 {
    layout
        .inspector
        .map(|rect| rect.x + rect.w)
        .unwrap_or(layout.composer.x + layout.composer.w)
}
