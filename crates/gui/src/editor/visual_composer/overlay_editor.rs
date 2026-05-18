use eframe::egui;

use super::{ComposerNodeMutation, VisualComposerAction};
use crate::editor::StoryNode;

pub(super) fn render_overlay_editor(
    ui: &mut egui::Ui,
    selected_node_id: Option<u32>,
    selected_node: Option<&StoryNode>,
) -> Option<VisualComposerAction> {
    let node_id = selected_node_id?;
    match selected_node? {
        StoryNode::Dialogue { speaker, text } => {
            render_dialogue_editor(ui, node_id, speaker.as_str(), text.as_str())
        }
        StoryNode::Choice { prompt, options } => {
            render_choice_editor(ui, node_id, prompt.as_str(), options)
        }
        _ => None,
    }
}

fn render_dialogue_editor(
    ui: &mut egui::Ui,
    node_id: u32,
    speaker: &str,
    text: &str,
) -> Option<VisualComposerAction> {
    let mut action = None;
    egui::CollapsingHeader::new("Overlay edit")
        .default_open(true)
        .show(ui, |ui| {
            let mut next_speaker = speaker.to_string();
            let mut next_text = text.to_string();
            ui.horizontal(|ui| {
                ui.label("Speaker");
                if ui.text_edit_singleline(&mut next_speaker).changed() {
                    action = Some(dialogue_action(
                        node_id,
                        next_speaker.clone(),
                        next_text.clone(),
                    ));
                }
            });
            ui.label("Text");
            if ui
                .add(egui::TextEdit::multiline(&mut next_text).desired_rows(3))
                .changed()
            {
                action = Some(dialogue_action(node_id, next_speaker, next_text));
            }
        });
    action
}

fn render_choice_editor(
    ui: &mut egui::Ui,
    node_id: u32,
    prompt: &str,
    options: &[String],
) -> Option<VisualComposerAction> {
    let mut action = None;
    egui::CollapsingHeader::new("Overlay edit")
        .default_open(true)
        .show(ui, |ui| {
            let mut next_prompt = prompt.to_string();
            ui.label("Prompt");
            if ui
                .add(egui::TextEdit::multiline(&mut next_prompt).desired_rows(2))
                .changed()
            {
                action = Some(VisualComposerAction::MutateNode {
                    node_id,
                    mutation: ComposerNodeMutation::ChoicePrompt {
                        prompt: next_prompt,
                    },
                });
            }
            ui.separator();
            for (idx, option) in options.iter().enumerate() {
                ui.horizontal(|ui| {
                    let mut next_option = option.clone();
                    if ui.text_edit_singleline(&mut next_option).changed() {
                        action = Some(VisualComposerAction::MutateNode {
                            node_id,
                            mutation: ComposerNodeMutation::ChoiceOptionText {
                                option_index: idx,
                                text: next_option,
                            },
                        });
                    }
                    if ui.small_button("Up").clicked() && idx > 0 {
                        action = Some(VisualComposerAction::MutateNode {
                            node_id,
                            mutation: ComposerNodeMutation::ChoiceOptionOrder {
                                from_index: idx,
                                to_index: idx - 1,
                            },
                        });
                    }
                    if ui.small_button("Down").clicked() && idx + 1 < options.len() {
                        action = Some(VisualComposerAction::MutateNode {
                            node_id,
                            mutation: ComposerNodeMutation::ChoiceOptionOrder {
                                from_index: idx,
                                to_index: idx + 1,
                            },
                        });
                    }
                });
            }
        });
    action
}

fn dialogue_action(node_id: u32, speaker: String, text: String) -> VisualComposerAction {
    VisualComposerAction::MutateNode {
        node_id,
        mutation: ComposerNodeMutation::DialogueText { speaker, text },
    }
}
