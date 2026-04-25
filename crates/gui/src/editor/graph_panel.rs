//! Graph panel for the editor workbench.
//!
//! Displays the story flow as a visual graph with nodes and edges.

use crate::editor::{NodeGraph, StoryNode};
use eframe::egui;

/// Graph panel widget.
pub struct GraphPanel<'a> {
    graph: &'a mut NodeGraph,
}

impl<'a> GraphPanel<'a> {
    pub fn new(graph: &'a mut NodeGraph) -> Self {
        Self { graph }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Story Graph");
        ui.separator();

        let node_count = self.graph.len();
        let connection_count = self.graph.connection_count();

        ui.horizontal(|ui| {
            ui.label(format!("Nodes: {}", node_count));
            ui.separator();
            ui.label(format!("Edges: {}", connection_count));
        });

        ui.separator();

        let nodes: Vec<(u32, String, egui::Color32)> = self
            .graph
            .nodes()
            .map(|(id, node, _)| {
                let info = match node {
                    StoryNode::Dialogue { speaker, text } => {
                        format!("Dialogue {}: {}", speaker, truncate(text, 20))
                    }
                    StoryNode::Choice { prompt, .. } => format!("Choice {}", truncate(prompt, 20)),
                    StoryNode::Scene {
                        background, music, ..
                    } => {
                        let bg = background.as_deref().unwrap_or("<none>");
                        let bgm = music.as_deref().unwrap_or("<none>");
                        format!("Scene bg:{} | bgm:{}", truncate(bg, 12), truncate(bgm, 12))
                    }
                    StoryNode::Jump { target } => format!("Jump to {}", target),
                    StoryNode::SetVariable { key, value } => format!("Set {} = {}", key, value),
                    StoryNode::ScenePatch(_) => "Scene Patch".to_string(),
                    StoryNode::JumpIf { target, .. } => format!("If -> {}", target),
                    StoryNode::Start => "Start".to_string(),
                    StoryNode::End => "End".to_string(),
                    StoryNode::Generic(event) => {
                        let json = event.to_json_value();
                        let type_name = json
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("unknown");
                        format!("Generic ({})", type_name)
                    }
                    StoryNode::AudioAction {
                        channel, action, ..
                    } => {
                        format!("Audio: {} {}", action, channel)
                    }
                    StoryNode::Transition { kind, .. } => {
                        format!("Transition: {}", kind)
                    }
                    StoryNode::CharacterPlacement {
                        name,
                        x,
                        y,
                        scale: _,
                    } => {
                        format!("Placement: {} ({}, {})", name, x, y)
                    }
                };
                (*id, info, node.color())
            })
            .collect();

        let mut new_selection = None;
        let current_selection = self.graph.selected;

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (id, text, color) in nodes {
                let is_selected = current_selection == Some(id);
                let response = ui.selectable_label(
                    is_selected,
                    egui::RichText::new(format!("{}: {}", id, text)).color(color),
                );

                if response.clicked() {
                    new_selection = Some(id);
                }
            }
        });

        if let Some(id) = new_selection {
            self.graph.selected = Some(id);
        }
    }
}

/// Truncates a string to a certain length with ellipsis.
pub fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut result: String = s.chars().take(max_chars).collect();
        result.push_str("...");
        result
    }
}
