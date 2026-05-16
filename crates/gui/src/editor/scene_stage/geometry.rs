use eframe::egui;
use visual_novel_engine::{EntityKind, EventCompiled, SceneState, Transform, VisualState};

use crate::editor::StageFit;

#[derive(Clone, Copy)]
pub(crate) struct StageGeometry {
    pub viewport_rect: egui::Rect,
    pub stage_rect: egui::Rect,
    pub scale: f32,
}

pub(crate) fn stage_geometry(
    viewport_rect: egui::Rect,
    stage_size: (f32, f32),
    stage_fit: StageFit,
) -> StageGeometry {
    let stage_rect = crate::editor::visual_composer_preview::fit_stage_rect(
        viewport_rect,
        stage_size,
        stage_fit,
    );
    let scale = crate::editor::visual_composer_preview::stage_scale(stage_rect, stage_size);
    StageGeometry {
        viewport_rect,
        stage_rect,
        scale,
    }
}

pub(crate) fn display_visual_for_event(
    current: &VisualState,
    event: &EventCompiled,
) -> VisualState {
    let mut visual = current.clone();
    match event {
        EventCompiled::Scene(scene) => visual.apply_scene(scene),
        EventCompiled::Patch(patch) => visual.apply_patch(patch),
        EventCompiled::SetCharacterPosition(pos) => visual.set_character_position(pos),
        _ => {}
    }
    visual
}

pub(crate) fn scene_from_visual_state(visual: &VisualState) -> SceneState {
    let mut scene = SceneState::new();
    if let Some(background) = &visual.background {
        let mut transform = Transform::at(0, 0);
        transform.z_order = -100;
        let _ = scene.spawn_with_transform(
            transform,
            EntityKind::Image(visual_novel_engine::ImageData {
                path: background.clone(),
                tint: None,
            }),
        );
    }
    for (index, character) in visual.characters.iter().enumerate() {
        let default_x = 220 + (index as i32) * 180;
        let default_y = 260;
        let mut transform = Transform::at(
            character.x.unwrap_or(default_x),
            character.y.unwrap_or(default_y),
        );
        transform.z_order = index as i32;
        transform.scale = (character.scale.unwrap_or(1.0).clamp(0.1, 4.0) * 1000.0) as u32;
        let _ = scene.spawn_with_transform(
            transform,
            EntityKind::Character(visual_novel_engine::CharacterData {
                name: character.name.clone(),
                expression: character.expression.clone(),
            }),
        );
    }
    scene
}

pub(crate) fn is_background_image(kind: &EntityKind, z_order: i32) -> bool {
    matches!(kind, EntityKind::Image(_)) && z_order <= -50
}

pub(crate) fn clamp_transform_to_stage(
    transform: &mut Transform,
    kind: &EntityKind,
    geometry: &StageGeometry,
) {
    if is_background_image(kind, transform.z_order) {
        transform.x = 0;
        transform.y = 0;
        return;
    }
    let logical_stage = egui::vec2(
        geometry.stage_rect.width() / geometry.scale,
        geometry.stage_rect.height() / geometry.scale,
    );
    let logical_size = entity_logical_size(kind, transform);
    let max_x = (logical_stage.x - logical_size.x).max(0.0).round() as i32;
    let max_y = (logical_stage.y - logical_size.y).max(0.0).round() as i32;
    transform.x = transform.x.clamp(0, max_x);
    transform.y = transform.y.clamp(0, max_y);
}

pub(crate) fn entity_rect(
    kind: &EntityKind,
    transform: &Transform,
    geometry: &StageGeometry,
) -> egui::Rect {
    if is_background_image(kind, transform.z_order) {
        return geometry.stage_rect;
    }
    let position = geometry.stage_rect.min
        + egui::vec2(
            transform.x as f32 * geometry.scale,
            transform.y as f32 * geometry.scale,
        );
    egui::Rect::from_min_size(
        position,
        entity_logical_size(kind, transform) * geometry.scale,
    )
}

fn entity_logical_size(kind: &EntityKind, transform: &Transform) -> egui::Vec2 {
    let scale = (transform.scale as f32 / 1000.0).clamp(0.1, 4.0);
    let base_size = match kind {
        EntityKind::Character(_) => egui::vec2(220.0, 340.0),
        EntityKind::Image(_) => egui::vec2(220.0, 140.0),
        EntityKind::Video(_) => egui::vec2(320.0, 180.0),
        EntityKind::Audio(_) => egui::vec2(300.0, 36.0),
        EntityKind::Text(_) => egui::vec2(300.0, 54.0),
    };
    base_size * scale
}
