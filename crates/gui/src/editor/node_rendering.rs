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

    let node_id = menu.node_id;
    let node_snapshot = graph.get_node(node_id).cloned();
    let scene_profile = match &node_snapshot {
        Some(StoryNode::Scene { profile, .. }) => profile.clone(),
        _ => None,
    };

    egui::Area::new(egui::Id::new("node_context_menu"))
        .fixed_pos(menu.position)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(160.0);

                ui.menu_button("Insert Node", |ui| {
                    if ui.button("Before").clicked() {
                        graph.insert_before(node_id, StoryNode::default());
                        graph.context_menu = None;
                        ui.close_menu();
                    }
                    if ui.button("After").clicked() {
                        graph.insert_after(node_id, StoryNode::default());
                        graph.context_menu = None;
                        ui.close_menu();
                    }
                });

                ui.separator();

                if ui.button("Convert to Choice").clicked() {
                    graph.convert_to_choice(node_id);
                    graph.context_menu = None;
                }

                if ui.button("Create Branch").clicked() {
                    graph.create_branch(node_id);
                    graph.context_menu = None;
                }

                ui.separator();

                if ui.button("Connect To...").clicked() {
                    graph.connecting_from = Some((node_id, 0));
                    graph.context_menu = None;
                }
                if ui.button("Disconnect Outputs").clicked() {
                    graph.disconnect_all_from(node_id);
                    graph.context_menu = None;
                }

                ui.separator();

                if ui.button("Edit").clicked() {
                    graph.editing = Some(node_id);
                    graph.context_menu = None;
                }

                if matches!(node_snapshot, Some(StoryNode::Scene { .. })) {
                    ui.separator();
                    ui.menu_button("Scene Composition", |ui| {
                        let profile_id = scene_profile
                            .clone()
                            .unwrap_or_else(|| format!("scene_{node_id}"));
                        if ui.button("Group as Profile").clicked() {
                            graph.save_scene_profile(profile_id, node_id);
                            graph.context_menu = None;
                            ui.close_menu();
                        }
                        if let Some(profile_id) = &scene_profile {
                            if ui.button("Refresh from Profile").clicked() {
                                graph.apply_scene_profile(profile_id, node_id);
                                graph.context_menu = None;
                                ui.close_menu();
                            }
                            if ui.button("Ungroup / Detach Profile").clicked() {
                                graph.detach_scene_profile(node_id);
                                graph.context_menu = None;
                                ui.close_menu();
                            }
                        }
                    });
                }

                if ui
                    .button(egui::RichText::new("Delete").color(egui::Color32::RED))
                    .clicked()
                {
                    graph.remove_node(node_id);
                    graph.context_menu = None;
                }
            });
        });
}

/// Renders the inline node editor window.
pub fn render_inline_editor(graph: &mut NodeGraph, ui: &egui::Ui) {
    let Some(editing_id) = graph.editing else {
        return;
    };

    let Some(node) = graph.get_node_mut(editing_id) else {
        graph.editing = None;
        return;
    };

    let mut changed = false;
    let mut close_editor = false;
    let mut node_clone = node.clone();

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
                    visual_novel_engine::EventRaw::ExtCall { command, args } => {
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
            }

            ui.separator();
            if ui.button("✓ Done").clicked() {
                close_editor = true;
            }
        });

    // Apply changes
    if changed {
        if let Some(node) = graph.get_node_mut(editing_id) {
            *node = node_clone;
        }
        graph.mark_modified();
    }

    if close_editor {
        graph.editing = None;
    }
}

/// Draws a bezier connection curve between two points.
pub fn draw_bezier_connection(painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2) {
    let delta = to - from;
    if delta.length_sq() <= f32::EPSILON {
        return;
    }

    let (control1, control2) = bezier_control_points(from, to);

    let points: Vec<egui::Pos2> = (0..=20)
        .map(|i| {
            let t = i as f32 / 20.0;
            let t2 = t * t;
            let t3 = t2 * t;
            let mt = 1.0 - t;
            let mt2 = mt * mt;
            let mt3 = mt2 * mt;

            egui::pos2(
                mt3 * from.x + 3.0 * mt2 * t * control1.x + 3.0 * mt * t2 * control2.x + t3 * to.x,
                mt3 * from.y + 3.0 * mt2 * t * control1.y + 3.0 * mt * t2 * control2.y + t3 * to.y,
            )
        })
        .collect();

    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 180, 100)),
    ));

    // Arrow head
    let arrow_size = 8.0;
    let mut arrow_dir = to - control2;
    if arrow_dir.length_sq() <= f32::EPSILON {
        arrow_dir = delta;
    }
    if arrow_dir.length_sq() <= f32::EPSILON {
        return;
    }
    let dir = arrow_dir.normalized();
    let arrow_left = to - dir * arrow_size + dir.rot90() * arrow_size * 0.5;
    let arrow_right = to - dir * arrow_size - dir.rot90() * arrow_size * 0.5;
    painter.add(egui::Shape::convex_polygon(
        vec![to, arrow_left, arrow_right],
        egui::Color32::from_rgb(100, 180, 100),
        egui::Stroke::NONE,
    ));
}

fn bezier_control_points(from: egui::Pos2, to: egui::Pos2) -> (egui::Pos2, egui::Pos2) {
    let delta = to - from;
    if delta.length_sq() <= f32::EPSILON {
        return (from, to);
    }

    let horizontal_bias = delta.x.abs() >= delta.y.abs();
    if horizontal_bias {
        let direction = delta.x.signum();
        let magnitude = (delta.x.abs() * 0.5).clamp(36.0, 220.0);
        let control_offset = magnitude * direction;
        (
            from + egui::vec2(control_offset, 0.0),
            to - egui::vec2(control_offset, 0.0),
        )
    } else {
        let direction = delta.y.signum();
        let magnitude = (delta.y.abs() * 0.5).clamp(36.0, 220.0);
        let control_offset = magnitude * direction;
        (
            from + egui::vec2(0.0, control_offset),
            to - egui::vec2(0.0, control_offset),
        )
    }
}

#[cfg(test)]
#[path = "node_rendering_tests.rs"]
mod tests;
