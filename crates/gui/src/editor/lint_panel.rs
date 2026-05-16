//! Validation panel for displaying lint issues.

use super::quick_fix::{suggest_fixes, QuickFixCandidate, QuickFixRisk};
use super::{DiagnosticLanguage, LintIssue, LintSeverity, NodeGraph};
use eframe::egui;

#[derive(Debug, Clone)]
pub enum LintPanelAction {
    ApplyFix {
        issue_index: usize,
        fix_id: String,
        structural: bool,
    },
    ApplyAllSafeFixes,
    PrepareAutoFixBatch {
        include_review: bool,
    },
    AutoFixIssue {
        issue_index: usize,
        include_review: bool,
    },
    RevertLastFix,
}

#[derive(Debug, Default)]
pub struct LintPanelResponse {
    pub actions: Vec<LintPanelAction>,
}

/// Panel for displaying validation results.
pub struct LintPanel<'a> {
    issues: &'a [LintIssue],
    selected_node: &'a mut Option<u32>,
    selected_issue: &'a mut Option<usize>,
    language: &'a mut DiagnosticLanguage,
    graph: &'a NodeGraph,
    can_revert_fix: bool,
}

impl<'a> LintPanel<'a> {
    pub fn new(
        issues: &'a [LintIssue],
        selected_node: &'a mut Option<u32>,
        selected_issue: &'a mut Option<usize>,
        language: &'a mut DiagnosticLanguage,
        graph: &'a NodeGraph,
        can_revert_fix: bool,
    ) -> Self {
        Self {
            issues,
            selected_node,
            selected_issue,
            language,
            graph,
            can_revert_fix,
        }
    }

    pub fn ui(self, ui: &mut egui::Ui) -> LintPanelResponse {
        let mut response = LintPanelResponse::default();

        ui.heading("Validation Report");
        ui.separator();

        ui.horizontal_wrapped(|ui| {
            ui.label("Language:");
            egui::ComboBox::from_id_source("diagnostic_language")
                .selected_text(self.language.label())
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(*self.language == DiagnosticLanguage::Es, "ES")
                        .clicked()
                    {
                        *self.language = DiagnosticLanguage::Es;
                    }
                    if ui
                        .selectable_label(*self.language == DiagnosticLanguage::En, "EN")
                        .clicked()
                    {
                        *self.language = DiagnosticLanguage::En;
                    }
                });
            ui.separator();
            if ui.button("Apply all safe fixes").clicked() {
                response.actions.push(LintPanelAction::ApplyAllSafeFixes);
            }
            if ui.button("Auto-fix complete (review)").clicked() {
                response.actions.push(LintPanelAction::PrepareAutoFixBatch {
                    include_review: true,
                });
            }
            if ui
                .add_enabled(self.can_revert_fix, egui::Button::new("Revert last fix"))
                .clicked()
            {
                response.actions.push(LintPanelAction::RevertLastFix);
            }
        });
        ui.separator();

        if self.issues.is_empty() {
            ui.label(egui::RichText::new("No issues found.").color(egui::Color32::GREEN));
            return response;
        }

        let error_count = self
            .issues
            .iter()
            .filter(|i| i.severity == LintSeverity::Error)
            .count();
        let warning_count = self
            .issues
            .iter()
            .filter(|i| i.severity == LintSeverity::Warning)
            .count();
        let info_count = self
            .issues
            .iter()
            .filter(|i| i.severity == LintSeverity::Info)
            .count();

        ui.label(format!(
            "Found {} errors, {} warnings, {} infos.",
            error_count, warning_count, info_count
        ));
        ui.separator();

        let issue_list_height = (ui.available_height() * 0.45).clamp(48.0, 220.0);
        egui::ScrollArea::vertical()
            .max_height(issue_list_height)
            .show(ui, |ui| {
                for (idx, issue) in self.issues.iter().enumerate() {
                    let icon = match issue.severity {
                        LintSeverity::Error => "ERROR",
                        LintSeverity::Warning => "WARN",
                        LintSeverity::Info => "INFO",
                    };

                    let color = match issue.severity {
                        LintSeverity::Error => egui::Color32::RED,
                        LintSeverity::Warning => egui::Color32::YELLOW,
                        LintSeverity::Info => egui::Color32::LIGHT_BLUE,
                    };

                    let selected = *self.selected_issue == Some(idx);
                    let text = egui::RichText::new(format!(
                        "{} [{}] {}",
                        icon,
                        issue.diagnostic_id(),
                        issue.localized_message(*self.language)
                    ))
                    .color(color);

                    let resp = ui.selectable_label(selected, text);

                    if resp.clicked() {
                        *self.selected_issue = Some(idx);
                        *self.selected_node = self.graph.focus_node_for_issue(issue);
                    }

                    ui.separator();
                }
            });

        if let Some(issue_idx) = *self.selected_issue {
            if let Some(issue) = self.issues.get(issue_idx) {
                let explanation = issue.explanation(*self.language);
                egui::CollapsingHeader::new("Error -> Cause -> Action")
                    .id_source("lint_issue_details")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Ocultar detalle").clicked() {
                                *self.selected_issue = None;
                            }
                        });
                        ui.label(format!("diagnostic_id: {}", issue.diagnostic_id()));
                        ui.label(format!(
                            "phase={} code={}",
                            issue.phase.label(),
                            issue.code.label()
                        ));
                        if let Some(node_id) = issue.node_id {
                            ui.label(format!("node_id={node_id}"));
                        }
                        if let (Some(edge_from), Some(edge_to)) = (issue.edge_from, issue.edge_to) {
                            ui.label(format!("edge={edge_from}->{edge_to}"));
                        } else if let Some(edge_from) = issue.edge_from {
                            ui.label(format!("edge_from={edge_from}"));
                        }
                        if let Some(event_ip) = issue.event_ip {
                            ui.label(format!("event_ip={event_ip}"));
                        }
                        if let Some(asset_path) = &issue.asset_path {
                            ui.label(format!("asset={asset_path}"));
                        }
                        if let Some(blocked_by) = &issue.blocked_by {
                            ui.label(format!("blocked_by={blocked_by}"));
                        }

                        ui.separator();
                        ui.label(egui::RichText::new("Cause").strong());
                        ui.label(explanation.root_cause);
                        ui.label(egui::RichText::new("Why failed").strong());
                        ui.label(explanation.why_failed);
                        ui.label(egui::RichText::new("How to fix").strong());
                        ui.label(explanation.how_to_fix);
                        ui.horizontal_wrapped(|ui| {
                            ui.hyperlink_to(
                                "Open diagnostic docs",
                                diagnostic_docs_url(&explanation.docs_ref),
                            );
                            ui.label(explanation.docs_ref);
                        });
                    });
                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Auto-fix selected (safe)").clicked() {
                        response.actions.push(LintPanelAction::AutoFixIssue {
                            issue_index: issue_idx,
                            include_review: false,
                        });
                    }
                    if ui.button("Auto-fix selected (full)").clicked() {
                        response.actions.push(LintPanelAction::AutoFixIssue {
                            issue_index: issue_idx,
                            include_review: true,
                        });
                    }
                });
                ui.separator();

                let fixes = suggest_fixes(issue, self.graph);
                if fixes.is_empty() {
                    ui.label("No deterministic quick-fix available for this issue.");
                } else {
                    ui.label(egui::RichText::new("Available quick-fixes").strong());
                    for fix in fixes {
                        render_fix_card(ui, fix, *self.language, &mut response.actions, issue_idx);
                    }
                }
            }
        }

        response
    }
}

fn render_fix_card(
    ui: &mut egui::Ui,
    fix: QuickFixCandidate,
    language: DiagnosticLanguage,
    out: &mut Vec<LintPanelAction>,
    issue_idx: usize,
) {
    ui.group(|ui| {
        ui.label(egui::RichText::new(fix.title(language)).strong());
        ui.label(format!(
            "risk={} structural={}",
            fix.risk.label(),
            fix.structural
        ));
        ui.label(format!("pre: {}", fix.preconditions(language)));
        ui.label(format!("post: {}", fix.postconditions(language)));

        let label = match fix.risk {
            QuickFixRisk::Safe => "Apply fix",
            QuickFixRisk::Review => "Apply fix (review)",
        };
        if ui.button(label).clicked() {
            out.push(LintPanelAction::ApplyFix {
                issue_index: issue_idx,
                fix_id: fix.fix_id.to_string(),
                structural: fix.structural,
            });
        }
    });
    ui.separator();
}

fn diagnostic_docs_url(docs_ref: &str) -> String {
    let (path, anchor) = docs_ref
        .split_once('#')
        .map_or((docs_ref, None), |(path, anchor)| (path, Some(anchor)));
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let absolute = repo_root.join(path);
    let normalized = absolute.to_string_lossy().replace('\\', "/");
    let mut url = if normalized.starts_with('/') {
        format!("file://{normalized}")
    } else {
        format!("file:///{normalized}")
    };
    if let Some(anchor) = anchor {
        url.push('#');
        url.push_str(anchor);
    }
    url
}

#[cfg(test)]
mod tests {
    #[test]
    fn diagnostic_docs_url_resolves_relative_docs_ref_to_local_file_url() {
        let url = super::diagnostic_docs_url("docs/diagnostics/authoring.md#val-asset-not-found");

        assert!(url.starts_with("file:///"), "{url}");
        assert!(url.contains("/docs/diagnostics/authoring.md#val-asset-not-found"));
        assert!(!url.contains('\\'));
    }
}
