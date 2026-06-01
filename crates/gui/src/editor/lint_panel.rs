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
        let report_text = validation_report_copy_text(self.issues, *self.language);
        ui.horizontal(|ui| {
            if ui.button("Copy report").clicked() {
                ui.output_mut(|output| {
                    output.copied_text = report_text.clone();
                });
            }
            ui.label("Full report is selectable below.");
        });
        egui::CollapsingHeader::new("Copyable report")
            .id_source("lint_copyable_report")
            .default_open(false)
            .show(ui, |ui| {
                let mut copyable = report_text.clone();
                ui.add(
                    egui::TextEdit::multiline(&mut copyable)
                        .desired_rows(8)
                        .desired_width(f32::INFINITY),
                );
            });
        ui.separator();

        let issue_list_height = (ui.available_height() * 0.45).clamp(48.0, 220.0);
        egui::ScrollArea::vertical()
            .id_source("lint_issue_list_scroll")
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

                    ui.horizontal(|ui| {
                        let resp = ui.selectable_label(selected, text);

                        if resp.clicked() {
                            *self.selected_issue = Some(idx);
                            *self.selected_node = self.graph.focus_node_for_issue(issue);
                        }
                        if ui.small_button("Copy").clicked() {
                            let issue_text =
                                validation_report_issue_copy_text(issue, idx, *self.language);
                            ui.output_mut(|output| {
                                output.copied_text = issue_text;
                            });
                        }
                    });

                    ui.separator();
                }
            });

        if let Some(issue_idx) = *self.selected_issue {
            if let Some(issue) = self.issues.get(issue_idx) {
                let explanation = issue.explanation(*self.language);
                egui::CollapsingHeader::new(issue_detail_heading(issue.severity))
                    .id_source("lint_issue_details")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Ocultar detalle").clicked() {
                                *self.selected_issue = None;
                            }
                            if ui.button("Copy issue").clicked() {
                                let issue_text = validation_report_issue_copy_text(
                                    issue,
                                    issue_idx,
                                    *self.language,
                                );
                                ui.output_mut(|output| {
                                    output.copied_text = issue_text;
                                });
                            }
                            if ui.button("Copy diagnostic ID").clicked() {
                                ui.output_mut(|output| {
                                    output.copied_text = issue.diagnostic_id();
                                });
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
                        ui.separator();
                        egui::CollapsingHeader::new("Copyable selected issue")
                            .id_source("lint_copyable_selected_issue")
                            .default_open(false)
                            .show(ui, |ui| {
                                let mut copyable = validation_report_issue_copy_text(
                                    issue,
                                    issue_idx,
                                    *self.language,
                                );
                                ui.add(
                                    egui::TextEdit::multiline(&mut copyable)
                                        .desired_rows(6)
                                        .desired_width(f32::INFINITY),
                                );
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

pub fn validation_report_copy_text(issues: &[LintIssue], language: DiagnosticLanguage) -> String {
    if issues.is_empty() {
        return "Validation Report\nNo issues found.".to_string();
    }
    let mut out = String::from("Validation Report\n");
    for (idx, issue) in issues.iter().enumerate() {
        out.push('\n');
        append_validation_issue_copy_text(&mut out, issue, idx, language);
    }
    out
}

pub fn validation_report_issue_copy_text(
    issue: &LintIssue,
    issue_index: usize,
    language: DiagnosticLanguage,
) -> String {
    let mut out = String::new();
    append_validation_issue_copy_text(&mut out, issue, issue_index, language);
    out
}

fn append_validation_issue_copy_text(
    out: &mut String,
    issue: &LintIssue,
    issue_index: usize,
    language: DiagnosticLanguage,
) {
    let explanation = issue.explanation(language);
    out.push_str(&format!(
        "#{} {} [{}]\n",
        issue_index + 1,
        issue.severity.label(),
        issue.diagnostic_id()
    ));
    out.push_str(&format!("diagnostic_id={}\n", issue.diagnostic_id()));
    out.push_str(&format!(
        "phase={} code={}\n",
        issue.phase.label(),
        issue.code.label()
    ));
    if let Some(node_id) = issue.node_id {
        out.push_str(&format!("node_id={node_id}\n"));
    }
    if let Some(event_ip) = issue.event_ip {
        out.push_str(&format!("event_ip={event_ip}\n"));
    }
    if let (Some(edge_from), Some(edge_to)) = (issue.edge_from, issue.edge_to) {
        out.push_str(&format!("edge={edge_from}->{edge_to}\n"));
    } else if let Some(edge_from) = issue.edge_from {
        out.push_str(&format!("edge_from={edge_from}\n"));
    } else if let Some(edge_to) = issue.edge_to {
        out.push_str(&format!("edge_to={edge_to}\n"));
    }
    if let Some(asset_path) = &issue.asset_path {
        out.push_str(&format!("asset={asset_path}\n"));
    }
    if let Some(blocked_by) = &issue.blocked_by {
        out.push_str(&format!("blocked_by={blocked_by}\n"));
    }
    if let Some(target) = &issue.target {
        out.push_str(&format!("target={}\n", target.stable_key()));
    }
    if let Some(field_path) = &issue.field_path {
        out.push_str(&format!("field_path={}\n", field_path.value));
    }
    out.push_str(&format!("message={}\n", issue.localized_message(language)));
    out.push_str(&format!("cause={}\n", explanation.root_cause));
    out.push_str(&format!("why_failed={}\n", explanation.why_failed));
    out.push_str(&format!("how_to_fix={}\n", explanation.how_to_fix));
    out.push_str(&format!("docs={}\n", explanation.docs_ref));
}

fn issue_detail_heading(severity: LintSeverity) -> &'static str {
    match severity {
        LintSeverity::Error => "Error -> Cause -> Action",
        LintSeverity::Warning => "Warning -> Cause -> Action",
        LintSeverity::Info => "Info -> Meaning -> Action",
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

pub fn diagnostic_docs_url(docs_ref: &str) -> String {
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
