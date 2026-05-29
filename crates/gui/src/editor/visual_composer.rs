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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposerChromeMode {
    Full,
    Compact,
    Narrow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComposerChromeLayout {
    pub mode: ComposerChromeMode,
    pub quality_width: f32,
    pub fit_width: f32,
    pub preview_width: f32,
    pub source_wrap_chars: usize,
    pub show_full_labels: bool,
}

pub fn composer_chrome_layout(available_width: f32) -> ComposerChromeLayout {
    let width = available_width.max(1.0);
    if width < 320.0 {
        ComposerChromeLayout {
            mode: ComposerChromeMode::Narrow,
            quality_width: 58.0,
            fit_width: 48.0,
            preview_width: 72.0,
            source_wrap_chars: 12,
            show_full_labels: false,
        }
    } else if width < 500.0 {
        ComposerChromeLayout {
            mode: ComposerChromeMode::Narrow,
            quality_width: 76.0,
            fit_width: 66.0,
            preview_width: 92.0,
            source_wrap_chars: 16,
            show_full_labels: false,
        }
    } else if width < 760.0 {
        ComposerChromeLayout {
            mode: ComposerChromeMode::Compact,
            quality_width: 90.0,
            fit_width: 76.0,
            preview_width: 110.0,
            source_wrap_chars: 22,
            show_full_labels: false,
        }
    } else {
        ComposerChromeLayout {
            mode: ComposerChromeMode::Full,
            quality_width: 118.0,
            fit_width: 96.0,
            preview_width: 132.0,
            source_wrap_chars: 28,
            show_full_labels: true,
        }
    }
}

pub fn visual_composer_heading_label(available_width: f32) -> &'static str {
    if available_width < 420.0 {
        "Composer"
    } else {
        "Visual Composer"
    }
}

fn preview_quality_label(quality: PreviewQuality, mode: ComposerChromeMode) -> &'static str {
    if mode == ComposerChromeMode::Full {
        return quality.label();
    }
    match quality {
        PreviewQuality::Draft => "Draft",
        PreviewQuality::Balanced => "Balanced",
        PreviewQuality::High => "High",
    }
}

fn preview_mode_label(
    mode_value: ComposerPreviewMode,
    chrome_mode: ComposerChromeMode,
) -> &'static str {
    if chrome_mode == ComposerChromeMode::Full {
        return mode_value.label();
    }
    match mode_value {
        ComposerPreviewMode::IsolatedNode => "Isolated",
        ComposerPreviewMode::RuntimeInherited => "Inherited",
        ComposerPreviewMode::RuntimeFromStart => "From start",
        ComposerPreviewMode::SelectedRoute => "Route",
    }
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
        let previous_clip = ui.clip_rect();
        let panel_clip = previous_clip.intersect(ui.available_rect_before_wrap());
        ui.set_clip_rect(panel_clip);
        ui.set_max_width(panel_clip.width().max(1.0));
        ui.set_width(panel_clip.width().max(1.0));
        let chrome = composer_chrome_layout(ui.available_width());
        ui.heading(visual_composer_heading_label(ui.available_width()));
        self.render_metadata_row(ui, entity_owners, chrome);
        self.render_control_rows(ui, chrome, &mut action);
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
        let layer_remaining = (panel_clip.bottom() - ui.cursor().min.y).max(0.0);
        if layer_remaining >= 36.0 {
            let layer_list_height = (layer_remaining - 28.0).clamp(24.0, 120.0);
            if let Some(layer_action) = self.render_layer_panel(ui, &objects, layer_list_height) {
                action = Some(layer_action);
            }
        }
        if ui.cursor().min.y < panel_clip.bottom() - 24.0 {
            if let Some(edit_action) = overlay_editor::render_overlay_editor(
                ui,
                self.selected_authoring_node_id,
                self.selected_authoring_node,
            ) {
                action = Some(edit_action);
            }
        }

        let visible_remaining = (panel_clip.bottom() - ui.cursor().min.y).max(0.0);
        let viewport_size = viewport::composer_viewport_size(
            egui::vec2(ui.available_width(), visible_remaining),
            self.stage_size(),
        );
        let viewport_rect = egui::Rect::from_min_size(ui.cursor().min, viewport_size);
        if viewport_rect.is_positive() && viewport_rect.height() > 1.0 {
            let geometry = crate::editor::scene_stage::stage_geometry(
                viewport_rect,
                self.stage_size(),
                *self.stage_fit,
            );

            let response = ui.allocate_rect(viewport_rect, egui::Sense::click());
            ui.advance_cursor_after_rect(viewport_rect);

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

                        ui.memory_mut(|mem| {
                            mem.data.remove::<String>(egui::Id::new("dragged_asset"))
                        });
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
        }

        if ui.cursor().min.y < panel_clip.bottom() - 12.0 {
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
        }

        ui.set_clip_rect(previous_clip);
        action
    }

    fn render_metadata_row(
        &self,
        ui: &mut egui::Ui,
        entity_owners: &HashMap<u32, u32>,
        chrome: ComposerChromeLayout,
    ) {
        ui.horizontal_wrapped(|ui| {
            let (w, h) = self.stage_size();
            let stage_label = if chrome.show_full_labels {
                format!("Stage: {}x{}", w as u32, h as u32)
            } else {
                format!("{}x{}", w as u32, h as u32)
            };
            ui.label(stage_label);
            ui.separator();
            ui.label(format!("Entities: {}", self.scene.len()));
            ui.separator();
            let source = preview_source_label(
                self.scene,
                self.engine,
                *self.preview_mode,
                self.selected_authoring_node_id,
                self.selected_authoring_node,
                entity_owners,
            );
            ui.add(
                egui::Label::new(crate::player_overlay::soft_wrap_long_tokens(
                    source,
                    chrome.source_wrap_chars,
                ))
                .wrap(true),
            );
        });
    }

    fn render_control_rows(
        &mut self,
        ui: &mut egui::Ui,
        chrome: ComposerChromeLayout,
        action: &mut Option<VisualComposerAction>,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            self.render_quality_view_background_controls(ui, chrome, action);
            if chrome.mode == ComposerChromeMode::Full {
                ui.separator();
                self.render_preview_run_controls(ui, chrome, action);
            }
        });
        if chrome.mode != ComposerChromeMode::Full {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                self.render_preview_run_controls(ui, chrome, action);
            });
        }
    }

    fn render_quality_view_background_controls(
        &mut self,
        ui: &mut egui::Ui,
        chrome: ComposerChromeLayout,
        action: &mut Option<VisualComposerAction>,
    ) {
        ui.label(if chrome.show_full_labels {
            "Pixels:"
        } else {
            "Px"
        });
        egui::ComboBox::from_id_source("composer_preview_quality")
            .width(chrome.quality_width)
            .selected_text(preview_quality_label(*self.preview_quality, chrome.mode))
            .show_ui(ui, |ui| {
                for quality in PreviewQuality::ALL {
                    ui.selectable_value(self.preview_quality, *quality, quality.label());
                }
            });
        ui.label(if chrome.show_full_labels {
            "View:"
        } else {
            "View"
        });
        egui::ComboBox::from_id_source("composer_stage_fit")
            .width(chrome.fit_width)
            .selected_text(self.stage_fit.label())
            .show_ui(ui, |ui| {
                for fit in StageFit::ALL {
                    ui.selectable_value(self.stage_fit, *fit, fit.label());
                }
            });
        let old_background_fit = *self.background_fit;
        ui.label("BG");
        egui::ComboBox::from_id_source("composer_background_fit")
            .width(chrome.fit_width)
            .selected_text(self.background_fit.label())
            .show_ui(ui, |ui| {
                for fit in BackgroundFit::ALL {
                    ui.selectable_value(self.background_fit, *fit, fit.label());
                }
            });
        if old_background_fit != *self.background_fit {
            *action = Some(VisualComposerAction::BackgroundFitChanged {
                node_id: self.selected_authoring_node_id,
                fit: *self.background_fit,
            });
        }
    }

    fn render_preview_run_controls(
        &mut self,
        ui: &mut egui::Ui,
        chrome: ComposerChromeLayout,
        action: &mut Option<VisualComposerAction>,
    ) {
        ui.label(if chrome.show_full_labels {
            "Preview:"
        } else {
            "Preview"
        });
        let old_preview_mode = *self.preview_mode;
        egui::ComboBox::from_id_source("composer_preview_mode")
            .width(chrome.preview_width)
            .selected_text(preview_mode_label(*self.preview_mode, chrome.mode))
            .show_ui(ui, |ui| {
                for mode in ComposerPreviewMode::ALL {
                    ui.selectable_value(self.preview_mode, *mode, mode.label());
                }
            });
        if old_preview_mode != *self.preview_mode {
            *action = Some(VisualComposerAction::PreviewModeChanged(*self.preview_mode));
        }
        if ui.small_button("Test here").clicked() {
            *action = Some(VisualComposerAction::TestFromSelection);
        }
        if ui.small_button("Restart").clicked() {
            *action = Some(VisualComposerAction::TestRestart);
        }
        overlays::render_runtime_controls(ui, self.engine, action);
    }

    fn render_layer_panel(
        &mut self,
        ui: &mut egui::Ui,
        objects: &[LayeredSceneObject],
        max_height: f32,
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
                    .max_height(max_height)
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width().max(1.0));
                        for object in objects.iter().rev() {
                            let entry = self
                                .layer_overrides
                                .get(&object.object_id)
                                .copied()
                                .unwrap_or(LayerOverride {
                                    visible: object.visible,
                                    locked: object.locked,
                                });
                            ui.horizontal_wrapped(|ui| {
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
                                let source_label = format!(
                                    "{} | z={} | {}",
                                    object.kind.label(),
                                    object.z_index,
                                    object.source_field_path
                                );
                                let label_width = ui.available_width().max(1.0);
                                ui.add_sized(
                                    [label_width, ui.spacing().interact_size.y],
                                    egui::Label::new(crate::player_overlay::soft_wrap_long_tokens(
                                        &source_label,
                                        28,
                                    ))
                                    .wrap(true),
                                );
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
