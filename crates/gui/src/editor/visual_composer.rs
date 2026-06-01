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
#[path = "visual_composer/panel_controls.rs"]
mod panel_controls;
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

pub fn composer_stage_available_height(
    visible_remaining: f32,
    selected_node: Option<&StoryNode>,
) -> f32 {
    if visible_remaining <= 0.0 {
        return 0.0;
    }
    let footer = composer_status_row_reserved_height(visible_remaining)
        + composer_overlay_editor_available_height(visible_remaining, selected_node);
    let minimum_stage = composer_minimum_stage_height(visible_remaining);
    (visible_remaining - footer)
        .max(minimum_stage)
        .min(visible_remaining)
}

pub fn composer_overlay_editor_available_height(
    visible_remaining: f32,
    selected_node: Option<&StoryNode>,
) -> f32 {
    let desired = overlay_editor::overlay_editor_reserved_height(selected_node);
    if desired <= 0.0 || visible_remaining <= 0.0 {
        return 0.0;
    }

    let header_height = 28.0_f32.min(visible_remaining);
    let status_height = composer_status_row_reserved_height(visible_remaining);
    let max_without_collapsing_stage =
        (visible_remaining - status_height - composer_minimum_stage_height(visible_remaining))
            .max(0.0);
    if visible_remaining < 360.0 {
        return header_height.min(max_without_collapsing_stage);
    }
    let max_editor = (visible_remaining * 0.26)
        .clamp(header_height, 140.0)
        .min(max_without_collapsing_stage)
        .min(visible_remaining);
    desired.min(max_editor)
}

pub fn composer_status_row_reserved_height(visible_remaining: f32) -> f32 {
    if visible_remaining < 32.0 {
        0.0
    } else if visible_remaining < 360.0 {
        20.0
    } else {
        26.0
    }
}

fn composer_minimum_stage_height(visible_remaining: f32) -> f32 {
    (visible_remaining * 0.62).max(96.0).min(visible_remaining)
}

pub fn composer_layer_list_height(available_height: f32) -> f32 {
    if available_height < 36.0 {
        return 0.0;
    }
    let header_budget = 28.0;
    let compact = available_height < 420.0;
    let fraction = if compact { 0.12 } else { 0.18 };
    let max_height = if compact { 48.0 } else { 96.0 };
    (available_height * fraction)
        .clamp(24.0, max_height)
        .min((available_height - header_budget).max(0.0))
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
        let layer_list_height = composer_layer_list_height(layer_remaining);
        if layer_list_height > 0.0 {
            if let Some(layer_action) = self.render_layer_panel(ui, &objects, layer_list_height) {
                action = Some(layer_action);
            }
        }
        let visible_remaining = (panel_clip.bottom() - ui.cursor().min.y).max(0.0);
        let viewport_available_height =
            composer_stage_available_height(visible_remaining, self.selected_authoring_node);
        let viewport_size = viewport::composer_viewport_size(
            egui::vec2(ui.available_width(), viewport_available_height),
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

        let overlay_editor_remaining = (panel_clip.bottom() - ui.cursor().min.y).max(0.0);
        let overlay_editor_available = composer_overlay_editor_available_height(
            overlay_editor_remaining,
            self.selected_authoring_node,
        );
        if overlay_editor_available >= 24.0 && ui.cursor().min.y < panel_clip.bottom() - 24.0 {
            ui.add_space(4.0);
            let editor_max_height = (panel_clip.bottom() - ui.cursor().min.y).max(0.0);
            egui::ScrollArea::vertical()
                .id_source("visual_composer_overlay_editor_scroll")
                .auto_shrink([false, false])
                .max_height(editor_max_height.min(overlay_editor_available))
                .show(ui, |ui| {
                    if let Some(edit_action) = overlay_editor::render_overlay_editor(
                        ui,
                        self.selected_authoring_node_id,
                        self.selected_authoring_node,
                    ) {
                        action = Some(edit_action);
                    }
                });
        }

        ui.set_clip_rect(previous_clip);
        action
    }

    fn stage_size(&self) -> (f32, f32) {
        let (w, h) = self.stage_resolution.unwrap_or((1280, 720));
        (w.max(1) as f32, h.max(1) as f32)
    }
}
