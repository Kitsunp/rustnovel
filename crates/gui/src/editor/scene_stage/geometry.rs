use eframe::egui;
use tracing::warn;
use visual_novel_engine::{
    runtime::{EventCompiled, VisualState},
    EntityKind, SceneState, Transform,
};

use crate::editor::{BackgroundFit, StageFit};

#[derive(Clone, Copy)]
pub struct StageGeometry {
    pub viewport_rect: egui::Rect,
    pub stage_rect: egui::Rect,
    pub scale: f32,
}

pub fn stage_geometry(
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

pub fn display_visual_for_event(current: &VisualState, event: &EventCompiled) -> VisualState {
    let mut visual = current.clone();
    match event {
        EventCompiled::Scene(scene) => visual.apply_scene(scene),
        EventCompiled::Patch(patch) => visual.apply_patch(patch),
        EventCompiled::SetCharacterPosition(pos) => {
            if let Err(err) = visual.set_character_position(pos) {
                warn!("preview visual position update failed: {err}");
            }
        }
        _ => {}
    }
    visual
}

pub fn scene_from_visual_state(visual: &VisualState) -> SceneState {
    let mut scene = SceneState::new();
    if let Some(background) = &visual.background {
        let mut transform = Transform::at(0, 0);
        transform.z_order = -100;
        if scene
            .spawn_with_transform(
                transform,
                EntityKind::Image(visual_novel_engine::ImageData {
                    path: background.clone(),
                    tint: None,
                }),
            )
            .is_none()
        {
            eprintln!("Failed to spawn preview background entity for '{background}'");
        }
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
        if scene
            .spawn_with_transform(
                transform,
                EntityKind::Character(visual_novel_engine::CharacterData {
                    name: character.name.clone(),
                    expression: character.expression.clone(),
                }),
            )
            .is_none()
        {
            eprintln!(
                "Failed to spawn preview character entity for '{}'",
                character.name
            );
        }
    }
    scene
}

pub fn is_background_image(kind: &EntityKind, z_order: i32) -> bool {
    matches!(kind, EntityKind::Image(_)) && z_order <= -50
}

pub fn clamp_transform_to_stage(
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

pub fn entity_rect_with_background_fit(
    kind: &EntityKind,
    transform: &Transform,
    geometry: &StageGeometry,
    background_fit: BackgroundFit,
) -> egui::Rect {
    if is_background_image(kind, transform.z_order) {
        return background_rect(kind, transform, geometry, background_fit);
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

fn background_rect(
    kind: &EntityKind,
    transform: &Transform,
    geometry: &StageGeometry,
    background_fit: BackgroundFit,
) -> egui::Rect {
    match background_fit {
        BackgroundFit::Cover | BackgroundFit::Stretch | BackgroundFit::Tile => geometry.stage_rect,
        BackgroundFit::Original => {
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
        BackgroundFit::Contain => {
            let logical = entity_logical_size(kind, transform);
            let stage = geometry.stage_rect.size();
            let scale = (stage.x / logical.x).min(stage.y / logical.y);
            let size = logical * scale;
            egui::Rect::from_center_size(geometry.stage_rect.center(), size)
        }
    }
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
