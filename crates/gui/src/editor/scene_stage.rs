use std::collections::HashMap;
use std::path::Path;

use eframe::egui;
use visual_novel_engine::{EntityId, EntityKind, SceneState};

use crate::editor::image_asset_cache::{
    image_failure_message, normalize_asset_path, scene_stage_cache_key,
    should_retry_missing_image_failure,
};
use crate::editor::visual_composer::scene_entity_object_id;
use crate::editor::visual_composer::LayerOverride;
use crate::editor::PreviewQuality;

#[path = "scene_stage/fallbacks.rs"]
mod fallbacks;
use fallbacks::*;
#[path = "scene_stage/geometry.rs"]
mod geometry;
pub(crate) use geometry::{
    clamp_transform_to_stage, display_visual_for_event, entity_rect, is_background_image,
    scene_from_visual_state, stage_geometry, StageGeometry,
};

pub(crate) struct SceneStagePainter<'a> {
    project_root: Option<&'a Path>,
    preview_quality: PreviewQuality,
    image_cache: &'a mut HashMap<String, egui::TextureHandle>,
    image_failures: &'a mut HashMap<String, String>,
    asset_store: Option<vnengine_assets::AssetStore>,
    layer_overrides: HashMap<String, LayerOverride>,
}

pub(crate) struct SceneStageInteraction {
    pub selected_node: Option<u32>,
    pub moved_character: Option<MovedCharacter>,
}

pub(crate) struct MovedCharacter {
    pub node_id: u32,
    pub name: String,
    pub expression: Option<String>,
    pub source_instance_index: usize,
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
            asset_store: None,
            layer_overrides: HashMap::new(),
        }
    }

    pub fn with_layer_overrides(mut self, layer_overrides: HashMap<String, LayerOverride>) -> Self {
        self.layer_overrides = layer_overrides;
        self
    }

    pub fn paint_read_only(
        &mut self,
        ui: &mut egui::Ui,
        scene: &SceneState,
        geometry: StageGeometry,
    ) {
        self.paint_canvas(ui, &geometry, scene.is_empty());
        for (index, entity) in scene.iter_sorted().enumerate() {
            if self
                .entity_layer_override(entity, None, index)
                .is_some_and(|entry| !entry.visible)
            {
                continue;
            }
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
        active_node_id: Option<u32>,
    ) -> SceneStageInteraction {
        self.paint_canvas(ui, &geometry, scene.is_empty());
        let mut selected_node = None;
        let mut moved_entity = None;
        let ids = scene
            .iter_sorted()
            .enumerate()
            .map(|(index, entity)| (index, entity.id.raw()))
            .collect::<Vec<_>>();

        for (index, raw_id) in ids {
            let Some(entity) = scene.get(EntityId::new(raw_id)).cloned() else {
                continue;
            };
            let source_node_id = entity_owners.get(&raw_id).copied();
            if self
                .entity_layer_override(&entity, source_node_id, index)
                .is_some_and(|entry| !entry.visible)
            {
                continue;
            }
            let is_background = is_background_image(&entity.kind, entity.transform.z_order);
            if is_background && *selected_entity_id == Some(raw_id) {
                *selected_entity_id = None;
            }

            let rect = entity_rect(&entity.kind, &entity.transform, &geometry);
            let locked = self
                .entity_layer_override(&entity, source_node_id, index)
                .is_some_and(|entry| entry.locked);
            let sense = if is_background || locked {
                egui::Sense::hover()
            } else {
                egui::Sense::click_and_drag()
            };
            let interact = ui.interact(rect, egui::Id::new(("scene_entity", raw_id)), sense);

            if !is_background && !locked && (interact.clicked() || interact.double_clicked()) {
                *selected_entity_id = Some(raw_id);
                selected_node = entity_owners.get(&raw_id).copied();
            }
            if !is_background && !locked && interact.dragged() {
                *selected_entity_id = Some(raw_id);
                moved_entity = Some((
                    raw_id,
                    ui.input(|input| input.pointer.delta()) / geometry.scale,
                ));
            }

            let is_selected = *selected_entity_id == Some(raw_id);
            let is_active = entity_matches_active_node(source_node_id, active_node_id);
            if is_active && !is_selected {
                ui.painter().rect_stroke(
                    rect.expand(3.0),
                    0.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(120, 220, 255)),
                );
            }
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
            let snapshot = scene.get(id)?.clone();
            let EntityKind::Character(character) = &snapshot.kind else {
                return None;
            };
            let node_id = *entity_owners.get(&raw_id)?;
            let expression = character
                .expression
                .as_ref()
                .map(|value| value.as_ref().to_string());
            let source_instance_index = character_instance_index(
                scene,
                entity_owners,
                raw_id,
                node_id,
                character.name.as_ref(),
                expression.as_deref(),
            );
            let entity = scene.get_mut(id)?;
            entity.transform.x += delta.x.round() as i32;
            entity.transform.y += delta.y.round() as i32;
            clamp_transform_to_stage(&mut entity.transform, &entity.kind, &geometry);
            let scale = if entity.transform.scale == 1000 {
                None
            } else {
                Some(entity.transform.scale as f32 / 1000.0)
            };
            Some(MovedCharacter {
                node_id,
                name: character.name.to_string(),
                expression,
                source_instance_index,
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

    fn entity_layer_override(
        &self,
        entity: &visual_novel_engine::Entity,
        source_node_id: Option<u32>,
        index: usize,
    ) -> Option<&LayerOverride> {
        let stable_id = scene_entity_object_id(entity, source_node_id, index);
        self.layer_overrides.get(&stable_id).or_else(|| {
            self.layer_overrides
                .get(&format!("entity:{}", entity.id.raw()))
        })
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
        let asset_path = normalize_asset_path(asset_path);
        let project_root = self.project_root?;
        let request_cache_key =
            scene_stage_cache_key(project_root, self.preview_quality, &asset_path);
        if let Some(texture) = self.image_cache.get(&request_cache_key) {
            return Some(texture.id());
        }
        if let Some(should_retry) = self
            .image_failures
            .get(&request_cache_key)
            .map(|failure| should_retry_missing_image_failure(failure, project_root, &asset_path))
        {
            if should_retry {
                self.image_failures.remove(&request_cache_key);
            } else {
                return None;
            }
        }

        self.asset_store(project_root, &asset_path, &request_cache_key)?;
        let resolved_asset_path = match self
            .asset_store
            .as_ref()
            .expect("asset store initialized")
            .resolve_image_path(&asset_path)
        {
            Ok(path) => normalize_asset_path(&path),
            Err(err) => {
                self.image_failures
                    .insert(request_cache_key, image_failure_message(&asset_path, &err));
                return None;
            }
        };
        let cache_key =
            scene_stage_cache_key(project_root, self.preview_quality, &resolved_asset_path);
        if let Some(texture) = self.image_cache.get(&cache_key) {
            let texture = texture.clone();
            if request_cache_key != cache_key {
                self.image_cache.insert(request_cache_key, texture.clone());
            }
            return Some(texture.id());
        }
        if self.image_failures.contains_key(&cache_key) {
            return None;
        }

        let image = self
            .load_image(project_root, &resolved_asset_path, &cache_key)
            .ok()?;
        let (size, pixels) = self.preview_quality.scaled_image(image.size, &image.pixels);
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_ref());
        let texture = ctx.load_texture(
            format!("scene_stage::{cache_key}"),
            color_image,
            self.preview_quality.texture_options(),
        );
        let id = texture.id();
        if request_cache_key != cache_key {
            self.image_cache.insert(request_cache_key, texture.clone());
        }
        self.image_cache.insert(cache_key, texture);
        Some(id)
    }

    fn load_image(
        &mut self,
        project_root: &Path,
        asset_path: &str,
        failure_key: &str,
    ) -> Result<vnengine_assets::LoadedImage, ()> {
        let Some(store) = self.asset_store(project_root, asset_path, failure_key) else {
            return Err(());
        };
        match store.load_image(asset_path) {
            Ok(image) => Ok(image),
            Err(err) => {
                self.image_failures.insert(
                    failure_key.to_string(),
                    image_failure_message(asset_path, &err),
                );
                Err(())
            }
        }
    }

    fn asset_store(
        &mut self,
        project_root: &Path,
        asset_path: &str,
        failure_key: &str,
    ) -> Option<&vnengine_assets::AssetStore> {
        if self.asset_store.is_none() {
            self.asset_store = match vnengine_assets::AssetStore::new(
                project_root.to_path_buf(),
                vnengine_assets::SecurityMode::Trusted,
                None,
                false,
            ) {
                Ok(store) => Some(store),
                Err(err) => {
                    self.image_failures.insert(
                        failure_key.to_string(),
                        format!("image '{asset_path}' asset store initialization failed: {err}"),
                    );
                    None
                }
            };
        }
        self.asset_store.as_ref()
    }
}

pub(crate) fn entity_matches_active_node(
    source_node_id: Option<u32>,
    active_node_id: Option<u32>,
) -> bool {
    source_node_id.is_some() && source_node_id == active_node_id
}

fn character_instance_index(
    scene: &SceneState,
    entity_owners: &HashMap<u32, u32>,
    moved_raw_id: u32,
    moved_node_id: u32,
    name: &str,
    expression: Option<&str>,
) -> usize {
    let mut index = 0usize;
    for entity in scene.iter_sorted() {
        let raw_id = entity.id.raw();
        if raw_id == moved_raw_id {
            break;
        }
        if entity_owners.get(&raw_id).copied() != Some(moved_node_id) {
            continue;
        }
        let EntityKind::Character(character) = &entity.kind else {
            continue;
        };
        if character.name.as_ref() == name && character.expression.as_deref() == expression {
            index += 1;
        }
    }
    index
}

#[cfg(test)]
#[path = "scene_stage_tests.rs"]
mod tests;
