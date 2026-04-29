use crate::editor::StoryNode;
use eframe::egui;

use super::{InspectorAction, InspectorPanel};

#[path = "inspector_panel_asset_fields.rs"]
mod asset_fields;
#[path = "inspector_panel_audio_section.rs"]
mod audio_section;
#[path = "inspector_panel_node_sections.rs"]
mod node_sections;

#[derive(Default)]
struct NodeEditActions {
    delete_option_idx: Option<usize>,
    add_option_req: bool,
    save_scene_profile_req: Option<String>,
    apply_scene_profile_req: Option<String>,
    inspector_action: Option<InspectorAction>,
}

impl<'a> InspectorPanel<'a> {
    pub(super) fn render_node_editor(&mut self, ui: &mut egui::Ui) -> Option<InspectorAction> {
        let mut actions = NodeEditActions::default();
        let mut standard_changed = false;
        let scene_profile_names = self.graph.scene_profile_names();

        let Some(node_id) = self.selected_node else {
            ui.label("No node selected");
            return None;
        };

        if let Some(node) = self.graph.get_node_mut(node_id) {
            ui.label(format!("Node ID: {}", node_id));
            ui.separator();

            match node {
                StoryNode::Dialogue { speaker, text } => {
                    node_sections::render_dialogue_node(ui, speaker, text, &mut standard_changed);
                }
                StoryNode::Choice { prompt, options } => {
                    node_sections::render_choice_node(
                        ui,
                        prompt,
                        options,
                        &mut standard_changed,
                        &mut actions,
                    );
                }
                StoryNode::Scene {
                    profile,
                    background,
                    music,
                    characters,
                } => {
                    node_sections::render_scene_node(
                        ui,
                        node_id,
                        node_sections::SceneNodeRefs {
                            profile,
                            background,
                            music,
                            characters,
                        },
                        &scene_profile_names,
                        &mut standard_changed,
                        &mut actions,
                    );
                }
                StoryNode::Jump { target } => {
                    ui.label("Jump Target (Label):");
                    standard_changed |= ui.text_edit_singleline(target).changed();
                }
                StoryNode::Start => {
                    ui.label("Start Node (Entry Point)");
                }
                StoryNode::End => {
                    ui.label("End Node (Termination)");
                }
                StoryNode::SetVariable { key, value } => {
                    ui.label("Variable Name:");
                    standard_changed |= ui.text_edit_singleline(key).changed();
                    ui.label("Value (i32):");
                    standard_changed |= ui.add(egui::DragValue::new(value)).changed();
                }
                StoryNode::SetFlag { key, value } => {
                    ui.label("Flag Name:");
                    standard_changed |= ui.text_edit_singleline(key).changed();
                    standard_changed |= ui.checkbox(value, "Set").changed();
                }
                StoryNode::JumpIf { target, cond } => {
                    node_sections::render_jump_if_node(ui, target, cond, &mut standard_changed);
                }
                StoryNode::ScenePatch(patch) => {
                    node_sections::render_scene_patch_node(
                        ui,
                        node_id,
                        patch,
                        &mut standard_changed,
                        &mut actions,
                    );
                }
                StoryNode::Generic(event) => {
                    render_generic_event_editor(ui, node_id, event, &mut standard_changed);
                }
                StoryNode::AudioAction {
                    channel,
                    action,
                    asset,
                    volume,
                    fade_duration_ms,
                    loop_playback,
                } => {
                    audio_section::render_audio_action_node(
                        ui,
                        node_id,
                        audio_section::AudioActionRefs {
                            channel,
                            action,
                            asset,
                            volume,
                            fade_duration_ms,
                            loop_playback,
                        },
                        &mut standard_changed,
                        &mut actions,
                    );
                }
                StoryNode::Transition {
                    kind,
                    duration_ms,
                    color,
                } => {
                    node_sections::render_transition_node(
                        ui,
                        kind,
                        duration_ms,
                        color,
                        &mut standard_changed,
                    );
                }
                StoryNode::CharacterPlacement { name, x, y, scale } => {
                    node_sections::render_character_placement_node(
                        ui,
                        name,
                        x,
                        y,
                        scale,
                        &mut standard_changed,
                    );
                }
            }

            if standard_changed {
                self.graph.mark_modified();
            }
        } else {
            ui.label("Node not found in editor graph.");
            return None;
        }

        if let Some(idx) = actions.delete_option_idx {
            self.graph.remove_choice_option(node_id, idx);
        }

        if actions.add_option_req {
            if let Some(StoryNode::Choice { options, .. }) = self.graph.get_node_mut(node_id) {
                options.push("New Option".to_string());
                self.graph.mark_modified();
            }
        }

        if let Some(profile_id) = actions.save_scene_profile_req {
            let _ = self.graph.save_scene_profile(profile_id, node_id);
        }
        if let Some(profile_id) = actions.apply_scene_profile_req {
            let _ = self.graph.apply_scene_profile(&profile_id, node_id);
        }
        actions.inspector_action
    }
}

fn render_generic_event_editor(
    ui: &mut egui::Ui,
    node_id: u32,
    event: &mut visual_novel_engine::EventRaw,
    standard_changed: &mut bool,
) {
    ui.label("Generic Event JSON");
    let buffer_id = egui::Id::new(("generic_event_json", node_id));
    let mut json = ui
        .ctx()
        .data_mut(|data| data.get_persisted::<String>(buffer_id))
        .unwrap_or_else(|| event.to_json_string());

    if ui
        .add(
            egui::TextEdit::multiline(&mut json)
                .code_editor()
                .desired_rows(8),
        )
        .changed()
    {
        ui.ctx()
            .data_mut(|data| data.insert_persisted(buffer_id, json.clone()));
        match parse_generic_event_json(&json) {
            Ok(updated) => {
                *event = updated;
                *standard_changed = true;
                ui.ctx()
                    .data_mut(|data| data.insert_persisted(buffer_id, event.to_json_string()));
            }
            Err(err) => {
                ui.colored_label(egui::Color32::YELLOW, format!("Invalid event JSON: {err}"));
            }
        }
    }
}

fn parse_generic_event_json(json: &str) -> Result<visual_novel_engine::EventRaw, String> {
    serde_json::from_str(json).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_event_json_parser_accepts_extcall() {
        let event = parse_generic_event_json(
            r#"{"type":"ext_call","command":"show_overlay","args":["inventory"]}"#,
        )
        .expect("valid ext call");

        match event {
            visual_novel_engine::EventRaw::ExtCall { command, args } => {
                assert_eq!(command, "show_overlay");
                assert_eq!(args, vec!["inventory".to_string()]);
            }
            _ => panic!("expected ext call"),
        }
    }

    #[test]
    fn generic_event_json_parser_rejects_invalid_payload() {
        assert!(parse_generic_event_json(r#"{"type":"unknown"}"#).is_err());
    }
}
