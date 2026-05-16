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
use super::node_types::{
    node_visual_height, ContextMenu, StoryNode, StoryNodeVisualExt, NODE_WIDTH,
};
use super::undo::UndoStack;
use visual_novel_engine::{CondRaw, EventRaw, ScenePatchRaw};

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
                    self.graph.set_single_selection(Some(id));
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
                    self.graph.set_single_selection(Some(id));
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
                    self.graph.set_single_selection(Some(id));
                    ui.close_menu();
                }
                if ui.button("↪ Jump").clicked() {
                    let id = self.graph.add_node(
                        StoryNode::Jump {
                            target: "label".to_string(),
                        },
                        pos,
                    );
                    self.graph.set_single_selection(Some(id));
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("▶ Start").clicked() {
                    let id = self
                        .graph
                        .add_node(StoryNode::Start, egui::pos2(50.0, 30.0));
                    self.graph.set_single_selection(Some(id));
                    ui.close_menu();
                }
                if ui.button("⏹ End").clicked() {
                    let id = self
                        .graph
                        .add_node(StoryNode::End, egui::pos2(200.0, 300.0));
                    self.graph.set_single_selection(Some(id));
                    ui.close_menu();
                }
            });

            ui.menu_button("More Nodes", |ui| {
                let pos = egui::pos2(120.0, 120.0) - self.graph.pan().to_pos2().to_vec2();
                for (label, node) in extended_node_palette_items() {
                    if ui.button(label).clicked() {
                        let id = self.graph.add_node(node, pos);
                        self.graph.set_single_selection(Some(id));
                        ui.close_menu();
                    }
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
                self.apply_undo_shortcut();
            }
            if ui
                .add_enabled(self.undo_stack.can_redo(), egui::Button::new("↪"))
                .clicked()
            {
                self.apply_redo_shortcut();
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
        let is_panning = response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged() && ui.input(|i| i.modifiers.ctrl));

        if is_panning {
            let delta = ui.input(|i| i.pointer.delta()) / self.graph.zoom();
            if delta.length_sq() > 0.0 {
                self.graph.pan_by(delta);
            }
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
            self.graph.clear_transient_interaction();
        }

        // Click outside to close menu
        if response.clicked() && self.graph.context_menu.is_some() {
            self.graph.context_menu = None;
        }

        if !graph_shortcuts_enabled(ui, response, self.graph) {
            return;
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

        // === Undo/Redo Keyboard Shortcuts ===
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)) {
            self.apply_undo_shortcut();
            return;
        }
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Y)) {
            self.apply_redo_shortcut();
            return;
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
            self.graph.zoom_to_fit_viewport(response.rect.size());
        }

        // === Node Action Shortcuts ===
        if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            let ids = self.graph.selected_node_ids();
            if !ids.is_empty() {
                for id in ids {
                    self.graph.remove_node(id);
                }
                self.graph.selected = None;
                self.graph.selected_nodes.clear();
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::E)) {
            if let Some(id) = self.graph.selected {
                self.graph.editing = Some(id);
            }
        }
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::D)) {
            self.graph.duplicate_selected_nodes();
        }
    }

    fn apply_undo_shortcut(&mut self) -> bool {
        if let Some(previous) = self.undo_stack.undo(self.graph.clone()) {
            *self.graph = previous;
            self.graph.queue_operation_hint(
                "undo",
                "Undo graph editor mutation",
                Some("graph".to_string()),
                false,
            );
            self.graph.mark_modified();
            true
        } else {
            false
        }
    }

    fn apply_redo_shortcut(&mut self) -> bool {
        if let Some(next) = self.undo_stack.redo(self.graph.clone()) {
            *self.graph = next;
            self.graph.queue_operation_hint(
                "redo",
                "Redo graph editor mutation",
                Some("graph".to_string()),
                false,
            );
            self.graph.mark_modified();
            true
        } else {
            false
        }
    }

    fn render_connections(&self, painter: &egui::Painter, rect: egui::Rect) {
        for conn in self.graph.connections() {
            let from_pos = self
                .graph
                .nodes()
                .find(|(id, _, _)| *id == conn.from)
                .map(|(_, node, p)| (p, node));
            let to_pos = self
                .graph
                .nodes()
                .find(|(id, _, _)| *id == conn.to)
                .map(|(_, node, p)| (p, node));

            if let (Some((from_base, from_node)), Some((to_base, to_node))) = (from_pos, to_pos) {
                // Determine source port position
                let from_screen = self.graph_to_screen(
                    rect,
                    self.calculate_port_pos(from_base, &from_node, conn.from_port),
                );

                let to_node_top_left = self.graph_to_screen(rect, to_base);
                let to_node_size =
                    egui::vec2(NODE_WIDTH, node_visual_height(&to_node)) * self.graph.zoom();
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
            StoryNode::JumpIf { .. } => {
                let y = if port == 0 {
                    node_visual_height(node) * 0.68
                } else {
                    node_visual_height(node) * 0.92
                };
                node_pos + egui::vec2(NODE_WIDTH / 2.0, y)
            }
            _ => {
                // Standard single output (Bottom Center)
                node_pos + egui::vec2(NODE_WIDTH / 2.0, node_visual_height(node))
            }
        }
    }
}

pub(crate) fn extended_node_palette_items() -> Vec<(&'static str, StoryNode)> {
    vec![
        (
            "Scene Patch",
            StoryNode::ScenePatch(ScenePatchRaw::default()),
        ),
        (
            "Branch If",
            StoryNode::JumpIf {
                target: "label".to_string(),
                cond: CondRaw::Flag {
                    key: "flag".to_string(),
                    is_set: true,
                },
            },
        ),
        (
            "Set Variable",
            StoryNode::SetVariable {
                key: "variable".to_string(),
                value: 0,
            },
        ),
        (
            "Set Flag",
            StoryNode::SetFlag {
                key: "flag".to_string(),
                value: true,
            },
        ),
        (
            "Audio",
            StoryNode::AudioAction {
                channel: "bgm".to_string(),
                action: "play".to_string(),
                asset: None,
                volume: Some(1.0),
                fade_duration_ms: Some(0),
                loop_playback: Some(true),
            },
        ),
        (
            "Transition",
            StoryNode::Transition {
                kind: "fade_black".to_string(),
                duration_ms: 500,
                color: Some("#000000".to_string()),
            },
        ),
        (
            "Character Placement",
            StoryNode::CharacterPlacement {
                name: "Character".to_string(),
                x: 0,
                y: 0,
                scale: Some(1.0),
            },
        ),
        (
            "ExtCall",
            StoryNode::Generic(EventRaw::ExtCall {
                command: "command".to_string(),
                args: Vec::new(),
            }),
        ),
        (
            "Subgraph Call",
            StoryNode::SubgraphCall {
                fragment_id: String::new(),
                entry_port: None,
                exit_port: None,
            },
        ),
    ]
}

fn graph_shortcuts_enabled(ui: &egui::Ui, response: &egui::Response, graph: &NodeGraph) -> bool {
    !ui.ctx().wants_keyboard_input()
        && graph_shortcut_scope_active(response.hovered(), graph.has_active_interaction())
}

fn graph_shortcut_scope_active(response_hovered: bool, interaction_active: bool) -> bool {
    response_hovered || interaction_active
}

mod render;
mod render_helpers;
#[cfg(test)]
#[path = "tests/node_editor_tests.rs"]
mod tests;
