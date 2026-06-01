use std::collections::VecDeque;

use eframe::egui;
use visual_novel_engine::{runtime::ChoiceHistoryEntry, RouteTree};

/// GUI-only view for the engine route tree snapshot.
///
/// The route model itself stays in core. This component only owns presentation:
/// summary copy, scroll behavior, and visual grouping for editor/player menus.
pub struct RouteTreeView<'a> {
    route_tree: &'a RouteTree,
    choice_history: Option<&'a VecDeque<ChoiceHistoryEntry>>,
    max_height: f32,
}

impl<'a> RouteTreeView<'a> {
    pub fn new(route_tree: &'a RouteTree) -> Self {
        Self {
            route_tree,
            choice_history: None,
            max_height: 320.0,
        }
    }

    pub fn with_choice_history(mut self, choice_history: &'a VecDeque<ChoiceHistoryEntry>) -> Self {
        self.choice_history = Some(choice_history);
        self
    }

    pub fn with_max_height(mut self, max_height: f32) -> Self {
        self.max_height = max_height.max(96.0);
        self
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        self.render_summary(ui);
        ui.separator();
        egui::ScrollArea::vertical()
            .id_source("route_tree_view_scroll")
            .max_height(self.max_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.render_tree(ui);
                self.render_choice_history(ui);
                self.render_progress_snapshot(ui);
            });
    }

    fn render_summary(&self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Nodes: {}/{} visited",
                self.route_tree.coverage.visited_nodes, self.route_tree.coverage.total_nodes
            ));
            ui.separator();
            ui.label(format!(
                "Endings: {}/{} reached",
                self.route_tree.coverage.reached_endings, self.route_tree.coverage.total_endings
            ));
            if let Some(current) = self.route_tree.current {
                ui.separator();
                ui.label(format!("Current ip: {current}"));
            }
        });
    }

    fn render_tree(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Route Tree")
            .default_open(true)
            .show(ui, |ui| {
                for node in &self.route_tree.nodes {
                    let marker = if node.current {
                        "->"
                    } else if node.visited {
                        "ok"
                    } else {
                        ".."
                    };
                    let label = node.label.as_deref().unwrap_or("");
                    ui.label(format!("{marker} ip={} {:?} {}", node.ip, node.kind, label));
                    for edge in self
                        .route_tree
                        .edges
                        .iter()
                        .filter(|edge| edge.from == node.id)
                    {
                        let target = edge
                            .to
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "end".to_string());
                        let selected = if edge.selected { "selected" } else { "" };
                        let discovered = if edge.discovered { "seen" } else { "unseen" };
                        let label = edge.label.as_deref().unwrap_or("");
                        ui.label(format!(
                            "   -> {target} {:?} {label} {selected} {discovered}",
                            edge.kind
                        ));
                    }
                }
            });
    }

    fn render_choice_history(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Choice History")
            .default_open(true)
            .show(ui, |ui| {
                let Some(choice_history) = self.choice_history else {
                    ui.label("No choices selected in this run yet.");
                    return;
                };
                if choice_history.is_empty() {
                    ui.label("No choices selected in this run yet.");
                }
                for (idx, entry) in choice_history.iter().enumerate() {
                    ui.colored_label(
                        egui::Color32::from_rgb(235, 238, 245),
                        crate::player_route_history_label(idx, entry),
                    );
                }
            });
    }

    fn render_progress_snapshot(&self, ui: &mut egui::Ui) {
        if self.route_tree.progress.selected_choices.is_empty() {
            return;
        }
        egui::CollapsingHeader::new("Saved Progress Snapshot")
            .default_open(true)
            .show(ui, |ui| {
                for choice in &self.route_tree.progress.selected_choices {
                    ui.label(format!(
                        "ip {} -> option {} '{}' -> {}",
                        choice.event_ip, choice.option_index, choice.option_text, choice.target_ip
                    ));
                }
            });
    }
}
