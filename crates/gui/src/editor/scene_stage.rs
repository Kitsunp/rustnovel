use std::collections::HashMap;
use std::path::Path;

use eframe::egui;
use visual_novel_engine::{
    EntityId, EntityKind, EventCompiled, SceneState, Transform, VisualState,
};

use crate::editor::{PreviewQuality, StageFit};

#[path = "scene_stage/fallbacks.rs"]
mod fallbacks;
use fallbacks::*;

#[derive(Clone, Copy)]
pub(crate) struct StageGeometry {
    pub viewport_rect: egui::Rect,
    pub stage_rect: egui::Rect,
    pub scale: f32,
}

pub(crate) struct SceneStagePainter<'a> {
    project_root: Option<&'a Path>,
    preview_quality: PreviewQuality,
    image_cache: &'a mut HashMap<String, egui::TextureHandle>,
    image_failures: &'a mut HashMap<String, String>,
}

pub(crate) struct SceneStageInteraction {
    pub selected_node: Option<u32>,
    pub moved_character: Option<MovedCharacter>,
}

pub(crate) struct MovedCharacter {
    pub node_id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub scale: Option<f32>,
}

impl<'a> SceneStagePainter<'a> {
    pub fn new(
        project_root: Option<&'a Path>,
        preview_quality: PreviewQuality,
        image_cache: &'a mut HashMap<String, egui::TextureHandle>,
        image_failures: &'a mut HashMap<String, String>,
    ) -> Self {
        Self {
            project_root,
            preview_quality,
            image_cache,
            image_failures,
        }
    }

    pub fn paint_read_only(
        &mut self,
        ui: &mut egui::Ui,
        scene: &SceneState,
        geometry: StageGeometry,
    ) {
        self.paint_canvas(ui, &geometry, scene.is_empty());
        for entity in scene.iter_sorted() {
            let rect = entity_rect(&entity.kind, &entity.transform, &geometry);
            self.paint_entity(ui, &entity.kind, rect, false);
        }
    }

    pub fn paint_interactive(
        &mut self,
        ui: &mut egui::Ui,
        scene: &mut SceneState,
        geometry: StageGeometry,
        selected_entity_id: &mut Option<u32>,
        entity_owners: &HashMap<u32, u32>,
    ) -> SceneStageInteraction {
        self.paint_canvas(ui, &geometry, scene.is_empty());
        let mut selected_node = None;
        let mut moved_entity = None;
        let ids = scene
            .iter_sorted()
            .map(|entity| entity.id.raw())
            .collect::<Vec<_>>();

        for raw_id in ids {
            let Some(entity) = scene.get(EntityId::new(raw_id)).cloned() else {
                continue;
            };
            let is_background = is_background_image(&entity.kind, entity.transform.z_order);
            if is_background && *selected_entity_id == Some(raw_id) {
                *selected_entity_id = None;
            }

            let rect = entity_rect(&entity.kind, &entity.transform, &geometry);
            let sense = if is_background {
                egui::Sense::hover()
            } else {
                egui::Sense::click_and_drag()
            };
            let interact = ui.interact(rect, egui::Id::new(("scene_entity", raw_id)), sense);

            if !is_background && (interact.clicked() || interact.double_clicked()) {
                *selected_entity_id = Some(raw_id);
                selected_node = entity_owners.get(&raw_id).copied();
            }
            if !is_background && interact.dragged() {
                *selected_entity_id = Some(raw_id);
                moved_entity = Some((
                    raw_id,
                    ui.input(|input| input.pointer.delta()) / geometry.scale,
                ));
            }

            let is_selected = *selected_entity_id == Some(raw_id);
            if is_selected && !is_background {
                ui.painter().rect_stroke(
                    rect.expand(2.0),
                    0.0,
                    egui::Stroke::new(2.0, egui::Color32::YELLOW),
                );
            }
            self.paint_entity(ui, &entity.kind, rect, is_selected);
        }

        let moved_character = moved_entity.and_then(|(raw_id, delta)| {
            let id = EntityId::new(raw_id);
            let entity = scene.get_mut(id)?;
            entity.transform.x += delta.x as i32;
            entity.transform.y += delta.y as i32;
            let EntityKind::Character(character) = &entity.kind else {
                return None;
            };
            let node_id = *entity_owners.get(&raw_id)?;
            let scale = if entity.transform.scale == 1000 {
                None
            } else {
                Some(entity.transform.scale as f32 / 1000.0)
            };
            Some(MovedCharacter {
                node_id,
                name: character.name.to_string(),
                x: entity.transform.x,
                y: entity.transform.y,
                scale,
            })
        });

        SceneStageInteraction {
            selected_node,
            moved_character,
        }
    }

    fn paint_canvas(&self, ui: &egui::Ui, geometry: &StageGeometry, is_empty: bool) {
        ui.painter().rect_filled(
            geometry.viewport_rect,
            0.0,
            egui::Color32::from_rgb(20, 20, 20),
        );
        ui.painter().rect_filled(
            geometry.stage_rect,
            0.0,
            egui::Color32::from_rgb(24, 24, 34),
        );
        ui.painter().rect_stroke(
            geometry.stage_rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(110)),
        );

        if is_empty {
            ui.painter().text(
                geometry.stage_rect.center(),
                egui::Align2::CENTER_CENTER,
                "No preview entities",
                egui::FontId::proportional(16.0),
                egui::Color32::from_gray(150),
            );
        }
    }

    fn paint_entity(
        &mut self,
        ui: &egui::Ui,
        kind: &EntityKind,
        rect: egui::Rect,
        is_selected: bool,
    ) {
        match kind {
            EntityKind::Image(image) => {
                if let Some(texture_id) = self.resolve_image_texture(ui.ctx(), image.path.as_ref())
                {
                    ui.painter().image(
                        texture_id,
                        rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    paint_asset_fallback(ui, rect, is_selected, "Image", image.path.as_ref());
                }
            }
            EntityKind::Character(character) => {
                if let Some(expression) = &character.expression {
                    if let Some(texture_id) =
                        self.resolve_image_texture(ui.ctx(), expression.as_ref())
                    {
                        ui.painter().image(
                            texture_id,
                            rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                        paint_caption(ui, rect, character.name.as_ref());
                        return;
                    }
                }
                paint_character_fallback(
                    ui,
                    rect,
                    is_selected,
                    character.name.as_ref(),
                    character.expression.as_deref(),
                );
            }
            EntityKind::Audio(audio) => {
                paint_audio_surface(ui, rect, is_selected, audio.path.as_ref(), audio.looping);
            }
            EntityKind::Video(video) => {
                paint_video_surface(ui, rect, is_selected, video.path.as_ref(), video.looping);
            }
            EntityKind::Text(text) => {
                paint_text_surface(
                    ui,
                    rect,
                    is_selected,
                    &text.content,
                    text.font_size,
                    text.color,
                );
            }
        }
    }

    fn resolve_image_texture(
        &mut self,
        ctx: &egui::Context,
        asset_path: &str,
    ) -> Option<egui::TextureId> {
        let cache_key = format!("{}::{asset_path}", self.preview_quality.label());
        if let Some(texture) = self.image_cache.get(&cache_key) {
            return Some(texture.id());
        }
        if self.image_failures.contains_key(asset_path) {
            return None;
        }
        let Some(project_root) = self.project_root else {
            self.image_failures.insert(
                asset_path.to_string(),
                format!("image '{asset_path}' project_root not available"),
            );
            return None;
        };

        let image = self.load_image(project_root, asset_path).ok()?;
        let (size, pixels) = self.preview_quality.scaled_image(image.size, &image.pixels);
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_ref());
        let texture = ctx.load_texture(
            format!("scene_stage::{cache_key}"),
            color_image,
            self.preview_quality.texture_options(),
        );
        let id = texture.id();
        self.image_cache.insert(cache_key, texture);
        Some(id)
    }

    fn load_image(
        &mut self,
        project_root: &Path,
        asset_path: &str,
    ) -> Result<vnengine_assets::LoadedImage, ()> {
        let store = match vnengine_assets::AssetStore::new(
            project_root.to_path_buf(),
            vnengine_assets::SecurityMode::Trusted,
            None,
            false,
        ) {
            Ok(store) => store,
            Err(err) => {
                self.image_failures.insert(
                    asset_path.to_string(),
                    format!("image '{asset_path}' asset store initialization failed: {err}"),
                );
                return Err(());
            }
        };

        match store.load_image(asset_path) {
            Ok(image) => Ok(image),
            Err(err) => {
                self.image_failures.insert(
                    asset_path.to_string(),
                    format!("image '{asset_path}' load failed: {err}"),
                );
                Err(())
            }
        }
    }
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

fn entity_rect(kind: &EntityKind, transform: &Transform, geometry: &StageGeometry) -> egui::Rect {
    if is_background_image(kind, transform.z_order) {
        return geometry.stage_rect;
    }
    let position = geometry.stage_rect.min
        + egui::vec2(
            transform.x as f32 * geometry.scale,
            transform.y as f32 * geometry.scale,
        );
    let scale = (transform.scale as f32 / 1000.0).clamp(0.1, 4.0);
    let base_size = match kind {
        EntityKind::Character(_) => egui::vec2(220.0, 340.0),
        EntityKind::Image(_) => egui::vec2(220.0, 140.0),
        EntityKind::Video(_) => egui::vec2(320.0, 180.0),
        EntityKind::Audio(_) => egui::vec2(300.0, 36.0),
        EntityKind::Text(_) => egui::vec2(300.0, 54.0),
    };
    egui::Rect::from_min_size(position, base_size * geometry.scale * scale)
}

#[cfg(test)]
#[path = "scene_stage_tests.rs"]
mod tests;
