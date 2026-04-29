use super::*;
use visual_novel_engine::SharedStr;

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
