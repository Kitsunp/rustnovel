use super::*;

impl EditorWorkbench {
    pub(super) fn render_export_wizard_window(&mut self, ctx: &egui::Context) {
        if !self.show_export_wizard {
            return;
        }
        let mut open = self.show_export_wizard;
        let mut choose_output = false;
        let mut choose_runtime = false;
        let mut plan = false;
        let mut execute = false;
        egui::Window::new("Export Game")
            .open(&mut open)
            .default_width(620.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Mode");
                    ui.selectable_value(
                        &mut self.export_wizard.export_kind,
                        ExportWizardKind::ExecutableGame,
                        "Executable game",
                    );
                    ui.selectable_value(
                        &mut self.export_wizard.export_kind,
                        ExportWizardKind::CompiledScriptBundle,
                        "Compiled script bundle",
                    );
                });
                self.export_wizard.require_executable =
                    self.export_wizard.export_kind == ExportWizardKind::ExecutableGame;
                ui.horizontal_wrapped(|ui| {
                    ui.label("Target");
                    egui::ComboBox::from_id_source("export_wizard_target")
                        .selected_text(self.export_wizard.target.as_str())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.export_wizard.target,
                                visual_novel_engine::ExportTargetPlatform::Windows,
                                "windows",
                            );
                            ui.selectable_value(
                                &mut self.export_wizard.target,
                                visual_novel_engine::ExportTargetPlatform::Linux,
                                "linux",
                            );
                            ui.selectable_value(
                                &mut self.export_wizard.target,
                                visual_novel_engine::ExportTargetPlatform::Macos,
                                "macos",
                            );
                        });
                    if self.export_wizard.target == visual_novel_engine::ExportTargetPlatform::Macos
                    {
                        ui.colored_label(egui::Color32::from_rgb(245, 190, 100), "experimental");
                    }
                    ui.label(if self.export_wizard.require_executable {
                        "release executable required"
                    } else {
                        "script bundle only"
                    });
                    ui.checkbox(&mut self.export_wizard.dry_run, "Dry-run");
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Output");
                    ui.text_edit_singleline(&mut self.export_wizard.output_root);
                    if ui.button("Pick").clicked() {
                        choose_output = true;
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Runtime");
                    ui.text_edit_singleline(&mut self.export_wizard.runtime_artifact);
                    if ui.button("Pick").clicked() {
                        choose_runtime = true;
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Entry");
                    ui.text_edit_singleline(&mut self.export_wizard.entry_script);
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Integrity");
                    egui::ComboBox::from_id_source("export_wizard_integrity")
                        .selected_text(match self.export_wizard.integrity {
                            visual_novel_engine::BundleIntegrity::None => "unsigned dev",
                            visual_novel_engine::BundleIntegrity::HmacSha256 => {
                                "HMAC release signed"
                            }
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.export_wizard.integrity,
                                visual_novel_engine::BundleIntegrity::None,
                                "unsigned dev",
                            );
                            ui.selectable_value(
                                &mut self.export_wizard.integrity,
                                visual_novel_engine::BundleIntegrity::HmacSha256,
                                "HMAC release signed",
                            );
                        });
                    if self.export_wizard.integrity
                        == visual_novel_engine::BundleIntegrity::HmacSha256
                    {
                        ui.text_edit_singleline(&mut self.export_wizard.hmac_key);
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Plan").clicked() {
                        plan = true;
                    }
                    if ui.button("Execute").clicked() {
                        execute = true;
                    }
                });

                if let Some(error) = &self.export_wizard.last_error {
                    ui.colored_label(egui::Color32::from_rgb(255, 140, 120), error);
                }
                if let Some(plan) = &self.export_wizard.last_plan {
                    ui.separator();
                    ui.label(format!("layout files: {}", plan.layout.len()));
                    let blocking = plan
                        .diagnostics
                        .iter()
                        .filter(|diagnostic| diagnostic.blocking_release)
                        .count();
                    ui.label(format!(
                        "diagnostics: {} total, {} blocking release",
                        plan.diagnostics.len(),
                        blocking
                    ));
                    Self::render_export_diagnostics(ui, &plan.diagnostics);
                    ui.collapsing("Plan JSON", |ui| {
                        let mut json = serde_json::to_string_pretty(plan)
                            .unwrap_or_else(|err| format!("serialization failed: {err}"));
                        ui.add(
                            egui::TextEdit::multiline(&mut json)
                                .font(egui::TextStyle::Monospace)
                                .desired_rows(12)
                                .interactive(false),
                        );
                    });
                }
                if let Some(report) = &self.export_wizard.last_report {
                    ui.separator();
                    for line in Self::export_report_summary_lines(report) {
                        ui.label(line);
                    }
                    Self::render_export_diagnostics(ui, &report.diagnostics);
                }
                if !self.export_wizard.logs.is_empty() {
                    ui.separator();
                    ui.collapsing("Export logs", |ui| {
                        for line in &self.export_wizard.logs {
                            ui.label(line);
                        }
                    });
                }
            });
        self.show_export_wizard = open;
        if choose_output {
            self.choose_export_wizard_output();
        }
        if choose_runtime {
            self.choose_export_wizard_runtime();
        }
        if plan {
            self.plan_export_wizard();
        }
        if execute {
            self.execute_export_wizard();
        }
    }

    pub fn open_export_wizard(&mut self) {
        if self.export_wizard.output_root.trim().is_empty() {
            if let Some(root) = self.project_root.as_ref() {
                self.export_wizard.output_root = root
                    .join("export")
                    .join(self.export_wizard.target.as_str())
                    .to_string_lossy()
                    .to_string();
            }
        }
        if self.export_wizard.entry_script.trim().is_empty() {
            if let Some(manifest) = self.manifest.as_ref() {
                self.export_wizard.entry_script = manifest.settings.entry_point.clone();
            }
        }
        self.show_export_wizard = true;
    }

    fn choose_export_wizard_output(&mut self) {
        let start = self.project_root.as_deref();
        let mut dialog = rfd::FileDialog::new();
        if let Some(start) = start {
            dialog = dialog.set_directory(start);
        }
        if let Some(path) = dialog.pick_folder() {
            self.export_wizard.output_root = path.to_string_lossy().to_string();
        }
    }

    fn choose_export_wizard_runtime(&mut self) {
        let start = self.project_root.as_deref();
        let mut dialog = rfd::FileDialog::new();
        if let Some(start) = start {
            dialog = dialog.set_directory(start);
        }
        if self.export_wizard.target == visual_novel_engine::ExportTargetPlatform::Windows {
            dialog = dialog.add_filter("Windows executable", &["exe"]);
        }
        if let Some(path) = dialog.pick_file() {
            self.export_wizard.runtime_artifact = path.to_string_lossy().to_string();
        }
    }

    fn render_export_diagnostics(
        ui: &mut egui::Ui,
        diagnostics: &[visual_novel_engine::ExportDiagnostic],
    ) {
        if diagnostics.is_empty() {
            return;
        }
        ui.collapsing("Structured diagnostics", |ui| {
            for diagnostic in diagnostics {
                let color = if diagnostic.blocking_release || diagnostic.severity == "error" {
                    egui::Color32::from_rgb(255, 140, 120)
                } else {
                    egui::Color32::from_rgb(245, 190, 100)
                };
                ui.colored_label(
                    color,
                    format!(
                        "{} [{}] {}",
                        diagnostic.code, diagnostic.trace_id, diagnostic.message
                    ),
                );
                ui.label(format!("cause: {}", diagnostic.probable_cause));
                ui.label(format!("action: {}", diagnostic.suggested_action));
                ui.label(format!("consequence: {}", diagnostic.consequence));
                ui.separator();
            }
        });
    }

    fn export_blocking_diagnostics_summary(
        diagnostics: &[visual_novel_engine::ExportDiagnostic],
    ) -> String {
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.blocking_release)
            .map(|diagnostic| {
                let mut scope = Vec::new();
                if let Some(file) = &diagnostic.file {
                    scope.push(format!("file={file}"));
                }
                if let Some(asset) = &diagnostic.asset {
                    scope.push(format!("asset={asset}"));
                }
                if let Some(node) = &diagnostic.node {
                    scope.push(format!("node={node}"));
                }
                if let Some(field) = &diagnostic.field {
                    scope.push(format!("field={field}"));
                }
                let scope = if scope.is_empty() {
                    "scope=project".to_string()
                } else {
                    scope.join(" ")
                };
                format!(
                    "{} trace_id={} {} action={}",
                    diagnostic.code, diagnostic.trace_id, scope, diagnostic.suggested_action
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub fn export_report_summary_lines(
        report: &visual_novel_engine::ExportBundleReport,
    ) -> Vec<String> {
        vec![
            format!("Exported assets: {}", report.assets_copied),
            format!(
                "Runtime: {} sha256={}",
                report.runtime_artifact.as_deref().unwrap_or("none"),
                report.runtime_artifact_sha256.as_deref().unwrap_or("none")
            ),
            format!("Launcher: {}", report.launcher),
            format!(
                "Executable: {}",
                report.executable.as_deref().unwrap_or("none")
            ),
            format!("Expected executable: {}", report.expected_executable),
            format!(
                "Backend: {} fallback={}",
                report.graphics_backend, report.wgpu_fallback
            ),
            format!(
                "Payload: files={} total_size={}",
                report.hashes.len(),
                report.total_size
            ),
            format!(
                "Manifest: {} sha256={}",
                report
                    .bundle_file_manifest
                    .as_deref()
                    .unwrap_or("meta/bundle_file_manifest.json"),
                report
                    .bundle_file_manifest_sha256
                    .as_deref()
                    .unwrap_or("none")
            ),
            format!(
                "Compat: {}",
                report
                    .compat_report
                    .as_deref()
                    .unwrap_or("meta/compat_report.json")
            ),
            format!(
                "Integrity: {} scope={} hmac={}",
                report.integrity,
                report.integrity_scope,
                report.bundle_hmac_sha256.as_deref().unwrap_or("none")
            ),
            format!(
                "Smoke: status={} backend={} trace_id={}",
                report.smoke_result.status,
                report.smoke_result.backend,
                report.smoke_result.trace_id
            ),
        ]
    }

    fn export_wizard_spec(&self) -> Result<visual_novel_engine::ExportBundleSpec, String> {
        let project_root = self
            .project_root
            .clone()
            .or_else(|| {
                self.pending_save_path
                    .as_ref()
                    .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
            })
            .ok_or_else(|| "load/save a project first so project_root is known".to_string())?;
        let output_root = self.export_wizard.output_root.trim();
        if output_root.is_empty() {
            return Err("choose an output folder".to_string());
        }
        let entry_script = if self.export_wizard.entry_script.trim().is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(
                self.export_wizard.entry_script.trim(),
            ))
        };
        let runtime_artifact = if self.export_wizard.runtime_artifact.trim().is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(
                self.export_wizard.runtime_artifact.trim(),
            ))
        };
        let hmac_key = if self.export_wizard.hmac_key.trim().is_empty() {
            None
        } else {
            Some(self.export_wizard.hmac_key.clone())
        };
        Ok(visual_novel_engine::ExportBundleSpec {
            project_root,
            output_root: std::path::PathBuf::from(output_root),
            target_platform: self.export_wizard.target,
            entry_script,
            runtime_artifact,
            integrity: self.export_wizard.integrity,
            output_layout_version: 1,
            hmac_key,
        })
    }

    fn plan_export_wizard(&mut self) {
        self.export_wizard.last_error = None;
        self.export_wizard.last_report = None;
        self.export_wizard.logs.clear();
        self.export_wizard.logs.push(format!(
            "plan: target={} mode={:?} integrity={}",
            self.export_wizard.target.as_str(),
            self.export_wizard.export_kind,
            self.export_wizard.integrity.as_str()
        ));
        let spec = match self.export_wizard_spec() {
            Ok(spec) => spec,
            Err(err) => {
                self.export_wizard.last_error = Some(err.clone());
                self.toast = Some(ToastState::error(format!("Export plan failed: {err}")));
                return;
            }
        };
        match visual_novel_engine::ExportService::new().plan_export(&spec) {
            Ok(plan) => {
                let blocking = plan
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.blocking_release)
                    .count();
                let has_errors = plan
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity == "error");
                self.export_wizard.logs.push(format!(
                    "plan complete: diagnostics={} blocking={}",
                    plan.diagnostics.len(),
                    blocking
                ));
                self.export_wizard.last_plan = Some(plan);
                self.toast = Some(if blocking > 0 {
                    ToastState::warning("Export plan has release-blocking diagnostics")
                } else if has_errors {
                    ToastState::warning("Export plan has error diagnostics")
                } else {
                    ToastState::success("Export plan ready")
                });
            }
            Err(err) => {
                self.export_wizard.last_plan = None;
                self.export_wizard.last_error = Some(err.to_string());
                self.toast = Some(ToastState::error(format!("Export plan failed: {err}")));
            }
        }
    }

    fn execute_export_wizard(&mut self) {
        self.export_wizard.last_error = None;
        self.export_wizard
            .logs
            .push("execute: validating plan".to_string());
        let spec = match self.export_wizard_spec() {
            Ok(spec) => spec,
            Err(err) => {
                self.export_wizard.last_error = Some(err.clone());
                self.toast = Some(ToastState::error(format!("Export failed: {err}")));
                return;
            }
        };
        match visual_novel_engine::ExportService::new().plan_export(&spec) {
            Ok(plan) => {
                let has_error_diagnostics = plan
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity == "error");
                let has_blocking_diagnostics = plan
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.blocking_release);
                if has_error_diagnostics {
                    self.export_wizard.last_plan = Some(plan);
                    self.toast = Some(ToastState::error("Export blocked by plan diagnostics"));
                    return;
                }
                if self.export_wizard.require_executable && has_blocking_diagnostics {
                    let diagnostics = Self::export_blocking_diagnostics_summary(&plan.diagnostics);
                    self.export_wizard.last_plan = Some(plan);
                    self.export_wizard.last_error = Some(format!(
                        "Executable export blocked by diagnostics: {diagnostics}"
                    ));
                    self.toast = Some(ToastState::error("Export blocked by release diagnostics"));
                    return;
                }
                if self.export_wizard.require_executable {
                    let expected = spec.target_platform.expected_executable_name();
                    if plan.executable.as_deref() != Some(expected) {
                        self.export_wizard.last_plan = Some(plan);
                        self.export_wizard.last_error = Some(format!(
                        "{} export cannot produce {}; missing expected executable after planning",
                        spec.target_platform.as_str(),
                        expected
                    ));
                        self.toast = Some(ToastState::error(
                            "Export blocked by executable diagnostics",
                        ));
                        return;
                    }
                }
                self.export_wizard.last_plan = Some(plan);
            }
            Err(err) => {
                self.export_wizard.last_error = Some(err.to_string());
                self.toast = Some(ToastState::error(format!("Export plan failed: {err}")));
                return;
            }
        }
        if self.export_wizard.dry_run {
            self.export_wizard
                .logs
                .push("execute: dry-run stopped before writing bundle".to_string());
            self.toast = Some(ToastState::success(
                "Export dry-run completed without writing",
            ));
            return;
        }
        let result = if self.export_wizard.require_executable {
            visual_novel_engine::export_executable_bundle(spec)
        } else {
            visual_novel_engine::ExportService::new().execute_export(spec)
        };
        match result {
            Ok(report) => {
                self.export_wizard.logs.push(format!(
                    "execute complete: launcher={} executable={} compat={}",
                    report.launcher,
                    report.executable.as_deref().unwrap_or("none"),
                    report.compat_report.as_deref().unwrap_or("none")
                ));
                self.export_wizard.last_report = Some(report.clone());
                self.last_export_report = Some(report);
                self.show_export_report_panel = true;
                self.toast = Some(ToastState::success("Game bundle exported"));
            }
            Err(err) => {
                self.export_wizard.last_error = Some(err.to_string());
                self.toast = Some(ToastState::error(format!("Export failed: {err}")));
            }
        }
    }
}

#[cfg(test)]
#[path = "export_wizard_tests.rs"]
mod tests;
