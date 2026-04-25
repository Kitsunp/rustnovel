use crate::editor::StoryNode;
use eframe::egui;
use std::collections::HashMap;
use std::path::Path;
use visual_novel_engine::{Engine, EntityId, SceneState};

use crate::editor::{PreviewQuality, StageFit};

pub enum ComposerNodeMutation {
    CharacterPosition {
        name: String,
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
}

impl<'a> VisualComposerPanel<'a> {
    pub fn new(
        scene: &'a mut SceneState,
        engine: &'a Option<Engine>,
        project_root: Option<&'a Path>,
        stage_resolution: Option<(u32, u32)>,
        preview_quality: &'a mut PreviewQuality,
        stage_fit: &'a mut StageFit,
        image_cache: &'a mut HashMap<String, egui::TextureHandle>,
        image_failures: &'a mut HashMap<String, String>,
        selected_entity_id: &'a mut Option<u32>,
    ) -> Self {
        Self {
            scene,
            engine,
            project_root,
            stage_resolution,
            preview_quality,
            stage_fit,
            image_cache,
            image_failures,
            selected_entity_id,
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
            self.render_runtime_controls(ui, &mut action);
        });
        ui.separator();

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
                    let node = match dragged.kind {
                        "char" => Some(StoryNode::ScenePatch(visual_novel_engine::ScenePatchRaw {
                            add: vec![visual_novel_engine::CharacterPlacementRaw {
                                name: dragged.name.to_string(),
                                expression: Some(dragged.path.to_string()),
                                position: None,
                                x: Some(pos.x.round() as i32),
                                y: Some(pos.y.round() as i32),
                                scale: Some(1.0),
                            }],
                            ..Default::default()
                        })),
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
        );
        let stage_action = painter.paint_interactive(
            ui,
            self.scene,
            geometry,
            self.selected_entity_id,
            entity_owners,
        );
        if let Some(node_id) = stage_action.selected_node {
            action = Some(VisualComposerAction::SelectNode(node_id));
        }
        if let Some(moved) = stage_action.moved_character {
            action = Some(VisualComposerAction::MutateNode {
                node_id: moved.node_id,
                mutation: ComposerNodeMutation::CharacterPosition {
                    name: moved.name,
                    x: moved.x,
                    y: moved.y,
                    scale: moved.scale,
                },
            });
        }

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

    fn render_runtime_controls(
        &self,
        ui: &mut egui::Ui,
        action: &mut Option<VisualComposerAction>,
    ) {
        let Some(engine) = self.engine else {
            return;
        };
        match engine.current_event() {
            Ok(visual_novel_engine::EventCompiled::Choice(choice)) => {
                for (idx, option) in choice.options.iter().enumerate() {
                    if ui.small_button(format!("Pick {}", idx + 1)).clicked() {
                        *action = Some(VisualComposerAction::TestChoose(idx));
                    }
                    ui.label(option.text.as_ref());
                }
            }
            Ok(_) => {
                if ui.small_button("Next").clicked() {
                    *action = Some(VisualComposerAction::TestAdvance);
                }
            }
            Err(_) => {}
        }
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

#[cfg(test)]
mod tests {
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
}
