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
        let mut clicked_connection_target = None;
        let mut clicked_on_any_node = false;
        let mut right_clicked_canvas = None;
        let nodes: Vec<_> = self.graph.nodes().collect();

        if self.graph.connecting_sticky && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.graph.cancel_connection();
        }

        // 1. Handle Drag Start (Nodes)
        if response.drag_started_by(egui::PointerButton::Primary) && !ui.input(|i| i.modifiers.ctrl)
        {
            if let Some(pos) = response.interact_pointer_pos() {
                // Check ports first (priority over node move)
                for (id, node, n_pos) in nodes.iter().rev() {
                    if let StoryNode::Choice { options, .. } = node {
                        // Check option ports + one extra "new option" port.
                        for i in 0..=options.len() {
                            let port_pos = self.calculate_port_pos(*n_pos, node, i);
                            let screen_pos = self.graph_to_screen(rect, port_pos);
                            if screen_pos.distance(pos) < 10.0 * self.graph.zoom() {
                                self.graph.start_connection_drag(*id, i);
                                return; // Consumed by port drag
                            }
                        }
                    } else if let StoryNode::JumpIf { .. } = node {
                        for port in 0..=1 {
                            let port_pos = self.calculate_port_pos(*n_pos, node, port);
                            let screen_pos = self.graph_to_screen(rect, port_pos);
                            if screen_pos.distance(pos) < 10.0 * self.graph.zoom() {
                                self.graph.start_connection_drag(*id, port);
                                return;
                            }
                        }
                    } else if node.can_connect_from() {
                        // Standard port
                        let port_pos = self.calculate_port_pos(*n_pos, node, 0);
                        let screen_pos = self.graph_to_screen(rect, port_pos);
                        if screen_pos.distance(pos) < 10.0 * self.graph.zoom() {
                            self.graph.start_connection_drag(*id, 0);
                            return;
                        }
                    }
                }

                // Then Node Drag
                for (id, _, n_pos) in nodes.iter().rev() {
                    let screen_pos = self.graph_to_screen(rect, *n_pos);
                    let Some(node) = self.graph.get_node(*id) else {
                        continue;
                    };
                    let height = self.get_node_height(node);
                    let size = egui::vec2(NODE_WIDTH, height) * self.graph.zoom();
                    let node_rect = egui::Rect::from_min_size(screen_pos, size);
                    if node_rect.contains(pos) {
                        self.undo_stack.push(self.graph.clone());
                        self.graph.dragging_node = Some(*id);
                        break;
                    }
                }

                if self.graph.dragging_node.is_none() && !ui.input(|i| i.modifiers.ctrl) {
                    let graph_pos = self.screen_to_graph(rect, pos);
                    self.graph.marquee_start = Some(graph_pos);
                    self.graph.marquee_current = Some(graph_pos);
                }
            }
        }

        // 2. Handle Dragging
        if response.dragged_by(egui::PointerButton::Primary) && self.graph.context_menu.is_none() {
            if let Some(id) = self.graph.dragging_node {
                let delta = ui.input(|i| i.pointer.delta()) / self.graph.zoom();
                if delta.length_sq() > 0.0 {
                    self.graph.translate_selected_or_node_for_drag(id, delta);
                }
            } else if self.graph.marquee_start.is_some() {
                if let Some(pos) = response.interact_pointer_pos() {
                    self.graph.marquee_current = Some(self.screen_to_graph(rect, pos));
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
                        let Some(node) = self.graph.get_node(*to_id) else {
                            continue;
                        };
                        let height = self.get_node_height(node);
                        let size = egui::vec2(NODE_WIDTH, height) * self.graph.zoom();
                        let node_rect = egui::Rect::from_min_size(screen_pos, size);

                        if node_rect.contains(pos) {
                            let port = self
                                .graph
                                .connecting_from
                                .map(|(_, port)| port)
                                .unwrap_or(0);
                            self.graph.connect_or_branch(from, port, *to_id);
                            dropped_on_node = true;
                            break;
                        }
                    }
                }
                let _ = dropped_on_node;
                self.graph.cancel_connection();
            }
            self.finish_marquee_selection(ui);
        } else if !ui.input(|i| i.pointer.primary_down()) {
            if self.graph.marquee_start.is_some() {
                self.finish_marquee_selection(ui);
            }
            self.graph.dragging_node = None;
            if !self.graph.connecting_sticky {
                self.graph.cancel_connection();
            }
        }

        if let (Some(start), Some(current)) = (self.graph.marquee_start, self.graph.marquee_current)
        {
            let marquee_rect = egui::Rect::from_two_pos(
                self.graph_to_screen(rect, start),
                self.graph_to_screen(rect, current),
            );
            painter.rect_filled(
                marquee_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(90, 150, 255, 28),
            );
            painter.rect_stroke(
                marquee_rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 180, 255)),
            );
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

            let is_selected =
                self.graph.selected == Some(*id) || self.graph.selected_nodes.contains(id);
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
                StoryNode::JumpIf { .. } => {
                    painter.text(
                        node_rect.min + egui::vec2(8.0, 28.0) * self.graph.zoom(),
                        egui::Align2::LEFT_TOP,
                        self.get_node_preview(node),
                        egui::FontId::proportional(11.0 * self.graph.zoom()),
                        egui::Color32::from_gray(200),
                    );
                    for (port, label) in [(0, "True"), (1, "False")] {
                        let socket_center =
                            self.graph_to_screen(rect, self.calculate_port_pos(*pos, node, port));
                        let hover_radius = 8.0 * self.graph.zoom();
                        let is_hovered = response
                            .hover_pos()
                            .is_some_and(|p| p.distance(socket_center) < hover_radius);
                        let color = if is_hovered {
                            egui::Color32::YELLOW
                        } else if port == 0 {
                            egui::Color32::LIGHT_GREEN
                        } else {
                            egui::Color32::LIGHT_BLUE
                        };
                        painter.circle_filled(
                            socket_center,
                            if is_hovered {
                                6.0 * self.graph.zoom()
                            } else {
                                4.0 * self.graph.zoom()
                            },
                            color,
                        );
                        painter.text(
                            socket_center + egui::vec2(8.0, -8.0),
                            egui::Align2::LEFT_BOTTOM,
                            label,
                            egui::FontId::proportional(10.0 * self.graph.zoom()),
                            color,
                        );
                    }
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

            if response.clicked() && !is_dragging {
                if let Some(p) = response.interact_pointer_pos() {
                    if node_rect.contains(p) {
                        clicked_on_any_node = true;
                        if let Some((from, port)) = self.graph.connecting_from {
                            clicked_connection_target = Some((from, port, *id));
                        } else {
                            clicked_node = Some(*id);
                        }
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

        if let Some((from, port, to)) = clicked_connection_target {
            self.graph.connecting_from = Some((from, port));
            self.graph.finish_connection_to(to);
            return;
        }
        if self.graph.connecting_sticky && response.clicked() && !clicked_on_any_node {
            self.graph.cancel_connection();
        }

        if let Some(id) = clicked_node {
            if ui.input(|input| input.modifiers.ctrl) {
                self.graph.toggle_multi_selection(id);
            } else {
                self.graph.set_single_selection(Some(id));
            }
        }
        if let Some((id, pos)) = right_clicked_node {
            self.graph.context_menu = Some(ContextMenu::for_node(id, pos));
        } else if response.secondary_clicked() {
            if let Some(pos) = response
                .interact_pointer_pos()
                .filter(|pos| rect.contains(*pos))
            {
                right_clicked_canvas = Some((pos, self.screen_to_graph(rect, pos)));
            }
        }
        if let Some((screen_pos, graph_pos)) = right_clicked_canvas {
            self.graph.context_menu = Some(ContextMenu::for_canvas(screen_pos, graph_pos));
        }
        if let Some(id) = double_clicked_node {
            if let Some(StoryNode::SubgraphCall { fragment_id, .. }) = self.graph.get_node(id) {
                let fragment_id = fragment_id.clone();
                if !self.graph.enter_fragment(&fragment_id) {
                    self.graph.editing = Some(id);
                }
            } else {
                self.graph.editing = Some(id);
            }
        }
    }
}
