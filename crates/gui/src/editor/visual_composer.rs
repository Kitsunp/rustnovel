use crate::editor::StoryNode;
use eframe::egui;
use std::collections::HashMap;
use std::path::Path;
use visual_novel_engine::{Engine, EntityId, SceneState};

use crate::editor::{AssetFieldTarget, PreviewQuality, StageFit};

mod layers;
mod overlays;
pub(crate) use layers::scene_entity_object_id;
pub use layers::{
    layered_scene_objects, layered_scene_objects_with_authoring_overlay, LayerOverride,
    LayeredSceneObject, StageLayerKind,
};

pub enum ComposerNodeMutation {
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
    image_cache: &'a mut HashMap<String, egui::TextureHandle>,
    image_failures: &'a mut HashMap<String, String>,
    selected_entity_id: &'a mut Option<u32>,
    layer_overrides: &'a mut HashMap<String, LayerOverride>,
    active_event_node_id: Option<u32>,
    selected_authoring_node_id: Option<u32>,
    selected_authoring_node: Option<&'a StoryNode>,
}

pub struct VisualComposerPanelParams<'a> {
    pub scene: &'a mut SceneState,
    pub engine: &'a Option<Engine>,
    pub project_root: Option<&'a Path>,
    pub stage_resolution: Option<(u32, u32)>,
    pub preview_quality: &'a mut PreviewQuality,
    pub stage_fit: &'a mut StageFit,
    pub image_cache: &'a mut HashMap<String, egui::TextureHandle>,
    pub image_failures: &'a mut HashMap<String, String>,
    pub selected_entity_id: &'a mut Option<u32>,
    pub layer_overrides: &'a mut HashMap<String, LayerOverride>,
    pub active_event_node_id: Option<u32>,
    pub selected_authoring_node_id: Option<u32>,
    pub selected_authoring_node: Option<&'a StoryNode>,
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
            image_cache: params.image_cache,
            image_failures: params.image_failures,
            selected_entity_id: params.selected_entity_id,
            layer_overrides: params.layer_overrides,
            active_event_node_id: params.active_event_node_id,
            selected_authoring_node_id: params.selected_authoring_node_id,
            selected_authoring_node: params.selected_authoring_node,
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
            if ui.small_button("Test here").clicked() {
                action = Some(VisualComposerAction::TestFromSelection);
            }
            if ui.small_button("Restart").clicked() {
                action = Some(VisualComposerAction::TestRestart);
            }
            overlays::render_runtime_controls(ui, self.engine, &mut action);
        });
        ui.separator();
        let objects = layered_scene_objects_with_authoring_overlay(
            self.scene,
            entity_owners,
            self.engine,
            self.selected_authoring_node_id,
            self.selected_authoring_node,
        );
        if let Some(layer_action) = self.render_layer_panel(ui, &objects) {
            action = Some(layer_action);
        }

        let available_size = ui.available_size();
        let status_height = 28.0;
        let viewport_height = (available_size.y - status_height).max(80.0);
        let viewport_rect = egui::Rect::from_min_size(
            ui.cursor().min,
            egui::vec2(available_size.x, viewport_height),
        );
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
                            "char" => {
                                Some(StoryNode::ScenePatch(visual_novel_engine::ScenePatchRaw {
                                    add: vec![visual_novel_engine::CharacterPlacementRaw {
                                        name: dragged.name.to_string(),
                                        expression: Some(dragged.path.to_string()),
                                        position: None,
                                        x: Some(pos.x.round() as i32),
                                        y: Some(pos.y.round() as i32),
                                        scale: Some(1.0),
                                    }],
                                    ..Default::default()
                                }))
                            }
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

        let mut painter = crate::editor::scene_stage::SceneStagePainter::new(
            self.project_root,
            *self.preview_quality,
            self.image_cache,
            self.image_failures,
        )
        .with_layer_overrides(self.layer_overrides.clone());
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
                                .entry(object.object_id.clone())
                                .or_insert(LayerOverride {
                                    visible: object.visible,
                                    locked: object.locked,
                                });
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut entry.visible, "").changed() {
                                    action = Some(VisualComposerAction::LayerVisibilityChanged {
                                        object_id: object.object_id.clone(),
                                        visible: entry.visible,
                                    });
                                }
                                if ui.checkbox(&mut entry.locked, "Lock").changed() {
                                    action = Some(VisualComposerAction::LayerLockChanged {
                                        object_id: object.object_id.clone(),
                                        locked: entry.locked,
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

struct DraggedAsset<'a> {
    kind: &'a str,
    name: &'a str,
    path: &'a str,
}

impl<'a> DraggedAsset<'a> {
    fn parse(payload: &'a str) -> Option<Self> {
        let payload = payload.strip_prefix("asset://")?;
        let mut lines = payload.lines();
        let header = lines.next()?;
        let path_override = lines.next();
        let (kind, name) = header.split_once('/')?;
        Some(Self {
            kind,
            name,
            path: path_override.unwrap_or(name),
        })
    }
}

fn short_event_label(event: &visual_novel_engine::EventCompiled) -> String {
    match event {
        visual_novel_engine::EventCompiled::Scene(scene) => {
            let background = scene.background.as_deref().unwrap_or("<none>");
            format!("Event: Scene bg={background}")
        }
        visual_novel_engine::EventCompiled::Patch(patch) => {
            let background = patch.background.as_deref().unwrap_or("<none>");
            format!("Event: Patch bg={background}")
        }
        visual_novel_engine::EventCompiled::Dialogue(dialogue) => {
            format!("Event: Dialogue {}", dialogue.speaker.as_ref())
        }
        visual_novel_engine::EventCompiled::Choice(choice) => {
            format!("Event: Choice {} option(s)", choice.options.len())
        }
        visual_novel_engine::EventCompiled::AudioAction(action) => {
            format!("Event: Audio {}", audio_channel_label(action.channel))
        }
        _ => format!("Event: {:?}", event),
    }
}

fn audio_channel_label(channel: u8) -> &'static str {
    match channel {
        0 => "bgm",
        1 => "sfx",
        2 => "voice",
        _ => "unknown",
    }
}

pub(crate) fn assignment_for_dropped_asset(
    kind: &str,
    asset_path: &str,
    selected_node_id: Option<u32>,
    selected_node: Option<&StoryNode>,
) -> Option<(u32, AssetFieldTarget, String)> {
    let node_id = selected_node_id?;
    let node = selected_node?;
    let target = match (kind, node) {
        ("bg", StoryNode::Scene { .. }) => AssetFieldTarget::SceneBackground,
        ("bg", StoryNode::ScenePatch(_)) => AssetFieldTarget::ScenePatchBackground,
        ("audio", StoryNode::Scene { .. }) => AssetFieldTarget::SceneMusic,
        ("audio", StoryNode::ScenePatch(_)) => AssetFieldTarget::ScenePatchMusic,
        ("audio", StoryNode::AudioAction { .. }) => AssetFieldTarget::AudioActionAsset,
        _ => return None,
    };
    Some((node_id, target, asset_path.to_string()))
}

pub(crate) fn character_drop_target_node(
    kind: &str,
    selected_node_id: Option<u32>,
    selected_node: Option<&StoryNode>,
) -> Option<u32> {
    let node_id = selected_node_id?;
    let node = selected_node?;
    (kind == "char" && matches!(node, StoryNode::Scene { .. } | StoryNode::ScenePatch(_)))
        .then_some(node_id)
}

#[cfg(test)]
mod tests {
    use crate::editor::{AssetFieldTarget, StoryNode};

    #[test]
    fn low_z_order_image_is_background_layer() {
        let image = visual_novel_engine::EntityKind::Image(visual_novel_engine::ImageData {
            path: visual_novel_engine::SharedStr::from("bg/room.png"),
            tint: None,
        });
        assert!(crate::editor::scene_stage::is_background_image(
            &image, -100
        ));
        assert!(!crate::editor::scene_stage::is_background_image(&image, 0));
    }

    #[test]
    fn dragged_character_payload_preserves_name_and_image_path() {
        let payload = "asset://char/furina\nassets/characters/furina.png";
        let parsed = super::DraggedAsset::parse(payload).expect("payload should parse");
        assert_eq!(parsed.kind, "char");
        assert_eq!(parsed.name, "furina");
        assert_eq!(parsed.path, "assets/characters/furina.png");
    }

    #[test]
    fn dropped_background_assigns_to_selected_scene_instead_of_creating_duplicate_scene() {
        let scene = StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: Vec::new(),
        };

        let assignment = super::assignment_for_dropped_asset(
            "bg",
            "assets/backgrounds/room.png",
            Some(9),
            Some(&scene),
        )
        .expect("selected scene should accept background drop");

        assert_eq!(
            assignment,
            (
                9,
                AssetFieldTarget::SceneBackground,
                "assets/backgrounds/room.png".to_string()
            )
        );
    }

    #[test]
    fn dropped_audio_can_target_scene_music_patch_music_or_audio_node() {
        let scene = StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: Vec::new(),
        };
        let patch = StoryNode::ScenePatch(Default::default());
        let audio = StoryNode::AudioAction {
            channel: "bgm".to_string(),
            action: "play".to_string(),
            asset: None,
            volume: None,
            fade_duration_ms: None,
            loop_playback: Some(true),
        };

        assert_eq!(
            super::assignment_for_dropped_asset(
                "audio",
                "assets/audio/theme.ogg",
                Some(1),
                Some(&scene)
            )
            .map(|(_, target, _)| target),
            Some(AssetFieldTarget::SceneMusic)
        );
        assert_eq!(
            super::assignment_for_dropped_asset(
                "audio",
                "assets/audio/theme.ogg",
                Some(2),
                Some(&patch)
            )
            .map(|(_, target, _)| target),
            Some(AssetFieldTarget::ScenePatchMusic)
        );
        assert_eq!(
            super::assignment_for_dropped_asset(
                "audio",
                "assets/audio/theme.ogg",
                Some(3),
                Some(&audio)
            )
            .map(|(_, target, _)| target),
            Some(AssetFieldTarget::AudioActionAsset)
        );
    }

    #[test]
    fn dropped_character_targets_selected_scene_instead_of_creating_duplicate_patch() {
        let scene = StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: Vec::new(),
        };

        assert!(super::assignment_for_dropped_asset(
            "char",
            "assets/characters/ava.png",
            Some(1),
            Some(&scene)
        )
        .is_none());
        assert_eq!(
            super::character_drop_target_node("char", Some(1), Some(&scene)),
            Some(1)
        );
    }
}
