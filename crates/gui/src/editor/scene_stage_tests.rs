use super::*;
use crate::editor::StageFit;
use visual_novel_engine::{EventCompiled, SharedStr, Transform, VisualState};

#[test]
fn display_visual_applies_scene_before_player_steps() {
    let current = VisualState::default();
    let event = EventCompiled::Scene(visual_novel_engine::SceneUpdateCompiled {
        background: Some(SharedStr::from("bg/room.png")),
        music: None,
        characters: vec![visual_novel_engine::CharacterPlacementCompiled {
            name: SharedStr::from("hero"),
            expression: Some(SharedStr::from("hero/smile.png")),
            position: None,
            x: Some(320),
            y: Some(180),
            scale: Some(1.2),
        }],
    });

    let visual = display_visual_for_event(&current, &event);
    let scene = scene_from_visual_state(&visual);

    assert!(scene.iter().any(|entity| matches!(
        &entity.kind,
        EntityKind::Image(image) if image.path.as_ref() == "bg/room.png"
    )));
    assert!(scene.iter().any(|entity| matches!(
        &entity.kind,
        EntityKind::Character(character)
            if character.name.as_ref() == "hero"
                && character.expression.as_deref() == Some("hero/smile.png")
    )));
}

#[test]
fn background_images_fill_stage_rect() {
    let geometry = stage_geometry(
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0)),
        (1280.0, 720.0),
        StageFit::Fill,
    );
    let mut transform = Transform::at(400, 300);
    transform.z_order = -100;
    let rect = entity_rect(
        &EntityKind::Image(visual_novel_engine::ImageData {
            path: SharedStr::from("bg.png"),
            tint: None,
        }),
        &transform,
        &geometry,
    );
    assert_eq!(rect, geometry.stage_rect);
}

#[test]
fn character_drag_is_clamped_inside_stage() {
    let geometry = stage_geometry(
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0)),
        (1280.0, 720.0),
        StageFit::Fill,
    );
    let kind = EntityKind::Character(visual_novel_engine::CharacterData {
        name: SharedStr::from("hero"),
        expression: Some(SharedStr::from("hero.png")),
    });
    let mut transform = Transform::at(5000, -20);
    transform.scale = 1000;

    clamp_transform_to_stage(&mut transform, &kind, &geometry);

    assert_eq!(transform.x, 1060);
    assert_eq!(transform.y, 0);
}

#[test]
fn scene_stage_cache_key_is_project_and_quality_specific() {
    let asset_path = "assets\\backgrounds\\room.png";
    let root_a = std::path::Path::new("C:/project-a");
    let root_b = std::path::Path::new("C:/project-b");

    let draft_a = scene_stage_cache_key(root_a, crate::editor::PreviewQuality::Draft, asset_path);
    let high_a = scene_stage_cache_key(root_a, crate::editor::PreviewQuality::High, asset_path);
    let draft_b = scene_stage_cache_key(root_b, crate::editor::PreviewQuality::Draft, asset_path);

    assert_ne!(draft_a, high_a);
    assert_ne!(draft_a, draft_b);
    assert!(
        draft_a.contains("assets/backgrounds/room.png"),
        "cache keys should normalize asset paths: {draft_a}"
    );
}

#[test]
fn active_stage_highlight_uses_owner_node_not_entity_name_or_asset() {
    assert!(entity_matches_active_node(Some(42), Some(42)));
    assert!(!entity_matches_active_node(Some(42), Some(7)));
    assert!(!entity_matches_active_node(None, Some(42)));
    assert!(!entity_matches_active_node(Some(42), None));
}

#[test]
fn stage_image_cache_aliases_extensionless_requests_to_resolved_texture() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("assets/backgrounds")).expect("mkdir assets");
    let image_path = temp.path().join("assets/backgrounds/room.png");
    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([80, 90, 100, 255]));
    image.save(&image_path).expect("write png");

    let ctx = egui::Context::default();
    let mut image_cache = HashMap::new();
    let mut failures = HashMap::new();
    let mut painter = SceneStagePainter::new(
        Some(temp.path()),
        crate::editor::PreviewQuality::Draft,
        &mut image_cache,
        &mut failures,
    );

    let first = painter
        .resolve_image_texture(&ctx, "backgrounds/room")
        .expect("extensionless image should resolve");
    let second = painter
        .resolve_image_texture(&ctx, "backgrounds/room")
        .expect("alias should hit cache");

    assert_eq!(first, second);
    drop(painter);
    assert!(failures.is_empty());
    assert!(image_cache.contains_key(&scene_stage_cache_key(
        temp.path(),
        crate::editor::PreviewQuality::Draft,
        "backgrounds/room"
    )));
    assert!(image_cache.contains_key(&scene_stage_cache_key(
        temp.path(),
        crate::editor::PreviewQuality::Draft,
        "assets/backgrounds/room.png"
    )));
}
