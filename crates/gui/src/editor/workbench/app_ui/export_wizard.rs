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
                    if ui.button("Check export").clicked() {
                        plan = true;
                    }
                    let label = if self.export_wizard.dry_run {
                        "Test export"
                    } else {
                        "Save and export"
                    };
                    if ui.button(label).clicked() {
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
            if let Some(entry) = self.default_export_entry() {
                self.export_wizard.entry_script = entry.to_string_lossy().to_string();
            }
        }
        if self.export_wizard.runtime_artifact.trim().is_empty() {
            let executable = std::env::current_exe().ok();
            if let Some(runtime) = Self::discover_export_runtime(
                executable.as_deref().and_then(std::path::Path::parent),
                &std::env::current_dir().unwrap_or_default(),
                self.export_wizard.target,
            ) {
                self.export_wizard.runtime_artifact = runtime.to_string_lossy().to_string();
            }
        }
        self.show_export_wizard = true;
    }

    pub fn discover_export_runtime(
        executable_dir: Option<&std::path::Path>,
        working_dir: &std::path::Path,
        target: visual_novel_engine::ExportTargetPlatform,
    ) -> Option<std::path::PathBuf> {
        let name = if target == visual_novel_engine::ExportTargetPlatform::Windows {
            "vn_player.exe"
        } else {
            "vn_player"
        };
        let mut candidates = Vec::new();
        if let Some(dir) = executable_dir {
            // Installed editor/player pairs work independently of the current directory.
            if dir.file_name().is_none_or(|part| part != "debug") {
                candidates.push(dir.join(name));
            }
            if let Some(parent) = dir.parent() {
                candidates.push(parent.join("desktop").join(name));
                candidates.push(parent.join("release").join(name));
            }
        }
        candidates.push(working_dir.join("target/desktop").join(name));
        candidates.push(working_dir.join("target/release").join(name));
        candidates
            .into_iter()
            .find_map(|path| path.is_file().then(|| path.canonicalize().ok()).flatten())
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
}

#[path = "export_workflow.rs"]
mod workflow;

#[cfg(test)]
#[path = "export_wizard_tests.rs"]
mod tests;
