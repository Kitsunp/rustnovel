use super::*;

impl EditorWorkbench {
    pub(super) fn render_fragments_panel(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Fragments")
            .default_open(true)
            .show(ui, |ui| {
                self.render_fragment_toolbar(ui);
                self.render_fragment_issues(ui);
                self.render_fragment_list(ui);
            });
    }

    fn render_fragment_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let selected = self.node_graph.selected_node_ids();
            if ui
                .add_enabled(
                    !selected.is_empty(),
                    egui::Button::new("Create Fragment from Selection"),
                )
                .clicked()
            {
                let index = self.node_graph.fragments().len() + 1;
                let id = format!("fragment_{index}");
                if self.node_graph.create_fragment_from_selection(&id, &id) {
                    self.node_graph.mark_modified();
                    self.queue_editor_operation(
                        "fragment_created",
                        format!("Created fragment {id} from selected nodes"),
                        Some(format!("graph.fragments.{id}")),
                    );
                }
            }
            if ui.button("Leave Fragment").clicked() && self.node_graph.leave_fragment() {
                self.node_graph.mark_modified();
                self.queue_editor_operation(
                    "fragment_left",
                    "Left active fragment",
                    Some("graph.graph_stack".to_string()),
                );
            }
            if let Some(active) = self.node_graph.active_fragment() {
                ui.label(format!("Active: {active}"));
            }
        });
    }

    fn render_fragment_issues(&mut self, ui: &mut egui::Ui) {
        for issue in self.node_graph.fragment_validation_issues() {
            ui.colored_label(
                egui::Color32::YELLOW,
                format!("{} {}", issue.code.label(), issue.message),
            );
        }
    }

    fn render_fragment_list(&mut self, ui: &mut egui::Ui) {
        let fragments = self.node_graph.fragments();
        if fragments.is_empty() {
            ui.label("No fragments");
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(120.0)
            .show(ui, |ui| {
                for fragment in fragments {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "{} | nodes:{} in:{} out:{}",
                            fragment.fragment_id,
                            fragment.node_ids.len(),
                            fragment.inputs.len(),
                            fragment.outputs.len()
                        ));
                        self.render_fragment_actions(ui, &fragment.fragment_id);
                    });
                }
            });
    }

    fn render_fragment_actions(&mut self, ui: &mut egui::Ui, fragment_id: &str) {
        if ui.small_button("Enter").clicked() && self.node_graph.enter_fragment(fragment_id) {
            self.node_graph.mark_modified();
            self.queue_editor_operation(
                "fragment_entered",
                format!("Entered fragment {fragment_id}"),
                Some("graph.graph_stack.active_fragment".to_string()),
            );
        }
        if ui.small_button("Refresh Ports").clicked()
            && self.node_graph.refresh_fragment_ports(fragment_id)
        {
            self.node_graph.mark_modified();
            self.queue_editor_operation(
                "field_edited",
                format!("Refreshed ports for fragment {fragment_id}"),
                Some(format!("graph.fragments[{fragment_id}].ports")),
            );
        }
        if ui.small_button("Ungroup").clicked() && self.node_graph.remove_fragment(fragment_id) {
            self.node_graph.mark_modified();
            self.queue_editor_operation(
                "fragment_removed",
                format!("Removed fragment {fragment_id}"),
                Some(format!("graph.fragments.{fragment_id}")),
            );
        }
    }
}
