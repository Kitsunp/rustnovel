use super::*;

impl<'a> NodeEditorPanel<'a> {
    pub(super) fn render_nodes(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
        let mut clicked_node = None;
        let mut right_clicked_node = None;
        let mut double_clicked_node = None;
        let nodes: Vec<_> = self.graph.nodes().cloned().collect();

        // 1. Handle Drag Start (Nodes)
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                // Check ports first (priority over node move)
                for (id, node, n_pos) in nodes.iter().rev() {
                    if let StoryNode::Choice { options, .. } = node {
                        // Check option ports + one extra "new option" port.
                        for i in 0..=options.len() {
                            let port_pos = self.calculate_port_pos(*n_pos, node, i);
                            let screen_pos = self.graph_to_screen(rect, port_pos);
                            if screen_pos.distance(pos) < 10.0 * self.graph.zoom() {
                                self.graph.connecting_from = Some((*id, i));
                                return; // Consumed by port drag
                            }
                        }
                    } else if node.can_connect_from() {
                        // Standard port
                        let port_pos = self.calculate_port_pos(*n_pos, node, 0);
                        let screen_pos = self.graph_to_screen(rect, port_pos);
                        if screen_pos.distance(pos) < 10.0 * self.graph.zoom() {
                            self.graph.connecting_from = Some((*id, 0));
                            return;
                        }
                    }
                }

                // Then Node Drag
                for (id, _, n_pos) in nodes.iter().rev() {
                    let screen_pos = self.graph_to_screen(rect, *n_pos);
                    let height = self.get_node_height(self.graph.get_node(*id).unwrap());
                    let size = egui::vec2(NODE_WIDTH, height) * self.graph.zoom();
                    let node_rect = egui::Rect::from_min_size(screen_pos, size);
                    if node_rect.contains(pos) {
                        self.graph.dragging_node = Some(*id);
                        break;
                    }
                }
            }
        }

        // 2. Handle Dragging
        if response.dragged_by(egui::PointerButton::Primary) && self.graph.context_menu.is_none() {
            if let Some(id) = self.graph.dragging_node {
                let delta = ui.input(|i| i.pointer.delta()) / self.graph.zoom();
                if let Some(node_pos) = self.graph.get_node_pos_mut(id) {
                    if delta.length_sq() > 0.0 {
                        *node_pos += delta;
                        self.graph.mark_modified();
                    }
                }
            }
        }

        // 3. Handle Drag End
        if response.drag_stopped() {
            self.graph.dragging_node = None;
            if let Some((from, _)) = self.graph.connecting_from {
                let mut dropped_on_node = false;
                if let Some(pos) = response.interact_pointer_pos() {
                    for (to_id, _, to_pos) in nodes.iter().rev() {
                        let screen_pos = self.graph_to_screen(rect, *to_pos);
                        let height = self.get_node_height(self.graph.get_node(*to_id).unwrap());
                        let size = egui::vec2(NODE_WIDTH, height) * self.graph.zoom();
                        let node_rect = egui::Rect::from_min_size(screen_pos, size);

                        if node_rect.contains(pos) {
                            if from != *to_id {
                                // FINALIZE CONNECTION
                                let port = self.graph.connecting_from.unwrap().1;
                                self.graph.connect_port(from, port, *to_id);
                            }
                            dropped_on_node = true;
                            break;
                        }
                    }
                }
                if !dropped_on_node {
                    let port = self.graph.connecting_from.unwrap().1;
                    self.graph.disconnect_port(from, port);
                }
                self.graph.connecting_from = None;
            }
        }

        // Rendering Loop
        for (id, node, pos) in &nodes {
            let screen_pos = self.graph_to_screen(rect, *pos);
            let height = self.get_node_height(node);
            let size = egui::vec2(NODE_WIDTH, height) * self.graph.zoom();
            let node_rect = egui::Rect::from_min_size(screen_pos, size);

            if !rect.intersects(node_rect) {
                continue;
            }

            let is_selected = self.graph.selected == Some(*id);
            let is_connecting = self.graph.connecting_from.map(|(nid, _)| nid) == Some(*id);
            let is_dragging = self.graph.dragging_node == Some(*id);

            // Shape
            let bg_color = if is_selected || is_dragging {
                node.color().linear_multiply(1.3)
            } else if is_connecting {
                egui::Color32::YELLOW.linear_multiply(0.3)
            } else {
                node.color()
            };

            painter.rect_filled(node_rect, 6.0 * self.graph.zoom(), bg_color);
            let border_color = if is_selected {
                egui::Color32::from_rgb(100, 150, 255)
            } else {
                egui::Color32::from_rgb(80, 80, 90)
            };
            painter.rect_stroke(
                node_rect,
                2.0 * self.graph.zoom(),
                egui::Stroke::new(2.0, border_color),
            );

            // Content
            let font_size = 13.0 * self.graph.zoom();
            let text_pos = node_rect.min + egui::vec2(8.0, 8.0) * self.graph.zoom();
            painter.text(
                text_pos,
                egui::Align2::LEFT_TOP,
                format!("{} {}", node.icon(), node.type_name()),
                egui::FontId::proportional(font_size),
                egui::Color32::WHITE,
            );

            // Body / Options
            match node {
                StoryNode::Choice { options, .. } => {
                    let header_height = 40.0 * self.graph.zoom();
                    let option_h = 30.0 * self.graph.zoom();

                    for (i, opt) in options.iter().enumerate() {
                        let y_off = header_height + (i as f32 * option_h);
                        let opt_rect = egui::Rect::from_min_size(
                            node_rect.min + egui::vec2(0.0, y_off),
                            egui::vec2(node_rect.width(), option_h),
                        );

                        // Double-click on option to edit
                        if ui.input(|inp| {
                            inp.pointer
                                .button_double_clicked(egui::PointerButton::Primary)
                        }) {
                            if let Some(p) = response.interact_pointer_pos() {
                                if opt_rect.contains(p) {
                                    double_clicked_node = Some(*id);
                                }
                            }
                        }

                        painter.line_segment(
                            [opt_rect.left_top(), opt_rect.right_top()],
                            egui::Stroke::new(1.0, egui::Color32::BLACK),
                        );

                        painter.text(
                            opt_rect.left_center() + egui::vec2(5.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            crate::editor::graph_panel::truncate(opt, 15),
                            egui::FontId::proportional(11.0 * self.graph.zoom()),
                            egui::Color32::LIGHT_GRAY,
                        );

                        // Socket visual & Interaction
                        let socket_center =
                            self.graph_to_screen(rect, self.calculate_port_pos(*pos, node, i));
                        let hover_radius = 8.0 * self.graph.zoom();
                        let is_hovered = response
                            .hover_pos()
                            .is_some_and(|p| p.distance(socket_center) < hover_radius);

                        let mut color = egui::Color32::WHITE;
                        let mut radius = 4.0 * self.graph.zoom();

                        if is_hovered {
                            color = egui::Color32::YELLOW;
                            radius = 6.0 * self.graph.zoom();
                            // Tooltip
                            painter.text(
                                socket_center + egui::vec2(10.0, -10.0),
                                egui::Align2::LEFT_BOTTOM,
                                format!("Connect '{}'", opt),
                                egui::FontId::proportional(12.0),
                                egui::Color32::YELLOW,
                            );
                        }

                        painter.circle_filled(socket_center, radius, color);
                    }

                    // "New option" row/socket for fast branching.
                    let add_idx = options.len();
                    let add_y_off = header_height + (add_idx as f32 * option_h);
                    let add_rect = egui::Rect::from_min_size(
                        node_rect.min + egui::vec2(0.0, add_y_off),
                        egui::vec2(node_rect.width(), option_h),
                    );
                    painter.line_segment(
                        [add_rect.left_top(), add_rect.right_top()],
                        egui::Stroke::new(1.0, egui::Color32::BLACK),
                    );
                    painter.text(
                        add_rect.left_center() + egui::vec2(5.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        "+ New Option",
                        egui::FontId::proportional(11.0 * self.graph.zoom()),
                        egui::Color32::from_rgb(180, 220, 180),
                    );
                    let add_socket =
                        self.graph_to_screen(rect, self.calculate_port_pos(*pos, node, add_idx));
                    let add_hovered = response
                        .hover_pos()
                        .is_some_and(|p| p.distance(add_socket) < 8.0 * self.graph.zoom());
                    painter.circle_filled(
                        add_socket,
                        if add_hovered {
                            6.0 * self.graph.zoom()
                        } else {
                            4.0 * self.graph.zoom()
                        },
                        if add_hovered {
                            egui::Color32::YELLOW
                        } else {
                            egui::Color32::LIGHT_GREEN
                        },
                    );
                }
                _ => {
                    painter.text(
                        node_rect.min + egui::vec2(8.0, 28.0) * self.graph.zoom(),
                        egui::Align2::LEFT_TOP,
                        self.get_node_preview(node),
                        egui::FontId::proportional(11.0 * self.graph.zoom()),
                        egui::Color32::from_gray(200),
                    );

                    if node.can_connect_from() {
                        let socket_center =
                            self.graph_to_screen(rect, self.calculate_port_pos(*pos, node, 0));
                        let hover_radius = 8.0 * self.graph.zoom();
                        let is_hovered = response
                            .hover_pos()
                            .is_some_and(|p| p.distance(socket_center) < hover_radius);

                        let mut color = egui::Color32::WHITE;
                        let mut radius = 4.0 * self.graph.zoom();

                        if is_hovered {
                            color = egui::Color32::YELLOW;
                            radius = 6.0 * self.graph.zoom();
                            painter.text(
                                socket_center + egui::vec2(10.0, -10.0),
                                egui::Align2::LEFT_BOTTOM,
                                "Standard Output",
                                egui::FontId::proportional(12.0),
                                egui::Color32::YELLOW,
                            );
                        }

                        painter.circle_filled(socket_center, radius, color);
                    }
                }
            }

            if response.clicked() && !is_dragging && self.graph.connecting_from.is_none() {
                if let Some(p) = response.interact_pointer_pos() {
                    if node_rect.contains(p) {
                        clicked_node = Some(*id);
                    }
                }
            }
            if response.secondary_clicked() {
                if let Some(p) = response.interact_pointer_pos() {
                    if node_rect.contains(p) {
                        right_clicked_node = Some((*id, p));
                    }
                }
            }
            if ui.input(|i| {
                i.pointer
                    .button_double_clicked(egui::PointerButton::Primary)
            }) {
                if let Some(p) = response.interact_pointer_pos() {
                    if node_rect.contains(p) {
                        double_clicked_node = Some(*id);
                    }
                }
            }
        }

        if let Some(id) = clicked_node {
            self.graph.selected = Some(id);
        }
        if let Some((id, pos)) = right_clicked_node {
            self.graph.context_menu = Some(ContextMenu {
                node_id: id,
                position: pos,
            });
        }
        if let Some(id) = double_clicked_node {
            self.graph.editing = Some(id);
        }
    }

    fn get_node_height(&self, node: &StoryNode) -> f32 {
        crate::editor::node_types::node_visual_height(node)
    }

    fn get_node_preview(&self, node: &StoryNode) -> String {
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
                        self.graph_to_screen(rect, self.calculate_port_pos(*pos, node, from_port));
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
            "Drag to node to connect - drag to empty space to disconnect - Esc cancels"
        } else {
            "Drag from socket to connect - Double-click to edit"
        };
        painter.text(
            rect.max - egui::vec2(10.0, 10.0),
            egui::Align2::RIGHT_BOTTOM,
            hint,
            egui::FontId::proportional(11.0),
            egui::Color32::from_rgb(120, 120, 130),
        );
    }
}
