use super::*;

impl NodeEditorPanel<'_> {
    pub(super) fn render_toolbar(&mut self, ui: &mut egui::Ui) -> bool {
        let compact = node_toolbar_is_compact(ui.available_width());
        let mut layout_requested = false;
        ui.horizontal_wrapped(|ui| {
            let add_label = if compact { "+ Add" } else { "Add Node" };
            ui.menu_button(add_label, |ui| {
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

            ui.menu_button(if compact { "More" } else { "More Nodes" }, |ui| {
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
            let reset_label = if compact { "Reset" } else { "🔍 Reset View" };
            if ui.button(reset_label).clicked() {
                self.graph.reset_view();
            }
            egui::ComboBox::from_id_source("node_editor_layout_orientation")
                .selected_text(self.graph.layout_orientation.label())
                .width(if compact { 72.0 } else { 96.0 })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(
                            &mut self.graph.layout_orientation,
                            GraphLayoutOrientation::Vertical,
                            "Vertical",
                        )
                        .changed()
                    {
                        layout_requested = true;
                    }
                    if ui
                        .selectable_value(
                            &mut self.graph.layout_orientation,
                            GraphLayoutOrientation::Horizontal,
                            "Horizontal",
                        )
                        .changed()
                    {
                        layout_requested = true;
                    }
                });
            let layout_label = if compact { "Layout" } else { "Auto Layout" };
            if ui.button(layout_label).clicked() {
                layout_requested = true;
            }
            ui.label(format!("{:.0}%", self.graph.zoom() * 100.0));

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
            if !compact {
                ui.label(format!(
                    "Nodes: {} | Connections: {}",
                    self.graph.len(),
                    self.graph.connection_count()
                ));
            }
            if self.graph.is_modified() {
                ui.label("⚠ Modified");
            }
        });
        layout_requested
    }
}
