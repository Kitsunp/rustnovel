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
fn timeline_can_share_bottom_area_with_validation_report() {
    let base = editor_panel_layout(1280.0, 720.0, &LayoutOverrides::default()).timeline;
    let stacked = timeline_panel_layout(base, true);
    assert!(stacked.default < base.default);
    assert!(stacked.max <= 160.0);
    assert!(timeline_panel_layout(base, false).default >= base.default);
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
