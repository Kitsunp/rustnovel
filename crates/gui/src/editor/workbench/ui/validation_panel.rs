use super::*;

impl EditorWorkbench {
    pub(super) fn render_validation_report_panel(&mut self, ui: &mut egui::Ui) {
        let error_count = self
            .validation_issues
            .iter()
            .filter(|issue| issue.severity == LintSeverity::Error)
            .count();
        let warning_count = self
            .validation_issues
            .iter()
            .filter(|issue| issue.severity == LintSeverity::Warning)
            .count();
        let info_count = self
            .validation_issues
            .iter()
            .filter(|issue| issue.severity == LintSeverity::Info)
            .count();
        let report_body_height = super::layout::validation_report_body_height(
            ui.available_height(),
            self.validation_issues.len(),
        );

        let mut close_validation = false;
        let mut toggle_validation_collapse = false;
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Validation Report | E:{} W:{} I:{}",
                        error_count, warning_count, info_count
                    ))
                    .strong(),
                );
                ui.separator();
                let collapse_label = if self.validation_collapsed {
                    "Expandir"
                } else {
                    "Minimizar"
                };
                if ui.small_button(collapse_label).clicked() {
                    toggle_validation_collapse = true;
                }
                if ui.small_button("Cerrar").clicked() {
                    close_validation = true;
                }
            });

            if self.validation_collapsed {
                return;
            }

            ui.add_space(2.0);
            if self.validation_issues.is_empty() {
                ui.colored_label(egui::Color32::GREEN, "No issues found.");
                return;
            }

            egui::ScrollArea::vertical()
                .id_source("validation_report_embedded_scroll")
                .max_height(report_body_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let lint_response = LintPanel::new(
                        &self.validation_issues,
                        &mut self.selected_node,
                        &mut self.selected_issue,
                        &mut self.diagnostic_language,
                        &self.node_graph,
                        self.last_fix_snapshot.is_some(),
                    )
                    .ui(ui);
                    self.handle_lint_panel_actions(lint_response.actions);
                });
        });

        if toggle_validation_collapse {
            self.validation_collapsed = !self.validation_collapsed;
        }
        if close_validation {
            self.show_validation = false;
            self.validation_collapsed = false;
        }
    }

    pub(super) fn handle_lint_panel_actions(
        &mut self,
        actions: Vec<crate::editor::lint_panel::LintPanelAction>,
    ) {
        for action in actions {
            match action {
                crate::editor::lint_panel::LintPanelAction::ApplyFix {
                    issue_index,
                    fix_id,
                    structural,
                } => {
                    if structural {
                        match self.prepare_structural_fix_confirmation(issue_index, &fix_id) {
                            Ok(()) => {
                                self.toast = Some(ToastState::warning(format!(
                                    "Review diff and confirm structural fix '{fix_id}'"
                                )));
                            }
                            Err(err) => {
                                self.toast = Some(ToastState::error(format!(
                                    "Fix '{fix_id}' preview failed: {err}"
                                )));
                            }
                        }
                    } else {
                        match self.apply_issue_fix(issue_index, &fix_id) {
                            Ok(()) => {
                                self.toast =
                                    Some(ToastState::success(format!("Applied fix '{fix_id}'")));
                            }
                            Err(err) => {
                                self.toast = Some(ToastState::error(format!(
                                    "Fix '{fix_id}' failed: {err}"
                                )));
                            }
                        }
                    }
                }
                crate::editor::lint_panel::LintPanelAction::ApplyAllSafeFixes => {
                    let applied = self.apply_all_safe_fixes();
                    if applied > 0 {
                        self.toast = Some(ToastState::success(format!(
                            "Applied {applied} safe fix(es)"
                        )));
                    } else {
                        self.toast = Some(ToastState::warning(
                            "No safe fixes available for current diagnostics",
                        ));
                    }
                }
                crate::editor::lint_panel::LintPanelAction::PrepareAutoFixBatch {
                    include_review,
                } => match self.prepare_autofix_batch_confirmation(include_review) {
                    Ok(planned) => {
                        self.toast = Some(ToastState::warning(format!(
                            "Review horizontal diff and confirm auto-fix batch ({planned} planned)"
                        )));
                    }
                    Err(err) => {
                        self.toast = Some(ToastState::warning(format!(
                            "Auto-fix batch not prepared: {err}"
                        )));
                    }
                },
                crate::editor::lint_panel::LintPanelAction::AutoFixIssue {
                    issue_index,
                    include_review,
                } => match self.apply_best_fix_for_issue(issue_index, include_review) {
                    Ok(outcome) => {
                        self.toast = Some(ToastState::success(outcome));
                    }
                    Err(err) => {
                        self.toast =
                            Some(ToastState::error(format!("Issue auto-fix failed: {err}")));
                    }
                },
                crate::editor::lint_panel::LintPanelAction::RevertLastFix => {
                    if self.revert_last_fix() {
                        self.toast = Some(ToastState::success("Last fix reverted successfully"));
                    } else {
                        self.toast = Some(ToastState::warning("No fix to revert"));
                    }
                }
            }
        }
    }
}
