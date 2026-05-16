use super::*;

impl<'a> NodeEditorPanel<'a> {
    pub(super) fn get_node_height(&self, node: &StoryNode) -> f32 {
        crate::editor::node_types::node_visual_height(node)
    }

    pub(super) fn get_node_preview(&self, node: &StoryNode) -> String {
        match node {
            StoryNode::Dialogue { speaker, .. } => speaker.chars().take(15).collect(),
            StoryNode::Choice { prompt, .. } => prompt.chars().take(15).collect(),
            StoryNode::Scene {
                background, music, ..
            } => {
                let bg = background.as_deref().unwrap_or("<none>");
                let bgm = music.as_deref().unwrap_or("<none>");
                format!(
                    "bg:{} bgm:{}",
                    bg.chars().take(8).collect::<String>(),
                    bgm.chars().take(8).collect::<String>()
                )
            }
            StoryNode::Jump { target } => {
                format!("→ {}", target.chars().take(10).collect::<String>())
            }
            StoryNode::SetVariable { key, value } => format!("{} = {}", key, value),
            StoryNode::SetFlag { key, value } => format!("{} = {}", key, value),
            StoryNode::ScenePatch(patch) => {
                let bg = patch
                    .background
                    .as_ref()
                    .map(|value| value.chars().take(10).collect::<String>())
                    .unwrap_or_else(|| "-".to_string());
                let bgm = patch
                    .music
                    .as_ref()
                    .map(|value| value.chars().take(10).collect::<String>())
                    .unwrap_or_else(|| "-".to_string());
                format!(
                    "Scene? bg:{bg} bgm:{bgm} add:{} upd:{} rem:{}",
                    patch.add.len(),
                    patch.update.len(),
                    patch.remove.len()
                )
            }
            StoryNode::JumpIf { .. } => "Conditional".to_string(),
            StoryNode::Start => "Entry Point".to_string(),
            StoryNode::End => "Exit Point".to_string(),
            StoryNode::Generic(event) => match event {
                visual_novel_engine::EventRaw::ExtCall { command, .. } => {
                    format!("Ext: {}", command.chars().take(12).collect::<String>())
                }
                _ => {
                    let json = event.to_json_value();
                    let type_name = json
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("unknown");
                    format!("Generic: {}", type_name)
                }
            },
            StoryNode::AudioAction {
                channel, action, ..
            } => {
                format!("{} {}", action, channel)
            }
            StoryNode::Transition { kind, .. } => {
                format!("Transition: {}", kind)
            }
            StoryNode::CharacterPlacement { name, x, y, .. } => {
                format!("{}: ({}, {})", name, x, y)
            }
            StoryNode::SubgraphCall {
                fragment_id,
                entry_port,
                exit_port,
            } => format!(
                "{} {} -> {}",
                fragment_id,
                entry_port.as_deref().unwrap_or("<entry>"),
                exit_port.as_deref().unwrap_or("<exit>")
            ),
        }
    }

    pub(super) fn render_connecting_line(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
        if let Some((from_id, from_port)) = self.graph.connecting_from {
            if let Some((_, node, pos)) = self.graph.nodes().find(|(id, _, _)| *id == from_id) {
                if let Some(cursor) = response.hover_pos() {
                    let from =
                        self.graph_to_screen(rect, self.calculate_port_pos(pos, &node, from_port));
                    painter.line_segment(
                        [from, cursor],
                        egui::Stroke::new(2.0, egui::Color32::YELLOW),
                    );
                }
            }
        }
    }

    pub(super) fn render_status_bar(&self, painter: &egui::Painter, rect: egui::Rect) {
        let hint = if self.graph.connecting_from.is_some() {
            "Drag to node to connect - Esc cancels"
        } else if self.graph.marquee_start.is_some() {
            "Release to select nodes - Shift adds to selection"
        } else {
            "Drag from socket to connect - drag empty canvas to select - Ctrl+drag pans"
        };
        painter.text(
            rect.max - egui::vec2(10.0, 10.0),
            egui::Align2::RIGHT_BOTTOM,
            hint,
            egui::FontId::proportional(11.0),
            egui::Color32::from_rgb(120, 120, 130),
        );
    }

    pub(super) fn finish_marquee_selection(&mut self, ui: &egui::Ui) {
        let Some(start) = self.graph.marquee_start.take() else {
            self.graph.marquee_current = None;
            return;
        };
        let Some(current) = self.graph.marquee_current.take() else {
            return;
        };
        if start.distance(current) < 4.0 {
            return;
        }
        let additive = ui.input(|i| i.modifiers.shift);
        self.graph
            .select_nodes_in_rect(egui::Rect::from_two_pos(start, current), additive);
    }
}
