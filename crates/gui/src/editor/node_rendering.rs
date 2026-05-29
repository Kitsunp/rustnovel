//! Node rendering utilities for the visual editor.
//!
//! This module contains helper functions for rendering node components:
//! context menus, inline editors, bezier curves, and toast notifications.
//! Extracted from node_editor.rs to comply with Criterio J (<500 lines).

use eframe::egui;

use super::node_graph::NodeGraph;
use super::node_types::{StoryNode, ToastState};

#[path = "node_rendering_inline.rs"]
mod inline;
use inline::*;
#[path = "node_rendering_edges.rs"]
mod edges;
pub use edges::bezier_control_points;
pub use edges::draw_bezier_connection;

pub fn node_context_menu_size() -> egui::Vec2 {
    egui::vec2(230.0, 360.0)
}

pub fn canvas_context_menu_size() -> egui::Vec2 {
    egui::vec2(280.0, 440.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextMenuLayout {
    pub width: f32,
    pub max_height: f32,
    pub list_max_height: f32,
}

impl ContextMenuLayout {
    pub fn size(self) -> egui::Vec2 {
        egui::vec2(self.width, self.max_height)
    }

    pub fn item_width(self) -> f32 {
        (self.width - 12.0).max(48.0)
    }
}

pub fn context_menu_layout(bounds: egui::Rect, preferred_size: egui::Vec2) -> ContextMenuLayout {
    let usable_width = (bounds.width() - 16.0).max(96.0);
    let usable_height = (bounds.height() - 16.0).max(120.0);
    let width = preferred_size.x.clamp(180.0, 300.0).min(usable_width);
    let max_height = preferred_size.y.clamp(160.0, 480.0).min(usable_height);
    ContextMenuLayout {
        width,
        max_height,
        list_max_height: (max_height - 78.0).max(84.0),
    }
}

pub fn clamped_context_menu_position(
    requested: egui::Pos2,
    bounds: egui::Rect,
    expected_size: egui::Vec2,
) -> egui::Pos2 {
    let max_x = (bounds.right() - expected_size.x).max(bounds.left());
    let max_y = (bounds.bottom() - expected_size.y).max(bounds.top());
    egui::pos2(
        requested.x.clamp(bounds.left(), max_x),
        requested.y.clamp(bounds.top(), max_y),
    )
}

/// Renders a toast notification if one is active.
///
/// Call this at the end of the UI rendering to ensure toast appears on top.
pub fn render_toast(ui: &egui::Ui, toast: &mut Option<ToastState>) {
    let Some(t) = toast else {
        return;
    };

    // Decrement frame counter
    if t.frames_remaining > 0 {
        t.frames_remaining -= 1;
    }

    // Calculate alpha for fade out (last 30 frames)
    let alpha = if t.frames_remaining < 30 {
        (t.frames_remaining as f32 / 30.0 * 255.0) as u8
    } else {
        255
    };

    if t.frames_remaining == 0 {
        *toast = None;
        return;
    }

    // Render toast in bottom-right corner
    let screen_rect = ui.ctx().screen_rect();
    let toast_pos = egui::pos2(screen_rect.max.x - 20.0, screen_rect.max.y - 60.0);

    egui::Area::new(egui::Id::new("toast_notification"))
        .fixed_pos(toast_pos)
        .pivot(egui::Align2::RIGHT_BOTTOM)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            let bg_color = t.kind.color().linear_multiply(0.9);
            let bg_color = egui::Color32::from_rgba_unmultiplied(
                bg_color.r(),
                bg_color.g(),
                bg_color.b(),
                alpha,
            );

            egui::Frame::none()
                .fill(bg_color)
                .rounding(8.0)
                .inner_margin(egui::Margin::symmetric(16.0, 10.0))
                .show(ui, |ui| {
                    let text_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(t.kind.icon())
                                .size(16.0)
                                .color(text_color),
                        );
                        ui.label(egui::RichText::new(&t.message).color(text_color));
                    });
                });
        });

    // Request repaint to animate
    ui.ctx().request_repaint();
}

/// Renders the context menu for a node.
pub fn render_context_menu(graph: &mut NodeGraph, ui: &egui::Ui) {
    let Some(menu) = graph.context_menu.clone() else {
        return;
    };

    let Some(node_id) = menu.node_id else {
        render_canvas_context_menu(graph, ui, menu.position, menu.graph_position);
        return;
    };
    let node_snapshot = graph.get_node(node_id).cloned();
    let scene_profile = match &node_snapshot {
        Some(StoryNode::Scene { profile, .. }) => profile.clone(),
        _ => None,
    };

    let bounds = ui.ctx().available_rect();
    let layout = context_menu_layout(bounds, node_context_menu_size());
    let menu_position = clamped_context_menu_position(menu.position, bounds, layout.size());
    egui::Area::new(egui::Id::new("node_context_menu"))
        .fixed_pos(menu_position)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(layout.width);
                ui.set_max_width(layout.width);
                ui.label(egui::RichText::new("Node actions").strong());
                ui.separator();
                ui.label(egui::RichText::new("Insert").small());
                if context_menu_button(ui, layout, "Before").clicked() {
                    graph.insert_before(node_id, StoryNode::default());
                    graph.context_menu = None;
                }
                if context_menu_button(ui, layout, "After").clicked() {
                    graph.insert_after(node_id, StoryNode::default());
                    graph.context_menu = None;
                }

                ui.separator();
                ui.label(egui::RichText::new("Route").small());
                if context_menu_button(ui, layout, "Convert to Choice").clicked() {
                    graph.convert_to_choice(node_id);
                    graph.context_menu = None;
                }

                if context_menu_button(ui, layout, "Create Branch").clicked() {
                    graph.create_branch(node_id);
                    graph.context_menu = None;
                }

                if context_menu_button(ui, layout, "Connect To...").clicked() {
                    let from_port = node_snapshot
                        .as_ref()
                        .map(default_context_connect_port)
                        .unwrap_or(0);
                    graph.start_connection_pick(node_id, from_port);
                    graph.context_menu = None;
                }
                if context_menu_button(ui, layout, "Disconnect Outputs").clicked() {
                    graph.disconnect_all_from(node_id);
                    graph.context_menu = None;
                }

                ui.separator();
                ui.label(egui::RichText::new("Edit").small());
                if context_menu_button(ui, layout, "Edit Node").clicked() {
                    graph.editing = Some(node_id);
                    graph.context_menu = None;
                }

                if matches!(node_snapshot, Some(StoryNode::Scene { .. })) {
                    ui.separator();
                    ui.label(egui::RichText::new("Scene composition").small());
                    let profile_id = scene_profile
                        .clone()
                        .unwrap_or_else(|| format!("scene_{node_id}"));
                    if context_menu_button(ui, layout, "Group as Profile").clicked() {
                        graph.save_scene_profile(profile_id, node_id);
                        graph.context_menu = None;
                    }
                    if let Some(profile_id) = &scene_profile {
                        if context_menu_button(ui, layout, "Refresh from Profile").clicked() {
                            graph.apply_scene_profile(profile_id, node_id);
                            graph.context_menu = None;
                        }
                        if context_menu_button(ui, layout, "Ungroup / Detach Profile").clicked() {
                            graph.detach_scene_profile(node_id);
                            graph.context_menu = None;
                        }
                    }
                }

                if ui
                    .add_sized(
                        [layout.item_width(), 22.0],
                        egui::Button::new(egui::RichText::new("Delete").color(egui::Color32::RED)),
                    )
                    .clicked()
                {
                    graph.remove_node(node_id);
                    graph.context_menu = None;
                }
            });
        });
}

fn render_canvas_context_menu(
    graph: &mut NodeGraph,
    ui: &egui::Ui,
    position: egui::Pos2,
    graph_position: Option<egui::Pos2>,
) {
    let insert_pos = graph_position.unwrap_or(egui::pos2(0.0, 0.0));
    let bounds = ui.ctx().available_rect();
    let layout = context_menu_layout(bounds, canvas_context_menu_size());
    let menu_position = clamped_context_menu_position(position, bounds, layout.size());
    egui::Area::new(egui::Id::new("node_canvas_context_menu"))
        .fixed_pos(menu_position)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(layout.width);
                ui.set_max_width(layout.width);
                ui.label(egui::RichText::new("Create node").strong());
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(layout.list_max_height)
                    .auto_shrink([true, true])
                    .show(ui, |ui| {
                        let mut last_section = "";
                        for (label, node) in canvas_node_palette_items() {
                            let section = canvas_palette_section(label);
                            if section != last_section {
                                if !last_section.is_empty() {
                                    ui.separator();
                                }
                                ui.label(egui::RichText::new(section).small());
                                last_section = section;
                            }
                            if context_menu_button(ui, layout, label).clicked() {
                                add_canvas_node_from_palette(graph, node, insert_pos);
                                graph.context_menu = None;
                            }
                        }
                    });
            });
        });
}

fn context_menu_button(
    ui: &mut egui::Ui,
    layout: ContextMenuLayout,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.add_sized([layout.item_width(), 22.0], egui::Button::new(text))
}

pub fn canvas_palette_section(label: &str) -> &'static str {
    match label {
        "Dialogue" | "Choice" | "Scene" | "Jump" | "Start" | "End" => "Basic",
        "Scene Patch" | "Branch If" | "Set Variable" | "Set Flag" => "Logic",
        _ => "Media and advanced",
    }
}

pub fn add_canvas_node_from_palette(
    graph: &mut NodeGraph,
    node: StoryNode,
    insert_pos: egui::Pos2,
) -> u32 {
    let id = graph.add_node(node, insert_pos);
    if graph.connecting_from.is_some() {
        let changed = graph.finish_connection_to(id);
        if !changed {
            graph.set_single_selection(Some(id));
        }
    } else {
        graph.set_single_selection(Some(id));
    }
    id
}

pub fn canvas_node_palette_items() -> Vec<(&'static str, StoryNode)> {
    let mut items = vec![
        ("Dialogue", StoryNode::default()),
        (
            "Choice",
            StoryNode::Choice {
                prompt: "Choose:".to_string(),
                options: vec!["A".to_string(), "B".to_string()],
            },
        ),
        (
            "Scene",
            StoryNode::Scene {
                profile: None,
                background: None,
                music: None,
                characters: Vec::new(),
            },
        ),
        (
            "Jump",
            StoryNode::Jump {
                target: "label".to_string(),
            },
        ),
        ("Start", StoryNode::Start),
        ("End", StoryNode::End),
    ];
    items.extend(crate::editor::node_editor::extended_node_palette_items());
    items
}

pub fn default_context_connect_port(node: &StoryNode) -> usize {
    match node {
        StoryNode::Choice { options, .. } => options.len(),
        _ => 0,
    }
}

/// Renders the inline node editor window.
pub fn render_inline_editor(graph: &mut NodeGraph, ui: &egui::Ui) {
    let Some(editing_id) = graph.editing else {
        return;
    };

    let Some(original_node) = graph.get_node(editing_id).cloned() else {
        graph.editing = None;
        return;
    };

    let mut changed = false;
    let mut close_editor = false;
    let mut node_clone = original_node.clone();

    egui::Window::new("Edit Node")
        .collapsible(false)
        .resizable(true)
        .show(ui.ctx(), |ui| {
            match &mut node_clone {
                StoryNode::Dialogue { speaker, text } => {
                    ui.horizontal(|ui| {
                        ui.label("Speaker:");
                        changed |= ui.text_edit_singleline(speaker).changed();
                    });
                    ui.label("Text:");
                    changed |= ui
                        .add(egui::TextEdit::multiline(text).desired_rows(4))
                        .changed();
                }
                StoryNode::Choice { prompt, options } => {
                    ui.horizontal(|ui| {
                        ui.label("Prompt:");
                        changed |= ui.text_edit_singleline(prompt).changed();
                    });
                    ui.label("Options:");
                    for option in options.iter_mut() {
                        changed |= ui.text_edit_singleline(option).changed();
                    }
                    if ui.button("➕ Add Option").clicked() {
                        options.push("New Option".to_string());
                        changed = true;
                    }
                }
                StoryNode::Scene {
                    background, music, ..
                } => {
                    let mut bg = background.clone().unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label("Background:");
                        if ui.text_edit_singleline(&mut bg).changed() {
                            *background = if bg.trim().is_empty() { None } else { Some(bg) };
                            changed = true;
                        }
                    });
                    let mut bgm = music.clone().unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label("Music:");
                        if ui.text_edit_singleline(&mut bgm).changed() {
                            *music = if bgm.trim().is_empty() {
                                None
                            } else {
                                Some(bgm)
                            };
                            changed = true;
                        }
                    });
                }
                StoryNode::Jump { target } => {
                    ui.horizontal(|ui| {
                        ui.label("Target:");
                        changed |= ui.text_edit_singleline(target).changed();
                    });
                }
                StoryNode::SetVariable { key, value } => {
                    // Simple inline editor for var
                    ui.horizontal(|ui| {
                        ui.label("Var:");
                        changed |= ui.text_edit_singleline(key).changed();
                        ui.label("Val:");
                        // egui DragValue for i32
                        changed |= ui.add(egui::DragValue::new(value)).changed();
                    });
                }
                StoryNode::SetFlag { key, value } => {
                    ui.horizontal(|ui| {
                        ui.label("Flag:");
                        changed |= ui.text_edit_singleline(key).changed();
                        changed |= ui.checkbox(value, "Set").changed();
                    });
                }
                StoryNode::JumpIf { target, .. } => {
                    ui.horizontal(|ui| {
                        ui.label("Target:");
                        changed |= ui.text_edit_singleline(target).changed();
                    });
                    ui.label("(Edit condition in Inspector)");
                }
                StoryNode::ScenePatch(patch) => {
                    changed |= edit_scene_patch_inline(ui, patch);
                }
                StoryNode::AudioAction {
                    channel,
                    action,
                    asset,
                    volume,
                    fade_duration_ms,
                    loop_playback,
                } => {
                    changed |= edit_audio_action_inline(
                        ui,
                        channel,
                        action,
                        asset,
                        volume,
                        fade_duration_ms,
                        loop_playback,
                    );
                }
                StoryNode::Transition {
                    kind,
                    duration_ms,
                    color,
                } => {
                    changed |= edit_transition_inline(ui, kind, duration_ms, color);
                }
                StoryNode::Start | StoryNode::End => {
                    ui.label("This node has no editable properties.");
                }
                StoryNode::Generic(event) => match event {
                    visual_novel_engine::runtime::EventRaw::ExtCall { command, args } => {
                        ui.label("External Action");
                        ui.horizontal(|ui| {
                            ui.label("Command:");
                            changed |= ui.text_edit_singleline(command).changed();
                        });
                        ui.label("Args:");
                        for arg in args.iter_mut() {
                            changed |= ui.text_edit_singleline(arg).changed();
                        }
                        if ui.button("Add Arg").clicked() {
                            args.push(String::new());
                            changed = true;
                        }
                    }
                    _ => {
                        changed |= edit_generic_event_inline(ui, event);
                    }
                },
                StoryNode::CharacterPlacement { name, x, y, scale } => {
                    ui.label("Character Placement");
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        changed |= ui.text_edit_singleline(name).changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label("X:");
                        changed |= ui.add(egui::DragValue::new(x)).changed();
                        ui.label("Y:");
                        changed |= ui.add(egui::DragValue::new(y)).changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label("Scale:");
                        let mut s = scale.unwrap_or(1.0);
                        if ui.add(egui::DragValue::new(&mut s).speed(0.1)).changed() {
                            *scale = Some(s);
                            changed = true;
                        }
                    });
                }
                StoryNode::SubgraphCall {
                    fragment_id,
                    entry_port,
                    exit_port,
                } => {
                    ui.label("Subgraph Call");
                    ui.horizontal(|ui| {
                        ui.label("Fragment:");
                        changed |= ui.text_edit_singleline(fragment_id).changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label("Entry:");
                        let mut value = entry_port.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut value).changed() {
                            *entry_port = (!value.trim().is_empty()).then_some(value);
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Exit:");
                        let mut value = exit_port.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut value).changed() {
                            *exit_port = (!value.trim().is_empty()).then_some(value);
                            changed = true;
                        }
                    });
                }
            }

            ui.separator();
            if ui.button("✓ Done").clicked() {
                close_editor = true;
            }
        });

    // Apply changes
    if changed {
        graph.replace_node_with_hint(
            editing_id,
            node_clone.clone(),
            format!("Edited node {editing_id}"),
            format!("graph.nodes[{editing_id}]"),
            true,
        );
    }

    if close_editor {
        graph.editing = None;
    }
}
