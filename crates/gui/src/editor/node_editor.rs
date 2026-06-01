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

use super::node_graph::{GraphLayoutOrientation, NodeGraph};
use super::node_rendering::{self, draw_story_connection_projected as draw_conn};
use super::node_types::{
    node_visual_height, node_visual_width, ContextMenu, StoryNode, StoryNodeVisualExt,
    CHOICE_HEADER_HEIGHT, CHOICE_OPTION_CELL_WIDTH, CHOICE_OPTION_ROW_HEIGHT, NODE_WIDTH,
};
use super::undo::UndoStack;

// =============================================================================
// NodeEditorPanel - UI Widget
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasDragMode {
    None,
    Pan,
    MarqueeSelect,
}

#[path = "node_editor/toolbar.rs"]
mod toolbar;
pub fn canvas_drag_mode(
    primary_drag: bool,
    middle_drag: bool,
    secondary_drag: bool,
    modifiers: egui::Modifiers,
    space_down: bool,
) -> CanvasDragMode {
    if middle_drag || secondary_drag || (primary_drag && (modifiers.ctrl || space_down)) {
        CanvasDragMode::Pan
    } else if primary_drag {
        CanvasDragMode::MarqueeSelect
    } else {
        CanvasDragMode::None
    }
}

pub fn logic_graph_heading_label(available_width: f32) -> &'static str {
    if available_width < 220.0 {
        "Graph"
    } else {
        "Logic Graph"
    }
}

pub fn node_editor_heading_label(available_width: f32) -> &'static str {
    if available_width < 220.0 {
        "Nodes"
    } else {
        "Node Editor"
    }
}

pub fn node_toolbar_is_compact(available_width: f32) -> bool {
    available_width < 440.0
}

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
        ui.heading(node_editor_heading_label(ui.available_width()));
        ui.separator();

        let layout_requested = self.render_toolbar(ui);
        ui.separator();

        let available_size = ui.available_size();
        let (response, painter) =
            ui.allocate_painter(available_size, egui::Sense::click_and_drag());
        let rect = response.rect;
        let painter = painter.with_clip_rect(rect);

        if layout_requested {
            self.graph.auto_layout_hierarchical();
            self.graph.zoom_to_fit_viewport(rect.size());
        }

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
        let primary_drag = response.dragged_by(egui::PointerButton::Primary);
        let middle_drag = response.dragged_by(egui::PointerButton::Middle);
        let secondary_drag = response.dragged_by(egui::PointerButton::Secondary);
        let (modifiers, space_down) = ui.input(|i| (i.modifiers, i.key_down(egui::Key::Space)));
        let drag_mode = canvas_drag_mode(
            primary_drag,
            middle_drag,
            secondary_drag,
            modifiers,
            space_down,
        );

        if drag_mode == CanvasDragMode::Pan {
            let delta = ui.input(|i| i.pointer.delta()) / self.graph.zoom();
            if delta.length_sq() > 0.0 {
                self.graph.pan_by(delta);
                self.graph.marquee_start = None;
                self.graph.marquee_current = None;
                self.graph.context_menu = None;
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

    pub fn apply_undo_shortcut(&mut self) -> bool {
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

    pub fn apply_redo_shortcut(&mut self) -> bool {
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
        let nodes = self.graph.visible_nodes().collect::<Vec<_>>();
        for conn in self.graph.visible_connections() {
            let from_pos = nodes
                .iter()
                .find(|(id, _, _)| *id == conn.from)
                .map(|(_, node, p)| (*p, node.clone()));
            let to_pos = nodes
                .iter()
                .find(|(id, _, _)| *id == conn.to)
                .map(|(_, node, p)| (*p, node.clone()));

            if let (Some((from_base, from_node)), Some((to_base, to_node))) = (from_pos, to_pos) {
                let zoom = self.graph.zoom();
                let to_rect = node_graph_rect(to_base, &to_node);
                let from_graph = node_output_port_pos_towards(
                    from_base,
                    &from_node,
                    conn.from_port,
                    to_rect.center(),
                );
                let to_graph = connection_target_point(from_graph, to_base, &to_node);

                if !node_rendering::connection_intersects_viewport_projected(
                    from_graph,
                    to_graph,
                    rect,
                    zoom,
                    |point| self.graph_to_screen(rect, point),
                ) {
                    continue;
                }

                let port = conn.from_port;
                draw_conn(
                    painter,
                    from_graph,
                    to_graph,
                    &from_node,
                    port,
                    zoom,
                    |point| self.graph_to_screen(rect, point),
                );
            }
        }
    }

    fn calculate_port_pos(
        &self,
        node_pos: egui::Pos2,
        node: &StoryNode,
        port: usize,
    ) -> egui::Pos2 {
        match node {
            StoryNode::Choice { .. } => match self.graph.layout_orientation {
                GraphLayoutOrientation::Horizontal => {
                    node_output_port_pos_on_side(node_pos, node, port, ConnectionSide::Right)
                }
                GraphLayoutOrientation::Vertical => {
                    node_output_port_pos_on_side(node_pos, node, port, ConnectionSide::Bottom)
                }
            },
            StoryNode::JumpIf { .. } => {
                let y = if port == 0 {
                    node_visual_height(node) * 0.68
                } else {
                    node_visual_height(node) * 0.92
                };
                node_pos + egui::vec2(NODE_WIDTH - 8.0, y)
            }
            _ => {
                // Standard single output (Bottom Center)
                node_pos + egui::vec2(NODE_WIDTH / 2.0, node_visual_height(node))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionSide {
    Top,
    Right,
    Bottom,
    Left,
}

pub fn node_output_port_pos_towards(
    node_pos: egui::Pos2,
    node: &StoryNode,
    port: usize,
    target_center: egui::Pos2,
) -> egui::Pos2 {
    let rect = node_graph_rect(node_pos, node);
    let delta = target_center - rect.center();
    let side = if delta.x.abs() >= delta.y.abs() {
        if delta.x >= 0.0 {
            ConnectionSide::Right
        } else {
            ConnectionSide::Left
        }
    } else if delta.y >= 0.0 {
        ConnectionSide::Bottom
    } else {
        ConnectionSide::Top
    };
    node_output_port_pos_on_side(node_pos, node, port, side)
}

pub fn node_output_port_pos_on_side(
    node_pos: egui::Pos2,
    node: &StoryNode,
    port: usize,
    side: ConnectionSide,
) -> egui::Pos2 {
    let width = node_visual_width(node);
    let height = node_visual_height(node);
    match node {
        StoryNode::Choice { options, .. } => {
            let route_count = (options.len() + 1).max(1);
            let route_index = port.min(route_count - 1);
            match side {
                ConnectionSide::Top | ConnectionSide::Bottom => {
                    let x = ((route_index as f32 + 0.5) * CHOICE_OPTION_CELL_WIDTH)
                        .clamp(8.0, width - 8.0);
                    let y = if side == ConnectionSide::Top {
                        0.0
                    } else {
                        height
                    };
                    node_pos + egui::vec2(x, y)
                }
                ConnectionSide::Right | ConnectionSide::Left => {
                    let min_y = CHOICE_HEADER_HEIGHT + 8.0;
                    let max_y = height - 8.0;
                    let y = distributed_route_lane(route_index, route_count, min_y, max_y);
                    let x = if side == ConnectionSide::Left {
                        0.0
                    } else {
                        width
                    };
                    node_pos + egui::vec2(x, y)
                }
            }
        }
        StoryNode::JumpIf { .. } => match side {
            ConnectionSide::Top | ConnectionSide::Bottom => {
                let x = if port == 0 {
                    width * 0.35
                } else {
                    width * 0.65
                };
                let y = if side == ConnectionSide::Top {
                    0.0
                } else {
                    height
                };
                node_pos + egui::vec2(x, y)
            }
            ConnectionSide::Right | ConnectionSide::Left => {
                let y = if port == 0 {
                    height * 0.68
                } else {
                    height * 0.92
                };
                let x = if side == ConnectionSide::Left {
                    0.0
                } else {
                    width
                };
                node_pos + egui::vec2(x, y)
            }
        },
        _ => match side {
            ConnectionSide::Top => node_pos + egui::vec2(width * 0.5, 0.0),
            ConnectionSide::Right => node_pos + egui::vec2(width, height * 0.5),
            ConnectionSide::Bottom => node_pos + egui::vec2(width * 0.5, height),
            ConnectionSide::Left => node_pos + egui::vec2(0.0, height * 0.5),
        },
    }
}

fn distributed_route_lane(index: usize, count: usize, min: f32, max: f32) -> f32 {
    if count <= 1 || (max - min).abs() <= f32::EPSILON {
        return (min + max) * 0.5;
    }
    let t = index as f32 / (count - 1) as f32;
    min + (max - min) * t
}

fn node_graph_rect(node_pos: egui::Pos2, node: &StoryNode) -> egui::Rect {
    egui::Rect::from_min_size(
        node_pos,
        egui::vec2(node_visual_width(node), node_visual_height(node)),
    )
}

fn connection_target_point(
    from_graph: egui::Pos2,
    to_base: egui::Pos2,
    to_node: &StoryNode,
) -> egui::Pos2 {
    let to_rect = node_graph_rect(to_base, to_node);
    if from_graph.y <= to_rect.top() {
        egui::pos2(to_rect.center().x, to_rect.top())
    } else if from_graph.y >= to_rect.bottom() {
        egui::pos2(to_rect.center().x, to_rect.bottom())
    } else if from_graph.x <= to_rect.left() {
        egui::pos2(to_rect.left(), to_rect.center().y)
    } else {
        egui::pos2(to_rect.right(), to_rect.center().y)
    }
}

fn graph_shortcuts_enabled(ui: &egui::Ui, response: &egui::Response, graph: &NodeGraph) -> bool {
    !ui.ctx().wants_keyboard_input()
        && graph_shortcut_scope_active(response.hovered(), graph.has_active_interaction())
}

pub fn graph_shortcut_scope_active(response_hovered: bool, interaction_active: bool) -> bool {
    response_hovered || interaction_active
}

mod palette;
pub use palette::extended_node_palette_items;
mod render;
mod render_helpers;
pub use render_helpers::node_status_hint;
