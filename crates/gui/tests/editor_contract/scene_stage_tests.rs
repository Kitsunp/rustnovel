use super::*;
use crate::editor::StageFit;
use visual_novel_engine::{
    runtime::{EventCompiled, SharedStr, VisualState},
    Transform,
};

#[test]
fn background_and_locked_character_can_be_inspected_without_moving() {
    use crate::editor::{
        resource_service::EditorResourceService,
        visual_composer::{scene_entity_object_id, LayerOverride},
        PreviewQuality,
    };
    let stage = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 360.0));
    let geometry = stage_geometry(stage, (640.0, 360.0), StageFit::Fill);
    for background in [true, false] {
        let ctx = egui::Context::default();
        let mut scene = visual_novel_engine::SceneState::new();
        let mut transform = Transform::at(320, 180);
        let kind = if background {
            transform.z_order = -100;
            EntityKind::Image(visual_novel_engine::ImageData {
                path: "missing-background.png".into(),
                tint: None,
            })
        } else {
            EntityKind::Character(visual_novel_engine::CharacterData {
                name: "Ava".into(),
                expression: None,
            })
        };
        let id = scene.spawn_with_transform(transform, kind).unwrap();
        let original = scene.get(id).unwrap().transform;
        let overrides = HashMap::from([(
            scene_entity_object_id(scene.get(id).unwrap(), Some(7), 0),
            LayerOverride {
                visible: true,
                locked: true,
            },
        )]);
        let owners = HashMap::from([(id.raw(), 7)]);
        let mut selected = None;
        let mut cache = HashMap::new();
        let mut failures = HashMap::new();
        let mut resources = EditorResourceService::new();
        let pos = stage.center();
        for pressed in [None, Some(true), Some(false)] {
            let events = pressed
                .map(|pressed| {
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]
                })
                .unwrap_or_default();
            let mut interaction = None;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(stage),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::none())
                        .show(ctx, |ui| {
                            interaction = Some(
                                SceneStagePainter::new(
                                    None,
                                    PreviewQuality::High,
                                    &mut cache,
                                    &mut failures,
                                    &mut resources,
                                )
                                .with_layer_overrides(overrides.clone())
                                .paint_interactive(
                                    ui,
                                    &mut scene,
                                    geometry,
                                    &mut selected,
                                    &owners,
                                    None,
                                ),
                            );
                        });
                },
            );
            let interaction = interaction.unwrap();
            assert!(interaction.moved_character.is_none());
            if pressed == Some(false) {
                assert_eq!(interaction.selected_node, Some(7));
                assert_eq!(selected, Some(id.raw()));
                if background {
                    assert!(
                        output.shapes.iter().any(|shape| {
                            matches!(&shape.shape, egui::epaint::Shape::Rect(rect)
                            if rect.stroke.color == egui::Color32::YELLOW
                                && stage.contains_rect(rect.rect.expand(rect.stroke.width / 2.0)))
                        }),
                        "the background selection border must be visible inside the stage clip"
                    );
                }
            }
        }
        assert_eq!(scene.get(id).unwrap().transform, original);
    }
}

#[test]
fn display_visual_applies_scene_before_player_steps() {
    let current = VisualState::default();
    let event = EventCompiled::Scene(visual_novel_engine::runtime::SceneUpdateCompiled {
        background: Some(SharedStr::from("bg/room.png")),
        music: None,
        characters: vec![visual_novel_engine::runtime::CharacterPlacementCompiled {
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
    let rect = entity_rect_with_background_fit(
        &EntityKind::Image(visual_novel_engine::ImageData {
            path: SharedStr::from("bg.png"),
            tint: None,
        }),
        &transform,
        &geometry,
        crate::editor::BackgroundFit::Cover,
    );
    assert_eq!(rect, geometry.stage_rect);
}

#[test]
fn background_fit_modes_change_background_rect_without_moving_stage() {
    let geometry = stage_geometry(
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0)),
        (1280.0, 720.0),
        StageFit::Fill,
    );
    let mut transform = Transform::at(20, 30);
    transform.z_order = -100;
    let kind = EntityKind::Image(visual_novel_engine::ImageData {
        path: SharedStr::from("bg.png"),
        tint: None,
    });

    let cover = entity_rect_with_background_fit(
        &kind,
        &transform,
        &geometry,
        crate::editor::BackgroundFit::Cover,
    );
    let original = entity_rect_with_background_fit(
        &kind,
        &transform,
        &geometry,
        crate::editor::BackgroundFit::Original,
    );
    let contain = entity_rect_with_background_fit(
        &kind,
        &transform,
        &geometry,
        crate::editor::BackgroundFit::Contain,
    );

    assert_eq!(cover, geometry.stage_rect);
    assert_ne!(original, geometry.stage_rect);
    assert!(geometry.stage_rect.contains_rect(contain));
}

#[test]
fn non_background_entities_keep_logical_scale_across_viewport_sizes() {
    let large = stage_geometry(
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0)),
        (1280.0, 720.0),
        StageFit::Fill,
    );
    let small = stage_geometry(
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 360.0)),
        (1280.0, 720.0),
        StageFit::Fill,
    );
    let scale_ratio = small.scale / large.scale;
    let mut transform = Transform::at(120, 80);
    transform.scale = 1250;
    let kinds = [
        EntityKind::Character(visual_novel_engine::CharacterData {
            name: SharedStr::from("hero"),
            expression: Some(SharedStr::from("hero.png")),
        }),
        EntityKind::Image(visual_novel_engine::ImageData {
            path: SharedStr::from("prop.png"),
            tint: None,
        }),
        EntityKind::Video(visual_novel_engine::VideoData {
            path: SharedStr::from("clip.webm"),
            looping: true,
        }),
        EntityKind::Text(visual_novel_engine::TextData {
            content: "Caption".to_string(),
            font_size: 24,
            color: 0xFFFFFFFF,
        }),
    ];

    for kind in kinds {
        let large_rect = entity_rect_with_background_fit(
            &kind,
            &transform,
            &large,
            crate::editor::BackgroundFit::Cover,
        );
        let small_rect = entity_rect_with_background_fit(
            &kind,
            &transform,
            &small,
            crate::editor::BackgroundFit::Cover,
        );
        assert!(
            (small_rect.width() - large_rect.width() * scale_ratio).abs() < 0.01,
            "{kind:?} width should scale with the stage"
        );
        assert!(
            (small_rect.height() - large_rect.height() * scale_ratio).abs() < 0.01,
            "{kind:?} height should scale with the stage"
        );
    }
}

#[test]
fn audio_entities_are_layer_metadata_not_stage_text() {
    let audio = EntityKind::Audio(visual_novel_engine::AudioData {
        path: SharedStr::from("bgm/theme.ogg"),
        volume: 1000,
        looping: true,
    });

    assert!(!entity_visible_on_stage(&audio));
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
    let mut resource_service = crate::editor::resource_service::EditorResourceService::new();
    let mut painter = SceneStagePainter::new(
        Some(temp.path()),
        crate::editor::PreviewQuality::Draft,
        &mut image_cache,
        &mut failures,
        &mut resource_service,
    );

    let first = painter
        .resolve_image_texture(&ctx, "backgrounds/room")
        .expect("extensionless image should resolve");
    let _ = ctx.run(Default::default(), |_| {});
    let second = painter
        .resolve_image_texture(&ctx, "backgrounds/room")
        .expect("alias should hit cache");

    assert_eq!(first, second);
    drop(painter);
    assert_eq!(resource_service.metrics().misses, 1);
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

    let inactive = ctx.load_texture(
        "inactive",
        egui::ColorImage::new([1, 1], egui::Color32::BLACK),
        egui::TextureOptions::LINEAR,
    );
    let inactive_id = inactive.id();
    for alias in 0..513 {
        image_cache.insert(format!("inactive-{alias}"), inactive.clone());
    }
    drop(inactive);
    let output = ctx.run(Default::default(), |ctx| {
        resource_service.trim_texture_cache(&mut image_cache, ctx.frame_nr());
    });
    assert_eq!(
        image_cache.len(),
        2,
        "the stage cache hit protects both active aliases"
    );
    assert!(image_cache.values().all(|texture| texture.id() == first));
    assert!(output.textures_delta.free.contains(&inactive_id));
    assert!(!output.textures_delta.free.contains(&first));
}
