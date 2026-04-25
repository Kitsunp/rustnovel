//! Node editor panel for the visual editor workbench.
//!
//! This module provides the UI widget for the visual graph editor.
//! The data structures (`NodeGraph`, `StoryNode`) are in separate modules.
//! Rendering utilities are in `node_rendering`.
//!
//! # Design Principles
//! - **Modularity**: UI separated from data (Criterio J ≤500 lines)
//! - **Single Responsibility**: Only rendering and input handling

use eframe::egui;

use super::node_graph::NodeGraph;
use super::node_rendering;
use super::node_types::{node_visual_height, ContextMenu, StoryNode, NODE_WIDTH};
use super::undo::UndoStack;

// =============================================================================
// NodeEditorPanel - UI Widget
// =============================================================================

/// Node editor panel widget with pan/zoom and context menu.
pub struct NodeEditorPanel<'a> {
    graph: &'a mut NodeGraph,
    undo_stack: &'a mut UndoStack,
}

impl<'a> NodeEditorPanel<'a> {
    pub fn new(graph: &'a mut NodeGraph, undo_stack: &'a mut UndoStack) -> Self {
        Self { graph, undo_stack }
    }

    #[inline]
    fn graph_to_screen(&self, rect: egui::Rect, pos: egui::Pos2) -> egui::Pos2 {
        rect.min + (pos.to_vec2() + self.graph.pan()) * self.graph.zoom()
    }

    #[inline]
    /// Transforms a screen-space position to graph-space.
    #[allow(dead_code)]
    fn screen_to_graph(&self, rect: egui::Rect, pos: egui::Pos2) -> egui::Pos2 {
        ((pos - rect.min) / self.graph.zoom() - self.graph.pan()).to_pos2()
    }

    /// Main UI entry point.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("📊 Node Editor");
        ui.separator();

        self.render_toolbar(ui);
        ui.separator();

        let available_size = ui.available_size();
        let (response, painter) =
            ui.allocate_painter(available_size, egui::Sense::click_and_drag());
        let rect = response.rect;

        painter.rect_filled(rect, 5.0, egui::Color32::from_rgb(25, 25, 35));

        self.render_grid(&painter, rect);
        self.handle_input(ui, &response);
        self.render_connections(&painter, rect);
        self.render_nodes(ui, &painter, rect, &response);
        self.render_connecting_line(&painter, rect, &response);
        node_rendering::render_context_menu(self.graph, ui);
        node_rendering::render_inline_editor(self.graph, ui);
        self.render_status_bar(&painter, rect);
    }

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.menu_button("➕ Add Node", |ui| {
                let pos = egui::pos2(100.0, 100.0) - self.graph.pan().to_pos2().to_vec2();
                if ui.button("💬 Dialogue").clicked() {
                    let id = self.graph.add_node(StoryNode::default(), pos);
                    self.graph.selected = Some(id);
                    ui.close_menu();
                }
                if ui.button("🔀 Choice").clicked() {
                    let id = self.graph.add_node(
                        StoryNode::Choice {
                            prompt: "Choose:".to_string(),
                            options: vec!["A".to_string(), "B".to_string()],
                        },
                        pos,
                    );
                    self.graph.selected = Some(id);
                    ui.close_menu();
                }
                if ui.button("🎬 Scene").clicked() {
                    let id = self.graph.add_node(
                        StoryNode::Scene {
                            profile: None,
                            background: Some("bg.png".to_string()),
                            music: None,
                            characters: Vec::new(),
                        },
                        pos,
                    );
                    self.graph.selected = Some(id);
                    ui.close_menu();
                }
                if ui.button("↪ Jump").clicked() {
                    let id = self.graph.add_node(
                        StoryNode::Jump {
                            target: "label".to_string(),
                        },
                        pos,
                    );
                    self.graph.selected = Some(id);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("▶ Start").clicked() {
                    let id = self
                        .graph
                        .add_node(StoryNode::Start, egui::pos2(50.0, 30.0));
                    self.graph.selected = Some(id);
                    ui.close_menu();
                }
                if ui.button("⏹ End").clicked() {
                    let id = self
                        .graph
                        .add_node(StoryNode::End, egui::pos2(200.0, 300.0));
                    self.graph.selected = Some(id);
                    ui.close_menu();
                }
            });

            ui.separator();
            if ui.button("🔍 Reset View").clicked() {
                self.graph.reset_view();
            }
            if ui.button("Auto Layout").clicked() {
                self.graph.auto_layout_hierarchical();
                self.graph.zoom_to_fit();
            }
            ui.label(format!("Zoom: {:.0}%", self.graph.zoom() * 100.0));

            ui.separator();

            // Undo/Redo
            if ui
                .add_enabled(self.undo_stack.can_undo(), egui::Button::new("↩"))
                .clicked()
            {
                if let Some(previous) = self.undo_stack.undo(self.graph.clone()) {
                    *self.graph = previous;
                }
            }
            if ui
                .add_enabled(self.undo_stack.can_redo(), egui::Button::new("↪"))
                .clicked()
            {
                if let Some(next) = self.undo_stack.redo(self.graph.clone()) {
                    *self.graph = next;
                }
            }

            ui.separator();
            ui.label(format!(
                "Nodes: {} | Connections: {}",
                self.graph.len(),
                self.graph.connection_count()
            ));
            if self.graph.is_modified() {
                ui.label("⚠ Modified");
            }
        });
    }

    fn render_grid(&self, painter: &egui::Painter, rect: egui::Rect) {
        let grid_spacing = 50.0 * self.graph.zoom();
        let grid_color_minor = egui::Color32::from_rgba_unmultiplied(80, 80, 100, 32);
        let grid_color_major = egui::Color32::from_rgba_unmultiplied(120, 120, 150, 70);
        let normalize_offset = |value: f32| ((value % grid_spacing) + grid_spacing) % grid_spacing;
        let offset_x = normalize_offset(self.graph.pan().x * self.graph.zoom());
        let offset_y = normalize_offset(self.graph.pan().y * self.graph.zoom());

        let mut x = rect.min.x + offset_x;
        let mut col_idx = 0usize;
        while x < rect.max.x {
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                egui::Stroke::new(
                    1.0,
                    if col_idx.is_multiple_of(5) {
                        grid_color_major
                    } else {
                        grid_color_minor
                    },
                ),
            );
            x += grid_spacing;
            col_idx += 1;
        }
        let mut y = rect.min.y + offset_y;
        let mut row_idx = 0usize;
        while y < rect.max.y {
            painter.line_segment(
                [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                egui::Stroke::new(
                    1.0,
                    if row_idx.is_multiple_of(5) {
                        grid_color_major
                    } else {
                        grid_color_minor
                    },
                ),
            );
            y += grid_spacing;
            row_idx += 1;
        }
    }

    fn handle_input(&mut self, ui: &egui::Ui, response: &egui::Response) {
        let mut is_panning = response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged() && ui.input(|i| i.modifiers.ctrl));

        if response.dragged_by(egui::PointerButton::Primary)
            && !is_panning
            && self.graph.dragging_node.is_none()
        {
            // Check if we started dragging on a node
            if let Some(_pos) = response.interact_pointer_pos() {
                // We need the START position of the drag, not current.
                if let Some(start_pos) = ui.input(|i| i.pointer.press_origin()) {
                    let mut started_on_node = false;
                    for (_, node, n_pos) in self.graph.nodes() {
                        let screen_pos = self.graph_to_screen(response.rect, *n_pos);
                        let size =
                            egui::vec2(NODE_WIDTH, node_visual_height(node)) * self.graph.zoom();
                        let rect = egui::Rect::from_min_size(screen_pos, size);
                        if rect.contains(start_pos) {
                            started_on_node = true;
                            break;
                        }
                    }

                    if !started_on_node {
                        is_panning = true;
                    }
                }
            }
        }

        if is_panning {
            let delta = ui.input(|i| i.pointer.delta()) / self.graph.zoom();
            if delta.length_sq() > 0.0 {
                self.graph.pan_by(delta);
            }
        }

        // Pan with Arrow Keys
        let pan_speed = 5.0; // Pixels per frame approx
        if ui.input(|i| i.key_down(egui::Key::ArrowUp)) {
            self.graph
                .pan_by(egui::vec2(0.0, pan_speed) / self.graph.zoom());
        }
        if ui.input(|i| i.key_down(egui::Key::ArrowDown)) {
            self.graph
                .pan_by(egui::vec2(0.0, -pan_speed) / self.graph.zoom());
        }
        if ui.input(|i| i.key_down(egui::Key::ArrowLeft)) {
            self.graph
                .pan_by(egui::vec2(pan_speed, 0.0) / self.graph.zoom());
        }
        if ui.input(|i| i.key_down(egui::Key::ArrowRight)) {
            self.graph
                .pan_by(egui::vec2(-pan_speed, 0.0) / self.graph.zoom());
        }

        // Zoom with scroll wheel only when pointer is over the graph canvas.
        // This avoids stealing wheel input from dialogs/panels (e.g. save diff).
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_delta.abs() > 0.0 {
                self.graph.zoom_by(scroll_delta * 0.002);
            }
        }

        // Escape to cancel modes
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.graph.connecting_from = None;
            self.graph.context_menu = None;
        }

        // Click outside to close menu
        if response.clicked() && self.graph.context_menu.is_some() {
            self.graph.context_menu = None;
        }

        // === Zoom Keyboard Shortcuts ===
        if ui.input(|i| i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)) {
            self.graph.zoom_by(0.1);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Minus)) {
            self.graph.zoom_by(-0.1);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Num0)) {
            self.graph.reset_view();
        }
        if ui.input(|i| i.key_pressed(egui::Key::H)) {
            self.graph.zoom_to_fit();
        }

        // === Node Action Shortcuts ===
        if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            if let Some(id) = self.graph.selected {
                self.graph.remove_node(id);
                self.graph.selected = None;
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::E)) {
            if let Some(id) = self.graph.selected {
                self.graph.editing = Some(id);
            }
        }
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::D)) {
            if let Some(id) = self.graph.selected {
                self.graph.duplicate_node(id);
            }
        }
    }

    fn render_connections(&self, painter: &egui::Painter, rect: egui::Rect) {
        for conn in self.graph.connections() {
            let from_pos = self
                .graph
                .nodes()
                .find(|(id, _, _)| *id == conn.from)
                .map(|(_, node, p)| (*p, node));
            let to_pos = self
                .graph
                .nodes()
                .find(|(id, _, _)| *id == conn.to)
                .map(|(_, node, p)| (*p, node));

            if let (Some((from_base, from_node)), Some((to_base, to_node))) = (from_pos, to_pos) {
                // Determine source port position
                let from_screen = self.graph_to_screen(
                    rect,
                    self.calculate_port_pos(from_base, from_node, conn.from_port),
                );

                let to_node_top_left = self.graph_to_screen(rect, to_base);
                let to_node_size =
                    egui::vec2(NODE_WIDTH, node_visual_height(to_node)) * self.graph.zoom();
                let to_rect = egui::Rect::from_min_size(to_node_top_left, to_node_size);
                let to_screen = if from_screen.y <= to_rect.top() {
                    egui::pos2(to_rect.center().x, to_rect.top())
                } else if from_screen.y >= to_rect.bottom() {
                    egui::pos2(to_rect.center().x, to_rect.bottom())
                } else if from_screen.x <= to_rect.left() {
                    egui::pos2(to_rect.left(), to_rect.center().y)
                } else {
                    egui::pos2(to_rect.right(), to_rect.center().y)
                };

                node_rendering::draw_bezier_connection(painter, from_screen, to_screen);
            }
        }
    }

    /// Calculates local graph position of an output port
    fn calculate_port_pos(
        &self,
        node_pos: egui::Pos2,
        node: &StoryNode,
        port: usize,
    ) -> egui::Pos2 {
        match node {
            StoryNode::Choice { .. } => {
                let header_height = 40.0;
                let option_height = 30.0;
                let option_offset =
                    header_height + (port as f32 * option_height) + (option_height / 2.0);

                node_pos + egui::vec2(NODE_WIDTH / 2.0, option_offset + 15.0)
            }
            _ => {
                // Standard single output (Bottom Center)
                node_pos + egui::vec2(NODE_WIDTH / 2.0, node_visual_height(node))
            }
        }
    }
}

mod render;
#[cfg(test)]
#[path = "tests/node_editor_tests.rs"]
mod tests;
