use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui;
use visual_novel_gui::editor::image_asset_cache::prune_image_cache_versions;
use visual_novel_gui::editor::node_types::StoryNode;
use visual_novel_gui::editor::resource_service::EditorResourceService;
use visual_novel_gui::editor::visual_composer_preview::{fit_stage_rect, stage_scale, StageFit};
use visual_novel_gui::editor::NodeGraph;

#[test]
fn resource_budget_evicts_least_recently_used_without_invalidating_borrowed_bytes() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["a.bin", "b.bin", "c.bin"] {
        std::fs::write(dir.path().join(name), [1; 4]).unwrap();
    }
    let mut service = EditorResourceService::new().with_cache_budget(8);
    let a = service.load_bytes_shared(dir.path(), "a.bin").unwrap();
    let b = service.load_bytes_shared(dir.path(), "b.bin").unwrap();
    let cached_a = service.load_bytes_shared(dir.path(), "a.bin").unwrap();
    assert!(Arc::ptr_eq(&a, &cached_a));
    service.load_bytes_shared(dir.path(), "c.bin").unwrap();
    assert_eq!(service.metrics().bytes, 8);
    assert_eq!(service.metrics().byte_entries, 2);
    assert!(Arc::ptr_eq(
        &a,
        &service.load_bytes_shared(dir.path(), "a.bin").unwrap()
    ));
    assert!(!Arc::ptr_eq(
        &b,
        &service.load_bytes_shared(dir.path(), "b.bin").unwrap()
    ));
    assert_eq!(&*b, &[1; 4]);
    service.clear();
    assert_eq!(service.metrics().bytes, 0);
    assert_eq!(service.metrics().byte_entries, 0);
    assert_eq!(service.metrics().image_entries, 0);
}

#[test]
fn decoded_pixels_count_toward_resource_budget() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("image.png");
    image::RgbaImage::from_pixel(32, 32, image::Rgba([255, 0, 0, 255]))
        .save(&path)
        .unwrap();
    let mut service = EditorResourceService::new().with_cache_budget(1024);
    let image = service
        .image_for_view(dir.path(), "image.png", "composer")
        .unwrap();
    assert_eq!(image.pixels.len(), 4096);
    assert!(service.metrics().bytes <= 1024);
    assert_eq!(service.metrics().image_entries, 0);
    assert!(service.metrics().evictions > 0);
}

#[test]
fn disabled_cache_retains_neither_empty_files_nor_decoded_images() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("empty.bin"), []).unwrap();
    let mut service = EditorResourceService::new().with_cache_budget(0);
    for _ in 0..1000 {
        assert!(service
            .load_bytes_shared(dir.path(), "empty.bin")
            .unwrap()
            .is_empty());
    }
    assert_eq!(service.metrics().byte_entries, 0);
    assert_eq!(service.metrics().bytes, 0);
}

#[test]
fn repeated_texture_revisions_release_old_handles_and_preserve_other_views() {
    let mut cache = HashMap::new();
    let previous = Arc::new(vec![0; 4096]);
    let weak = Arc::downgrade(&previous);
    cache.insert("scene_stage::project::high::image.png::1".into(), previous);
    cache.insert(
        "asset_browser::thumb::project::image.png::1".into(),
        Arc::new(vec![1]),
    );
    for revision in 2..1000 {
        let key = format!("scene_stage::project::high::image.png::{revision}");
        prune_image_cache_versions(&mut cache, &key);
        cache.insert(key, Arc::new(vec![0; 4096]));
        assert_eq!(cache.len(), 2);
    }
    assert!(weak.upgrade().is_none());
    assert!(cache.contains_key("asset_browser::thumb::project::image.png::1"));
}

#[test]
fn graph_view_recovers_from_nonfinite_nodes_and_ignores_invalid_pan() {
    let mut graph = NodeGraph::new();
    graph.add_node(StoryNode::Start, egui::pos2(f32::INFINITY, 0.0));
    graph.add_node(StoryNode::End, egui::pos2(f32::NEG_INFINITY, 0.0));
    graph.zoom_to_fit_viewport(egui::vec2(800.0, 600.0));
    assert_eq!(graph.pan(), egui::Vec2::ZERO);
    graph.add_node(StoryNode::End, egui::pos2(100.0, 100.0));
    graph.zoom_to_fit_viewport(egui::vec2(800.0, 600.0));
    assert!(graph.pan().x.is_finite() && graph.pan().y.is_finite());
    let pan = graph.pan();
    graph.pan_by(egui::vec2(f32::NAN, 0.0));
    assert_eq!(graph.pan(), pan);
    graph.set_zoom(f32::NAN);
    assert!(graph.zoom().is_finite());
}

#[test]
fn composer_geometry_is_finite_for_invalid_bounds_and_extreme_aspects() {
    for bounds in [
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1.0, 1.0)),
        egui::Rect::NOTHING,
        egui::Rect::EVERYTHING,
    ] {
        for size in [
            (0.0, 0.0),
            (-1.0, 1.0),
            (f32::NAN, 1.0),
            (f32::INFINITY, 1.0),
            (f32::MIN_POSITIVE, f32::MAX),
            (f32::MAX, f32::MIN_POSITIVE),
            (1280.0, 720.0),
        ] {
            let rect = fit_stage_rect(bounds, size, StageFit::Normal);
            assert!(rect.min.x.is_finite() && rect.min.y.is_finite());
            assert!(rect.max.x.is_finite() && rect.max.y.is_finite());
            assert!(rect.width() >= 0.0 && rect.height() >= 0.0);
            assert!(stage_scale(rect, size).is_finite());
        }
    }
}
