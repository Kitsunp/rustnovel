use std::path::Path;

use eframe::egui;
use visual_novel_engine::{EntityId, PropertyType, Timeline};
use visual_novel_gui::editor::timeline_panel::{
    add_keyframe, keyframe_marker_x, property_label, timeline_lane_rect,
    timeline_track_list_height, ANIMATABLE_PROPERTIES, TIMELINE_TRACK_LIST_MAX_HEIGHT,
    TIMELINE_TRACK_LIST_MIN_HEIGHT,
};
use visual_novel_gui::editor::undo::{UndoStack, MAX_UNDO_STATES};
use visual_novel_gui::editor::visual_composer::viewport::composer_viewport_size;
use visual_novel_gui::editor::visual_composer_preview::{
    fit_stage_rect, scaled_size_for_max_edge, stage_viewport_size, PreviewQuality, StageFit,
};
use visual_novel_gui::editor::workbench::audio_preview_store::GuiAudioAssetStore;
use visual_novel_gui::editor::{NodeGraph, StoryNode};
use visual_novel_runtime::AssetStore;

fn create_graph_with_nodes(count: usize) -> NodeGraph {
    let mut graph = NodeGraph::new();
    for i in 0..count {
        graph.add_node(StoryNode::Start, egui::pos2(i as f32 * 50.0, 0.0));
    }
    graph
}

fn create_file_symlink(link: &Path, target: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = link;
        let _ = target;
        false
    }
}

#[test]
fn undo_stack_push_undo_redo_and_limit_contract() {
    let mut stack = UndoStack::new();
    let state1 = create_graph_with_nodes(1);
    let state2 = create_graph_with_nodes(2);
    let state3 = create_graph_with_nodes(3);

    stack.push(state1.clone());
    stack.push(state2.clone());
    assert!(stack.can_undo());
    assert!(!stack.can_redo());
    let restored = stack.undo(state3.clone()).expect("undo");
    assert_eq!(restored.len(), 2);
    assert!(stack.can_redo());

    let redone = stack.redo(restored).expect("redo");
    assert_eq!(redone.len(), 3);
    stack.push(state3);
    assert!(!stack.can_redo());

    for i in 0..60 {
        stack.push(create_graph_with_nodes(i));
    }
    assert_eq!(stack.undo_count(), MAX_UNDO_STATES);
}

#[test]
fn timeline_helpers_create_tracks_and_label_supported_properties() {
    let mut timeline = Timeline::new(60);
    add_keyframe(&mut timeline, 7, PropertyType::PositionX, 12, 300).expect("add keyframe");

    assert_eq!(timeline.track_count(), 1);
    let track = timeline.get_track(0).expect("track exists");
    assert_eq!(track.target, EntityId::new(7));
    assert_eq!(track.property, PropertyType::PositionX);
    assert_eq!(track.len(), 1);

    for property in ANIMATABLE_PROPERTIES {
        assert!(!property_label(property).is_empty());
    }
}

#[test]
fn timeline_track_geometry_keeps_markers_inside_lane_for_window_sizes() {
    for width in [220.0, 480.0, 1280.0] {
        let row = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 28.0));
        let lane = timeline_lane_rect(row);
        assert!(lane.left() >= row.left());
        assert!(lane.right() <= row.right());
        assert!(lane.width() >= 1.0);

        let start = keyframe_marker_x(lane, 0, 120);
        let middle = keyframe_marker_x(lane, 60, 120);
        let end = keyframe_marker_x(lane, 120, 120);
        assert!(start >= lane.left() && start <= lane.right());
        assert!(middle > start && middle < end);
        assert!(end >= lane.left() && end <= lane.right());
    }
}

#[test]
fn timeline_track_list_height_is_stable_for_compact_and_fullscreen_modes() {
    assert_eq!(
        timeline_track_list_height(12.0),
        TIMELINE_TRACK_LIST_MIN_HEIGHT
    );
    assert_eq!(
        timeline_track_list_height(900.0),
        TIMELINE_TRACK_LIST_MAX_HEIGHT
    );
    assert_eq!(timeline_track_list_height(96.0), 96.0);
}

#[test]
fn preview_stage_fit_and_quality_contracts() {
    let wide = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 900.0));
    let compact = fit_stage_rect(wide, (1280.0, 720.0), StageFit::Compact);
    let normal = fit_stage_rect(wide, (1280.0, 720.0), StageFit::Normal);
    let fill = fit_stage_rect(wide, (1280.0, 720.0), StageFit::Fill);
    assert!(compact.width() < normal.width());
    assert!(normal.width() < fill.width());

    let square = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 1000.0));
    let rect = fit_stage_rect(square, (1280.0, 720.0), StageFit::Fill);
    assert!((rect.width() / rect.height() - (16.0 / 9.0)).abs() < 0.01);

    assert_eq!(scaled_size_for_max_edge([1920, 1080], 640), [640, 360]);
    assert_eq!(scaled_size_for_max_edge([1920, 1080], 1280), [1280, 720]);
    assert_eq!(
        PreviewQuality::High.scaled_image([1920, 1080], &[0; 16]).0,
        [1920, 1080]
    );
}

#[test]
fn shared_stage_viewport_caps_tall_panels_and_handles_tiny_space() {
    let tall = stage_viewport_size(egui::vec2(900.0, 1000.0), (1280.0, 720.0), 28.0, 0.78);
    assert_eq!(tall.x, 900.0);
    assert!(tall.y < 540.0);

    let tiny = stage_viewport_size(egui::vec2(320.0, 20.0), (1280.0, 720.0), 28.0, 0.78);
    assert_eq!(tiny, egui::vec2(320.0, 0.0));
}

#[test]
fn composer_viewport_reserves_editor_space() {
    let tall = composer_viewport_size(egui::vec2(900.0, 1000.0), (1280.0, 720.0));
    assert_eq!(tall.x, 900.0);
    assert!(tall.y <= 900.0 * 9.0 / 16.0 + 12.0 + 0.1);
    assert!(tall.y < 700.0);

    let short = composer_viewport_size(egui::vec2(720.0, 180.0), (1280.0, 720.0));
    assert!(short.y <= 152.0);
    assert!(short.y >= 96.0);

    let tiny = composer_viewport_size(egui::vec2(320.0, 20.0), (1280.0, 720.0));
    assert_eq!(tiny, egui::vec2(320.0, 0.0));
}

#[test]
fn preview_store_loads_absolute_audio_without_project_root() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("preview.wav");
    std::fs::write(&path, b"audio-bytes").expect("write audio");

    let store = GuiAudioAssetStore::new(None).expect("store");
    let bytes = store
        .load_bytes(path.to_str().expect("utf8 path"))
        .expect("absolute path should load");

    assert_eq!(bytes, b"audio-bytes");
}

#[test]
fn preview_store_does_not_bypass_asset_store_symlink_escape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    let outside = temp.path().join("outside-theme.ogg");
    std::fs::create_dir_all(project_root.join("audio")).expect("audio dir");
    std::fs::write(&outside, b"outside-audio").expect("outside audio");
    let link = project_root.join("audio/theme.ogg");
    if !create_file_symlink(&link, &outside) {
        eprintln!("file symlink creation not supported on this platform");
        return;
    }

    let store = GuiAudioAssetStore::new(Some(project_root)).expect("store");
    let err = store
        .load_bytes("audio/theme.ogg")
        .expect_err("relative audio symlink escape must be blocked");

    assert!(err.contains("traversal"), "err={err}");
}
