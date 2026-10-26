use std::collections::HashMap;
use std::path::Path;

use eframe::egui;
use visual_novel_engine::{EntityId, EntityKind, SceneState};

use crate::editor::image_asset_cache::{ImageTextureCache, ImageView};
use crate::editor::resource_service::EditorResourceService;
use crate::editor::visual_composer::scene_entity_object_id;
use crate::editor::visual_composer::LayerOverride;
use crate::editor::PreviewQuality;

#[path = "scene_stage/fallbacks.rs"]
mod fallbacks;
use fallbacks::*;
#[path = "scene_stage/geometry.rs"]
mod geometry;
pub use geometry::{
    clamp_transform_to_stage, display_visual_for_event, entity_rect_with_background_fit,
    is_background_image, scene_from_visual_state, stage_geometry, StageGeometry,
};

pub fn entity_visible_on_stage(kind: &EntityKind) -> bool {
    !matches!(kind, EntityKind::Audio(_))
}

pub struct SceneStagePainter<'a> {
    project_root: Option<&'a Path>,
    preview_quality: PreviewQuality,
    image_cache: &'a mut HashMap<String, egui::TextureHandle>,
    image_failures: &'a mut HashMap<String, String>,
    resource_service: &'a mut EditorResourceService,
    layer_overrides: Option<&'a std::collections::BTreeMap<String, LayerOverride>>,
    entity_object_ids: HashMap<u32, String>,
    background_fit: crate::editor::BackgroundFit,
}

pub struct SceneStageInteraction {
    pub selected_node: Option<u32>,
    pub moved_character: Option<MovedCharacter>,
}

pub struct MovedCharacter {
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
        resource_service: &'a mut EditorResourceService,
    ) -> Self {
        Self {
            project_root,
            preview_quality,
            image_cache,
            image_failures,
            resource_service,
            layer_overrides: None,
            entity_object_ids: HashMap::new(),
            background_fit: crate::editor::BackgroundFit::default(),
        }
    }

    pub fn with_layer_overrides(
        mut self,
        layer_overrides: &'a std::collections::BTreeMap<String, LayerOverride>,
    ) -> Self {
        self.layer_overrides = Some(layer_overrides);
        self
    }

    pub fn with_background_fit(mut self, background_fit: crate::editor::BackgroundFit) -> Self {
        self.background_fit = background_fit;
        self
    }

    pub fn with_entity_object_ids(mut self, ids: HashMap<u32, String>) -> Self {
        self.entity_object_ids = ids;
        self
    }

    pub fn paint_read_only(
        &mut self,
        ui: &mut egui::Ui,
        scene: &SceneState,
        geometry: StageGeometry,
    ) {
        self.paint_canvas(ui, &geometry, scene.is_empty());
        let previous_clip = ui.clip_rect();
        ui.set_clip_rect(previous_clip.intersect(geometry.stage_rect));
        for (index, entity) in scene.iter_sorted().enumerate() {
            if !entity_visible_on_stage(&entity.kind) {
                continue;
            }
            if self
                .entity_layer_override(entity, None, index)
                .is_some_and(|entry| !entry.visible)
            {
                continue;
            }
            let rect = entity_rect_with_background_fit(
                &entity.kind,
                &entity.transform,
                &geometry,
                self.background_fit,
            );
            self.paint_entity(ui, &entity.kind, rect, false, entity.transform.opacity);
        }
        ui.set_clip_rect(previous_clip);
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
            if !entity_visible_on_stage(&entity.kind) {
                if *selected_entity_id == Some(raw_id) {
                    *selected_entity_id = None;
                }
                continue;
            }
            let source_node_id = entity_owners.get(&raw_id).copied();
            if self
                .entity_layer_override(&entity, source_node_id, index)
                .is_some_and(|entry| !entry.visible)
            {
                continue;
            }
            let is_background = is_background_image(&entity.kind, entity.transform.z_order);

            let rect = entity_rect_with_background_fit(
                &entity.kind,
                &entity.transform,
                &geometry,
                self.background_fit,
            );
            let locked = self
                .entity_layer_override(&entity, source_node_id, index)
                .is_some_and(|entry| entry.locked);
            let interact_rect = rect.intersect(geometry.stage_rect);
            if !interact_rect.is_positive() {
                continue;
            }
            let sense = if is_background || locked {
                egui::Sense::click()
            } else {
                egui::Sense::click_and_drag()
            };
            let previous_clip = ui.clip_rect();
            ui.set_clip_rect(previous_clip.intersect(geometry.stage_rect));
            let interact = ui.interact(
                interact_rect,
                egui::Id::new(("scene_entity", raw_id)),
                sense,
            );

            if interact.clicked() || interact.double_clicked() {
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
            self.paint_entity(
                ui,
                &entity.kind,
                rect,
                is_selected,
                entity.transform.opacity,
            );
            if is_active && !is_selected {
                ui.painter().rect_stroke(
                    if is_background {
                        geometry.stage_rect.shrink(3.0)
                    } else {
                        rect.expand(3.0)
                    },
                    0.0,
                    egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(120, 220, 255)),
                );
            }
            if is_selected {
                ui.painter().rect_stroke(
                    if is_background {
                        geometry.stage_rect.shrink(2.0)
                    } else {
                        rect.expand(2.0)
                    },
                    0.0,
                    egui::Stroke::new(2.0_f32, egui::Color32::YELLOW),
                );
            }
            ui.set_clip_rect(previous_clip);
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
            entity.transform.x = entity.transform.x.saturating_add(delta.x.round() as i32);
            entity.transform.y = entity.transform.y.saturating_add(delta.y.round() as i32);
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
            egui::Stroke::new(1.0_f32, egui::Color32::from_gray(110)),
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
        let overrides = self.layer_overrides?;
        let stable_id = scene_entity_object_id(entity, source_node_id, index);
        self.entity_object_ids
            .get(&entity.id.raw())
            .and_then(|id| overrides.get(id))
            .or_else(|| overrides.get(&stable_id))
            .or_else(|| overrides.get(&format!("entity:{}", entity.id.raw())))
    }

    fn paint_entity(
        &mut self,
        ui: &mut egui::Ui,
        kind: &EntityKind,
        rect: egui::Rect,
        is_selected: bool,
        opacity: u32,
    ) {
        let mut content_ui = ui.child_ui(rect, *ui.layout(), None);
        content_ui.set_clip_rect(ui.clip_rect());
        content_ui.set_opacity(opacity.min(1000) as f32 / 1000.0);
        let ui = &content_ui;
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

    pub fn resolve_image_texture(
        &mut self,
        ctx: &egui::Context,
        asset_path: &str,
    ) -> Option<egui::TextureId> {
        ImageTextureCache::new(self.image_cache, self.image_failures, self.resource_service)
            .texture(
                ctx,
                self.project_root?,
                asset_path,
                ImageView::Scene(self.preview_quality),
            )
    }
}

pub fn entity_matches_active_node(
    source_node_id: Option<u32>,
    active_node_id: Option<u32>,
) -> bool {
    source_node_id.is_some() && source_node_id == active_node_id
}

pub(crate) fn character_instance_index(
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
