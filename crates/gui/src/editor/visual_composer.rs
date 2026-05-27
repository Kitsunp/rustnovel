use crate::editor::StoryNode;
use eframe::egui;
use std::collections::HashMap;
use std::path::Path;
use visual_novel_engine::{
    authoring::composer::PresentationSnapshot, runtime::Engine, EntityId, SceneState,
};

use crate::editor::resource_service::EditorResourceService;
use crate::editor::{
    AssetFieldTarget, BackgroundFit, ComposerPreviewMode, PreviewQuality, StageFit,
};

pub mod drop_target;
pub mod layers;
pub mod overlay_editor;
pub mod overlays;
pub mod preview_badge;
pub mod viewport;
pub use drop_target::{assignment_for_dropped_asset, character_drop_target_node, DraggedAsset};
pub use layers::scene_entity_object_id;
pub use layers::{
    layered_scene_objects, layered_scene_objects_with_authoring_overlay, LayerOverride,
    LayeredSceneObject, StageLayerKind,
};
pub use preview_badge::preview_source_label;
use preview_badge::short_event_label;

pub enum ComposerNodeMutation {
    DialogueText {
        speaker: String,
        text: String,
    },
    ChoicePrompt {
        prompt: String,
    },
    ChoiceOptionText {
        option_index: usize,
        text: String,
    },
    ChoiceOptionOrder {
        from_index: usize,
        to_index: usize,
    },
    ChoiceOptionTarget {
        option_index: usize,
        target_node_id: Option<u32>,
    },
    CharacterPosition {
        name: String,
        expression: Option<String>,
        source_instance_index: usize,
        x: i32,
        y: i32,
        scale: Option<f32>,
    },
}

pub enum VisualComposerAction {
    SelectNode(u32),
    CreateNode {
        node: StoryNode,
        pos: egui::Pos2,
    },
    MutateNode {
        node_id: u32,
        mutation: ComposerNodeMutation,
    },
    AssignAssetToNode {
        node_id: u32,
        target: AssetFieldTarget,
        asset: String,
    },
    AddCharacterToNode {
        node_id: u32,
        name: String,
        asset: String,
        x: i32,
        y: i32,
    },
    LayerVisibilityChanged {
        object_id: String,
        visible: bool,
    },
    LayerLockChanged {
        object_id: String,
        locked: bool,
    },
    BackgroundFitChanged {
        node_id: Option<u32>,
        fit: BackgroundFit,
    },
    PreviewModeChanged(ComposerPreviewMode),
    TestFromSelection,
    TestRestart,
    TestAdvance,
    TestChoose(usize),
}

/// The WYSIWYG Scene Composer.
pub struct VisualComposerPanel<'a> {
    scene: &'a mut SceneState,
    engine: &'a Option<Engine>,
    project_root: Option<&'a Path>,
    stage_resolution: Option<(u32, u32)>,
    preview_quality: &'a mut PreviewQuality,
    stage_fit: &'a mut StageFit,
    background_fit: &'a mut BackgroundFit,
    preview_mode: &'a mut ComposerPreviewMode,
    image_cache: &'a mut HashMap<String, egui::TextureHandle>,
    image_failures: &'a mut HashMap<String, String>,
    resource_service: &'a mut EditorResourceService,
    selected_entity_id: &'a mut Option<u32>,
    layer_overrides: &'a HashMap<String, LayerOverride>,
    active_event_node_id: Option<u32>,
    selected_authoring_node_id: Option<u32>,
    selected_authoring_node: Option<&'a StoryNode>,
    presentation_snapshot: Option<&'a PresentationSnapshot>,
}

pub struct VisualComposerPanelParams<'a> {
    pub scene: &'a mut SceneState,
    pub engine: &'a Option<Engine>,
    pub project_root: Option<&'a Path>,
    pub stage_resolution: Option<(u32, u32)>,
    pub preview_quality: &'a mut PreviewQuality,
    pub stage_fit: &'a mut StageFit,
    pub background_fit: &'a mut BackgroundFit,
    pub preview_mode: &'a mut ComposerPreviewMode,
    pub image_cache: &'a mut HashMap<String, egui::TextureHandle>,
    pub image_failures: &'a mut HashMap<String, String>,
    pub resource_service: &'a mut EditorResourceService,
    pub selected_entity_id: &'a mut Option<u32>,
    pub layer_overrides: &'a HashMap<String, LayerOverride>,
    pub active_event_node_id: Option<u32>,
    pub selected_authoring_node_id: Option<u32>,
    pub selected_authoring_node: Option<&'a StoryNode>,
    pub presentation_snapshot: Option<&'a PresentationSnapshot>,
}

impl<'a> VisualComposerPanel<'a> {
    pub fn new(params: VisualComposerPanelParams<'a>) -> Self {
        Self {
            scene: params.scene,
            engine: params.engine,
            project_root: params.project_root,
            stage_resolution: params.stage_resolution,
            preview_quality: params.preview_quality,
            stage_fit: params.stage_fit,
            background_fit: params.background_fit,
            preview_mode: params.preview_mode,
            image_cache: params.image_cache,
            image_failures: params.image_failures,
            resource_service: params.resource_service,
            selected_entity_id: params.selected_entity_id,
            layer_overrides: params.layer_overrides,
            active_event_node_id: params.active_event_node_id,
            selected_authoring_node_id: params.selected_authoring_node_id,
            selected_authoring_node: params.selected_authoring_node,
            presentation_snapshot: params.presentation_snapshot,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        entity_owners: &HashMap<u32, u32>,
    ) -> Option<VisualComposerAction> {
        let mut action = None;
        ui.heading("Visual Composer");
        ui.horizontal_wrapped(|ui| {
            let (w, h) = self.stage_size();
            ui.label(format!("Stage: {}x{}", w as u32, h as u32));
            ui.separator();
            ui.label(format!("Entities: {}", self.scene.len()));
            ui.separator();
            ui.label(preview_source_label(
                self.scene,
                self.engine,
                *self.preview_mode,
                self.selected_authoring_node_id,
                self.selected_authoring_node,
                entity_owners,
            ));
            ui.separator();
            ui.label("Pixels:");
            egui::ComboBox::from_id_source("composer_preview_quality")
                .selected_text(self.preview_quality.label())
                .show_ui(ui, |ui| {
                    for quality in PreviewQuality::ALL {
                        ui.selectable_value(self.preview_quality, *quality, quality.label());
                    }
                });
            ui.separator();
            ui.label("View:");
            egui::ComboBox::from_id_source("composer_stage_fit")
                .selected_text(self.stage_fit.label())
                .show_ui(ui, |ui| {
                    for fit in StageFit::ALL {
                        ui.selectable_value(self.stage_fit, *fit, fit.label());
                    }
                });
            ui.separator();
            let old_background_fit = *self.background_fit;
            ui.label("BG:");
            egui::ComboBox::from_id_source("composer_background_fit")
                .selected_text(self.background_fit.label())
                .show_ui(ui, |ui| {
                    for fit in BackgroundFit::ALL {
                        ui.selectable_value(self.background_fit, *fit, fit.label());
                    }
                });
            if old_background_fit != *self.background_fit {
                action = Some(VisualComposerAction::BackgroundFitChanged {
                    node_id: self.selected_authoring_node_id,
                    fit: *self.background_fit,
                });
            }
            ui.separator();
            let old_preview_mode = *self.preview_mode;
            ui.label("Preview:");
            egui::ComboBox::from_id_source("composer_preview_mode")
                .selected_text(self.preview_mode.label())
                .show_ui(ui, |ui| {
                    for mode in ComposerPreviewMode::ALL {
                        ui.selectable_value(self.preview_mode, *mode, mode.label());
                    }
                });
            if old_preview_mode != *self.preview_mode {
                action = Some(VisualComposerAction::PreviewModeChanged(*self.preview_mode));
            }
            ui.separator();
            if ui.small_button("Test here").clicked() {
                action = Some(VisualComposerAction::TestFromSelection);
            }
            if ui.small_button("Restart").clicked() {
                action = Some(VisualComposerAction::TestRestart);
            }
            overlays::render_runtime_controls(ui, self.engine, &mut action);
        });
        ui.separator();
        let objects = self
            .presentation_snapshot
            .map(|snapshot| snapshot.objects.clone())
            .unwrap_or_else(|| {
                layered_scene_objects_with_authoring_overlay(
                    self.scene,
                    entity_owners,
                    self.engine,
                    self.selected_authoring_node_id,
                    self.selected_authoring_node,
                )
            });
        if let Some(layer_action) = self.render_layer_panel(ui, &objects) {
            action = Some(layer_action);
        }
        if let Some(edit_action) = overlay_editor::render_overlay_editor(
            ui,
            self.selected_authoring_node_id,
            self.selected_authoring_node,
        ) {
            action = Some(edit_action);
        }

        let viewport_size =
            viewport::composer_viewport_size(ui.available_size(), self.stage_size());
        let viewport_rect = egui::Rect::from_min_size(ui.cursor().min, viewport_size);
        let geometry = crate::editor::scene_stage::stage_geometry(
            viewport_rect,
            self.stage_size(),
            *self.stage_fit,
        );

        let response = ui.allocate_rect(viewport_rect, egui::Sense::click());

        if response.hovered() && ui.input(|input| input.pointer.any_released()) {
            if let Some(payload) =
                ui.memory(|mem| mem.data.get_temp::<String>(egui::Id::new("dragged_asset")))
            {
                if let Some(dragged) = DraggedAsset::parse(&payload) {
                    let drop_pos = response.hover_pos().unwrap_or(viewport_rect.center());
                    let local = (drop_pos - geometry.stage_rect.min) / geometry.scale;
                    let pos = egui::pos2(local.x.max(0.0), local.y.max(0.0));
                    if let Some((node_id, target, asset)) = assignment_for_dropped_asset(
                        dragged.kind,
                        dragged.path,
                        self.selected_authoring_node_id,
                        self.selected_authoring_node,
                    ) {
                        action = Some(VisualComposerAction::AssignAssetToNode {
                            node_id,
                            target,
                            asset,
                        });
                    } else if let Some(node_id) = character_drop_target_node(
                        dragged.kind,
                        self.selected_authoring_node_id,
                        self.selected_authoring_node,
                    ) {
                        action = Some(VisualComposerAction::AddCharacterToNode {
                            node_id,
                            name: dragged.name.to_string(),
                            asset: dragged.path.to_string(),
                            x: pos.x.round() as i32,
                            y: pos.y.round() as i32,
                        });
                    } else {
                        let node = match dragged.kind {
                            "char" => Some(StoryNode::ScenePatch(
                                visual_novel_engine::runtime::ScenePatchRaw {
                                    add: vec![
                                        visual_novel_engine::runtime::CharacterPlacementRaw {
                                            name: dragged.name.to_string(),
                                            expression: Some(dragged.path.to_string()),
                                            position: None,
                                            x: Some(pos.x.round() as i32),
                                            y: Some(pos.y.round() as i32),
                                            scale: Some(1.0),
                                        },
                                    ],
                                    ..Default::default()
                                },
                            )),
                            "bg" => Some(StoryNode::Scene {
                                profile: None,
                                background: Some(dragged.path.to_string()),
                                music: None,
                                characters: Vec::new(),
                            }),
                            "audio" => Some(StoryNode::AudioAction {
                                channel: "bgm".to_string(),
                                action: "play".to_string(),
                                asset: Some(dragged.path.to_string()),
                                volume: None,
                                fade_duration_ms: None,
                                loop_playback: Some(true),
                            }),
                            _ => None,
                        };

                        if let Some(node) = node {
                            action = Some(VisualComposerAction::CreateNode { node, pos });
                        }
                    }

                    ui.memory_mut(|mem| mem.data.remove::<String>(egui::Id::new("dragged_asset")));
                }
            }
        }

        if response.clicked() {
            *self.selected_entity_id = None;
        }

        let painter = crate::editor::scene_stage::SceneStagePainter::new(
            self.project_root,
            *self.preview_quality,
            self.image_cache,
            self.image_failures,
            self.resource_service,
        )
        .with_layer_overrides(self.layer_overrides.clone());
        let mut painter = painter.with_background_fit(*self.background_fit);
        let stage_action = painter.paint_interactive(
            ui,
            self.scene,
            geometry,
            self.selected_entity_id,
            entity_owners,
            self.active_event_node_id,
        );
        if let Some(node_id) = stage_action.selected_node {
            action = Some(VisualComposerAction::SelectNode(node_id));
        }
        if let Some(moved) = stage_action.moved_character {
            action = Some(VisualComposerAction::MutateNode {
                node_id: moved.node_id,
                mutation: ComposerNodeMutation::CharacterPosition {
                    name: moved.name,
                    expression: moved.expression,
                    source_instance_index: moved.source_instance_index,
                    x: moved.x,
                    y: moved.y,
                    scale: moved.scale,
                },
            });
        }
        overlays::render_runtime_overlay(
            ui,
            geometry,
            self.engine,
            self.selected_authoring_node,
            *self.preview_mode,
            self.layer_overrides,
            &mut action,
        );

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Entities: {}", self.scene.len()));
            if let Some(sel) = *self.selected_entity_id {
                ui.label(format!("Selected: #{}", sel));
                if let Some(entity) = self.scene.get(EntityId::new(sel)) {
                    ui.label(format!(
                        "Pos: ({}, {})",
                        entity.transform.x, entity.transform.y
                    ));
                }
            }

            if let Some(engine) = self.engine {
                if let Ok(event) = engine.current_event() {
                    ui.separator();
                    ui.label(short_event_label(&event));
                }
            }
        });

        action
    }

    fn render_layer_panel(
        &mut self,
        ui: &mut egui::Ui,
        objects: &[LayeredSceneObject],
    ) -> Option<VisualComposerAction> {
        let mut action = None;
        egui::CollapsingHeader::new("Layers")
            .default_open(true)
            .show(ui, |ui| {
                if objects.is_empty() {
                    ui.label("No layers");
                    return;
                }
                egui::ScrollArea::vertical()
                    .max_height(120.0)
                    .show(ui, |ui| {
                        for object in objects.iter().rev() {
                            let entry = self
                                .layer_overrides
                                .get(&object.object_id)
                                .copied()
                                .unwrap_or(LayerOverride {
                                    visible: object.visible,
                                    locked: object.locked,
                                });
                            ui.horizontal(|ui| {
                                let mut visible = entry.visible;
                                if ui.checkbox(&mut visible, "").changed() {
                                    action = Some(VisualComposerAction::LayerVisibilityChanged {
                                        object_id: object.object_id.clone(),
                                        visible,
                                    });
                                }
                                let mut locked = entry.locked;
                                if ui.checkbox(&mut locked, "Lock").changed() {
                                    action = Some(VisualComposerAction::LayerLockChanged {
                                        object_id: object.object_id.clone(),
                                        locked,
                                    });
                                }
                                if object.source_node_id == self.active_event_node_id {
                                    ui.label(
                                        egui::RichText::new("active")
                                            .color(egui::Color32::from_rgb(120, 220, 255)),
                                    );
                                }
                                ui.label(format!(
                                    "{} | z={} | {}",
                                    object.kind.label(),
                                    object.z_index,
                                    object.source_field_path
                                ));
                            });
                        }
                    });
            });
        action
    }

    fn stage_size(&self) -> (f32, f32) {
        let (w, h) = self.stage_resolution.unwrap_or((1280, 720));
        (w.max(1) as f32, h.max(1) as f32)
    }
}
