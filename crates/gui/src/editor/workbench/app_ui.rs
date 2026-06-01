use super::*;

impl EditorWorkbench {
    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_menu_bar").show(ctx, |ui| {
            crate::editor::menu_bar::render_menu_bar(ui, self);
        });

        egui::TopBottomPanel::top("mode_switcher").show(ctx, |ui| {
            self.render_mode_switcher(ui, ctx);
        });

        match self.mode {
            EditorMode::Player => self.render_player_mode(ctx),
            EditorMode::Editor => self.render_editor_mode(ctx),
        }

        self.handle_save_confirmation(ctx);
        self.handle_fix_confirmation(ctx);
        self.render_player_menu_settings_window(ctx);
        self.render_theme_editor_window(ctx);
        self.render_export_wizard_window(ctx);
        self.render_scene_frame_inspector_window(ctx);
        self.render_export_report_panel(ctx);
        self.render_profiler_cache_panel(ctx);
        self.render_layout_debug_overlay(ctx);
        self.persist_layout_prefs_if_changed();
    }

    fn render_mode_switcher(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            let (label, color) = match self.mode {
                EditorMode::Editor => ("EDITOR", egui::Color32::from_rgb(70, 130, 220)),
                EditorMode::Player => ("PLAYER", egui::Color32::from_rgb(230, 140, 50)),
            };
            ui.label(
                egui::RichText::new(format!("Modo: {}", label))
                    .strong()
                    .color(color),
            );
            ui.separator();

            if ui
                .selectable_label(self.mode == EditorMode::Editor, "Edit")
                .clicked()
            {
                self.mode = EditorMode::Editor;
            }
            if ui
                .selectable_label(self.mode == EditorMode::Player, "Play")
                .clicked()
                && self.prepare_player_mode()
            {
                self.mode = EditorMode::Player;
            }

            ui.separator();
            if ui.button("Validar (Dry Run)").clicked() {
                self.run_dry_validation();
            }
            if ui.button("Compilar").clicked() {
                self.compile_preview();
            }
            if ui.button("Menu Player").clicked() {
                self.show_player_menu_settings = true;
            }
            if ui.button("Theme").clicked() {
                self.show_theme_editor = true;
            }
            if ui.button("Guardar").clicked() {
                self.prepare_save_confirmation();
            }

            match super::layout::editor_toolbar_mode(ui.available_width()) {
                super::layout::EditorToolbarMode::Expanded => {
                    if ui.button("Exportar script compilado").clicked() {
                        self.export_compiled_project();
                    }
                    if ui.button("Exportar juego").clicked() {
                        self.open_export_wizard();
                    }
                    if ui.button("Exportar Repro Dry Run").clicked() {
                        self.export_dry_run_repro();
                    }
                    if ui.button("Exportar Repro Case").clicked() {
                        self.export_repro_case();
                    }
                    if ui.button("Importar Repro Case").clicked() {
                        self.import_repro_case();
                    }
                    if ui.button("Ejecutar Repro Cargado").clicked() {
                        self.run_loaded_repro_case();
                    }
                    if ui.button("Exportar Reporte Diagnostico").clicked() {
                        self.export_diagnostic_report();
                    }
                    if ui.button("Importar Reporte Diagnostico").clicked() {
                        self.import_diagnostic_report();
                    }
                    if ui.button("Reset Layout").clicked() {
                        self.reset_layout_state(ctx);
                    }
                }
                super::layout::EditorToolbarMode::Grouped => {
                    ui.menu_button("Exportar", |ui| {
                        if ui.button("Script compilado").clicked() {
                            self.export_compiled_project();
                            ui.close_menu();
                        }
                        if ui.button("Juego").clicked() {
                            self.open_export_wizard();
                            ui.close_menu();
                        }
                    });
                    ui.menu_button("Repro", |ui| {
                        if ui.button("Exportar Dry Run").clicked() {
                            self.export_dry_run_repro();
                            ui.close_menu();
                        }
                        if ui.button("Exportar Case").clicked() {
                            self.export_repro_case();
                            ui.close_menu();
                        }
                        if ui.button("Importar Case").clicked() {
                            self.import_repro_case();
                            ui.close_menu();
                        }
                        if ui.button("Ejecutar cargado").clicked() {
                            self.run_loaded_repro_case();
                            ui.close_menu();
                        }
                    });
                    ui.menu_button("Diagnostico", |ui| {
                        if ui.button("Exportar reporte").clicked() {
                            self.export_diagnostic_report();
                            ui.close_menu();
                        }
                        if ui.button("Importar reporte").clicked() {
                            self.import_diagnostic_report();
                            ui.close_menu();
                        }
                    });
                    if ui.button("Reset Layout").clicked() {
                        self.reset_layout_state(ctx);
                    }
                }
            }
        });
    }

    fn handle_save_confirmation(&mut self, ctx: &egui::Context) {
        let mut should_save = false;
        if self.show_save_confirm {
            if let Some(dialog) = &self.diff_dialog {
                if dialog.show(ctx, &mut self.show_save_confirm) {
                    should_save = true;
                }
            }
        }
        if !should_save {
            return;
        }

        if self.run_dry_validation() {
            if let Some(path) = self.pending_save_path.clone() {
                self.execute_save(&path, "");
                self.toast = Some(ToastState::success("Saved successfully"));
            }
        } else {
            self.toast = Some(ToastState::error(
                "Save blocked: fix validation errors first",
            ));
        }
        self.diff_dialog = None;
        self.show_save_confirm = false;
    }

    fn handle_fix_confirmation(&mut self, ctx: &egui::Context) {
        let mut should_apply = false;
        if self.show_fix_confirm {
            if let Some(dialog) = &self.fix_diff_dialog {
                if dialog.show(ctx, &mut self.show_fix_confirm) {
                    should_apply = true;
                }
            }
        }

        if should_apply {
            self.apply_confirmed_fix();
        } else if !self.show_fix_confirm {
            self.pending_structural_fix = None;
            self.pending_auto_fix_batch = None;
            self.fix_diff_dialog = None;
        }
    }

    fn apply_confirmed_fix(&mut self) {
        if self.pending_auto_fix_batch.is_some() {
            match self.apply_pending_autofix_batch() {
                Ok(result) => {
                    let summary = format!(
                        "Auto-fix batch applied: {} applied, {} skipped",
                        result.applied, result.skipped
                    );
                    if let Some(first_skip) = result.skipped_details.first() {
                        let remaining = result.skipped_details.len().saturating_sub(1);
                        let suffix = if remaining == 0 {
                            String::new()
                        } else {
                            format!(" (+{remaining} more)")
                        };
                        self.toast = Some(ToastState::warning(format!(
                            "{summary}; first skipped fix '{}' for {} failed: {}{}",
                            first_skip.fix_id, first_skip.diagnostic_id, first_skip.reason, suffix
                        )));
                    } else {
                        self.toast = Some(ToastState::success(summary));
                    }
                }
                Err(err) => {
                    self.toast = Some(ToastState::error(format!("Auto-fix batch failed: {err}")));
                }
            }
        } else {
            match self.apply_pending_structural_fix() {
                Ok(fix_id) => {
                    self.toast = Some(ToastState::success(format!(
                        "Applied structural fix '{fix_id}'"
                    )));
                }
                Err(err) => {
                    self.toast = Some(ToastState::error(format!("Structural fix failed: {err}")));
                }
            }
        }
        self.fix_diff_dialog = None;
        self.show_fix_confirm = false;
    }

    fn render_theme_editor_window(&mut self, ctx: &egui::Context) {
        if !self.show_theme_editor {
            return;
        }
        if self.theme_editor_draft.is_none() {
            self.theme_editor_draft = Some(ThemeEditorDraft {
                original: self.active_ui_theme.clone(),
                draft: self.active_ui_theme.clone(),
                preview_applied: false,
            });
        }

        let mut open = self.show_theme_editor;
        let mut apply_theme = false;
        let mut preview_theme = false;
        let mut revert_theme = false;
        let mut save_theme = false;
        egui::Window::new("Theme Editor")
            .open(&mut open)
            .default_width(520.0)
            .resizable(true)
            .show(ctx, |ui| {
                let Some(editor) = self.theme_editor_draft.as_mut() else {
                    ui.label("Theme editor draft is not available.");
                    return;
                };
                let theme = &mut editor.draft;
                ui.horizontal_wrapped(|ui| {
                    ui.label("Theme id");
                    ui.text_edit_singleline(&mut theme.id);
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Locale");
                    let locale = theme.locale.get_or_insert_with(String::new);
                    ui.text_edit_singleline(locale);
                    if ui.button("Clear").clicked() {
                        theme.locale = None;
                    }
                });

                ui.separator();
                ui.collapsing("Colors", |ui| {
                    let keys = theme.colors.keys().cloned().collect::<Vec<_>>();
                    for key in keys {
                        if let Some(value) = theme.colors.get_mut(&key) {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(key);
                                ui.text_edit_singleline(value);
                            });
                        }
                    }
                    if ui.button("Add custom color token").clicked() {
                        theme
                            .colors
                            .entry("custom.accent".to_string())
                            .or_insert_with(|| "#66CCFF".to_string());
                    }
                });

                ui.collapsing("Typography", |ui| {
                    let keys = theme.typography.keys().cloned().collect::<Vec<_>>();
                    for key in keys {
                        if let Some(token) = theme.typography.get_mut(&key) {
                            ui.group(|ui| {
                                ui.label(key);
                                ui.horizontal_wrapped(|ui| {
                                    ui.label("Font");
                                    ui.text_edit_singleline(&mut token.font_family);
                                });
                                ui.add(egui::Slider::new(&mut token.size, 8.0..=64.0).text("Size"));
                                ui.add(
                                    egui::Slider::new(&mut token.weight, 100..=900).text("Weight"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut token.line_height, 0.8..=2.4)
                                        .text("Line height"),
                                );
                            });
                        }
                    }
                });

                ui.collapsing("Action Text", |ui| {
                    let keys = theme.action_text.keys().cloned().collect::<Vec<_>>();
                    for key in keys {
                        if let Some(value) = theme.action_text.get_mut(&key) {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(key);
                                ui.text_edit_singleline(value);
                            });
                        }
                    }
                });

                ui.separator();
                let validation = visual_novel_engine::validate_ui_theme(theme);
                if validation.valid {
                    ui.colored_label(egui::Color32::from_rgb(120, 220, 150), "Theme valid");
                } else {
                    ui.colored_label(egui::Color32::from_rgb(255, 120, 120), "Theme invalid");
                }
                for warning in &validation.warnings {
                    ui.label(format!("warning: {warning}"));
                }
                for error in &validation.errors {
                    ui.colored_label(egui::Color32::from_rgb(255, 150, 120), error);
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Preview").clicked() {
                        preview_theme = true;
                    }
                    if ui.button("Apply").clicked() {
                        apply_theme = true;
                    }
                    if ui.button("Revert").clicked() {
                        revert_theme = true;
                    }
                    if ui.button("Save as theme").clicked() {
                        save_theme = true;
                    }
                });
                ui.collapsing("Theme JSON", |ui| {
                    let mut json = serde_json::to_string_pretty(theme)
                        .unwrap_or_else(|err| format!("serialization failed: {err}"));
                    ui.add(
                        egui::TextEdit::multiline(&mut json)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(12)
                            .interactive(false),
                    );
                });
            });
        self.show_theme_editor = open;
        if !open {
            self.theme_editor_draft = None;
        } else if preview_theme {
            if let Some(editor) = self.theme_editor_draft.as_mut() {
                self.active_ui_theme = editor.draft.clone();
                editor.preview_applied = true;
                self.toast = Some(ToastState::success("Theme preview applied"));
            }
        } else if apply_theme {
            if let Some(editor) = self.theme_editor_draft.take() {
                self.active_ui_theme = editor.draft;
                self.show_theme_editor = false;
                self.toast = Some(ToastState::success("Theme applied"));
            }
        } else if revert_theme {
            if let Some(editor) = self.theme_editor_draft.as_mut() {
                self.active_ui_theme = editor.original.clone();
                editor.draft = editor.original.clone();
                editor.preview_applied = false;
                self.toast = Some(ToastState::success("Theme reverted"));
            }
        } else if save_theme {
            self.save_theme_editor_draft();
        }
    }

    fn render_export_wizard_window(&mut self, ctx: &egui::Context) {
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

    fn save_theme_editor_draft(&mut self) {
        let Some(editor) = self.theme_editor_draft.as_ref() else {
            self.toast = Some(ToastState::warning("No theme draft to save"));
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("VN Theme", &["vntheme.json", "json"])
            .set_file_name(format!("{}.vntheme.json", editor.draft.id))
            .save_file()
        else {
            self.toast = Some(ToastState::warning("Theme export cancelled"));
            return;
        };
        let payload = match serde_json::to_string_pretty(&editor.draft) {
            Ok(payload) => payload,
            Err(err) => {
                self.toast = Some(ToastState::error(format!(
                    "Theme serialization failed: {err}"
                )));
                return;
            }
        };
        match crate::editor::atomic_io::atomic_replace(&path, payload.as_bytes()) {
            Ok(()) => self.toast = Some(ToastState::success("Theme saved")),
            Err(err) => self.toast = Some(ToastState::error(format!("Theme save failed: {err}"))),
        }
    }

    fn render_scene_frame_inspector_window(&mut self, ctx: &egui::Context) {
        if !self.show_scene_frame_inspector {
            return;
        }
        let mut open = self.show_scene_frame_inspector;
        egui::Window::new("SceneFrame Inspector")
            .open(&mut open)
            .default_width(560.0)
            .resizable(true)
            .show(ctx, |ui| {
                let Some(engine) = self.engine.as_ref() else {
                    ui.label("Start Play Mode to inspect the current SceneFrame.");
                    return;
                };
                let display = Self::display_profile_from_context(ctx);
                let mut frame = engine.scene_frame();
                frame.layout = Some(visual_novel_engine::resolve_layout(
                    display.clone(),
                    visual_novel_engine::StageProfile::default(),
                    visual_novel_engine::LayoutPolicy::default(),
                ));
                let mut presenter = crate::editor::EguiSceneFramePresenter::default();
                let response = visual_novel_engine::SceneFramePresenter::present(
                    &mut presenter,
                    &frame,
                    &display,
                    &self.active_ui_theme,
                );
                ui.label(format!("schema: {}", frame.frame_schema));
                ui.label(format!("commands: {}", frame.commands.len()));
                ui.label(format!("interactions: {}", frame.interactions.len()));
                if let Some(route) = &frame.route {
                    ui.label(format!(
                        "route nodes: {} / current: {:?}",
                        route.nodes.len(),
                        route.current
                    ));
                }
                if !response.diagnostics.is_empty() {
                    ui.separator();
                    for diagnostic in &response.diagnostics {
                        ui.label(diagnostic);
                    }
                }
                ui.collapsing("Presenter Preview", |ui| {
                    presenter.show_debug_bounds = true;
                    let preview_response =
                        presenter.present_egui(ui, &frame, &display, &self.active_ui_theme);
                    for diagnostic in preview_response.diagnostics {
                        ui.label(diagnostic);
                    }
                });
                ui.collapsing("Frame JSON", |ui| {
                    let mut json = serde_json::to_string_pretty(&frame)
                        .unwrap_or_else(|err| format!("serialization failed: {err}"));
                    ui.add(
                        egui::TextEdit::multiline(&mut json)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(14)
                            .interactive(false),
                    );
                });
            });
        self.show_scene_frame_inspector = open;
    }

    fn render_export_report_panel(&mut self, ctx: &egui::Context) {
        if !self.show_export_report_panel {
            return;
        }
        let mut open = self.show_export_report_panel;
        egui::Window::new("Export Report")
            .open(&mut open)
            .default_width(560.0)
            .resizable(true)
            .show(ctx, |ui| {
                let Some(report) = &self.last_export_report else {
                    ui.label("No game bundle export has completed in this session.");
                    return;
                };
                ui.label(format!("target: {}", report.target_platform));
                ui.label(format!("assets copied: {}", report.assets_copied));
                ui.label(format!("launcher: {}", report.launcher));
                ui.label(format!(
                    "executable: {}",
                    report.executable.as_deref().unwrap_or("none")
                ));
                ui.label(format!("integrity: {}", report.integrity));
                if !report.integrity_scope.is_empty() {
                    ui.label(format!("integrity scope: {}", report.integrity_scope));
                }
                if let Some(signature) = &report.bundle_hmac_sha256 {
                    ui.label(format!("hmac: {signature}"));
                }
                ui.collapsing("Report JSON", |ui| {
                    let mut json = serde_json::to_string_pretty(report)
                        .unwrap_or_else(|err| format!("serialization failed: {err}"));
                    ui.add(
                        egui::TextEdit::multiline(&mut json)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(14)
                            .interactive(false),
                    );
                });
            });
        self.show_export_report_panel = open;
    }

    fn render_profiler_cache_panel(&mut self, ctx: &egui::Context) {
        if !self.show_profiler_cache_panel {
            return;
        }
        let mut open = self.show_profiler_cache_panel;
        egui::Window::new("Profiler / Cache")
            .open(&mut open)
            .default_width(420.0)
            .resizable(true)
            .show(ctx, |ui| {
                let metrics = self.resource_service.metrics();
                ui.label(format!("asset cache bytes: {}", metrics.bytes));
                ui.label(format!("asset cache peak bytes: {}", metrics.peak_bytes));
                ui.label(format!("byte entries: {}", metrics.byte_entries));
                ui.label(format!("image entries: {}", metrics.image_entries));
                ui.label(format!("audio entries: {}", metrics.audio_entries));
                ui.label(format!("hits: {}", metrics.hits));
                ui.label(format!("misses: {}", metrics.misses));
                ui.label(format!("evictions: {}", metrics.evictions));
                ui.label(format!("decode ms: {}", metrics.decode_ms));
                ui.label(format!("upload ms: {}", metrics.upload_ms));
                let (compile_hits, compile_misses) = self.compilation_cache_stats();
                ui.separator();
                ui.label(format!("compile cache hits: {compile_hits}"));
                ui.label(format!("compile cache misses: {compile_misses}"));
                if ui.button("Clear asset cache").clicked() {
                    self.resource_service.clear();
                    self.toast = Some(ToastState::success("Asset cache cleared"));
                }
            });
        self.show_profiler_cache_panel = open;
    }

    fn render_layout_debug_overlay(&mut self, ctx: &egui::Context) {
        if !self.show_layout_debug_overlay {
            return;
        }
        let display = Self::display_profile_from_context(ctx);
        let layout = visual_novel_engine::resolve_layout(
            display.clone(),
            visual_novel_engine::StageProfile::default(),
            visual_novel_engine::LayoutPolicy::default(),
        );
        let layer = egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("vnengine_layout_debug_overlay"),
        );
        let painter = ctx.layer_painter(layer);
        let rect = egui::Rect::from_min_size(
            egui::pos2(layout.stage_rect.x, layout.stage_rect.y),
            egui::vec2(layout.stage_rect.width, layout.stage_rect.height),
        );
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.5, egui::Color32::from_rgb(90, 190, 255)),
        );

        egui::Area::new(egui::Id::new("layout_debug_overlay_panel"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
            .show(ctx, |ui| {
                egui::Frame::popup(&ctx.style()).show(ui, |ui| {
                    ui.label(format!(
                        "{}x{} logical",
                        display.logical_size[0] as u32, display.logical_size[1] as u32
                    ));
                    ui.label(format!(
                        "{}x{} physical",
                        display.physical_size[0], display.physical_size[1]
                    ));
                    ui.label(format!("scale factor: {:.2}", display.scale_factor));
                    ui.label(format!("user scale: {:.2}", display.user_scale));
                    ui.label(format!("breakpoint: {}", layout.breakpoint));
                    ui.label(format!(
                        "stage: {:.0},{:.0} {:.0}x{:.0}",
                        layout.stage_rect.x,
                        layout.stage_rect.y,
                        layout.stage_rect.width,
                        layout.stage_rect.height
                    ));
                });
            });
    }

    fn display_profile_from_context(ctx: &egui::Context) -> visual_novel_engine::DisplayProfile {
        let screen = ctx.screen_rect();
        let scale = ctx.pixels_per_point();
        let mut display = visual_novel_engine::DisplayProfile::new(screen.width(), screen.height());
        display.scale_factor = scale;
        display.physical_size = [
            (screen.width() * scale).round().max(1.0) as u32,
            (screen.height() * scale).round().max(1.0) as u32,
        ];
        display.orientation = if screen.width() >= screen.height() {
            visual_novel_engine::DisplayOrientation::Landscape
        } else {
            visual_novel_engine::DisplayOrientation::Portrait
        };
        display
    }

    fn render_player_menu_settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_player_menu_settings {
            return;
        }

        let mut open = self.show_player_menu_settings;
        let mut changed = false;
        let mut reset_defaults = false;
        egui::Window::new("Player Menu Settings")
            .open(&mut open)
            .default_width(520.0)
            .resizable(true)
            .show(ctx, |ui| {
                let Some(manifest) = self.manifest.as_mut() else {
                    ui.label("Open a project manifest before editing the player menu.");
                    return;
                };
                let menu = &mut manifest.settings.player_menu;

                changed |= ui.checkbox(&mut menu.enabled, "Enabled").changed();
                changed |= ui
                    .checkbox(&mut menu.open_on_start, "Open on start")
                    .changed();
                changed |= ui
                    .checkbox(&mut menu.advance_on_text_panel_click, "Text panel advances")
                    .changed();
                ui.horizontal_wrapped(|ui| {
                    ui.label("Title");
                    changed |= ui.text_edit_singleline(&mut menu.title).changed();
                });
                changed |= ui
                    .add(egui::Slider::new(&mut menu.save_slots, 1..=24).text("Save slots"))
                    .changed();

                ui.separator();
                ui.collapsing("Layout", |ui| {
                    changed |= combo_value(
                        ui,
                        "Panel anchor",
                        &mut menu.layout.panel_anchor,
                        &[
                            visual_novel_engine::PlayerMenuPanelAnchor::Center,
                            visual_novel_engine::PlayerMenuPanelAnchor::TopLeft,
                            visual_novel_engine::PlayerMenuPanelAnchor::TopRight,
                            visual_novel_engine::PlayerMenuPanelAnchor::BottomLeft,
                            visual_novel_engine::PlayerMenuPanelAnchor::BottomRight,
                        ],
                    );
                    changed |= combo_value(
                        ui,
                        "Quick actions",
                        &mut menu.layout.quick_action_placement,
                        &[
                            visual_novel_engine::PlayerMenuQuickActionPlacement::MenuHeader,
                            visual_novel_engine::PlayerMenuQuickActionPlacement::Toolbar,
                            visual_novel_engine::PlayerMenuQuickActionPlacement::Hidden,
                        ],
                    );
                    changed |= combo_value(
                        ui,
                        "Tabs",
                        &mut menu.layout.tabs_position,
                        &[
                            visual_novel_engine::PlayerMenuTabsPosition::Left,
                            visual_novel_engine::PlayerMenuTabsPosition::Top,
                        ],
                    );
                });

                ui.collapsing("Responsive Style", |ui| {
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.panel_width, 320.0..=1280.0)
                                .text("Panel width"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.panel_width_fraction, 0.25..=1.0)
                                .text("Viewport width"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.panel_height_fraction, 0.25..=0.95)
                                .text("Viewport height"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.button_min_width, 48.0..=240.0)
                                .text("Button min width"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.button_height, 24.0..=64.0)
                                .text("Button height"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.button_corner_radius, 0.0..=32.0)
                                .text("Button corner radius"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.panel_alpha, 120..=255)
                                .text("Panel alpha"),
                        )
                        .changed();
                });

                ui.collapsing("Color Palette", |ui| {
                    changed |= color_value(ui, "Background", &mut menu.style.background);
                    changed |= color_value(ui, "Accent", &mut menu.style.accent);
                    changed |= color_value(ui, "Warning", &mut menu.style.warning);
                    changed |= color_value(ui, "Danger", &mut menu.style.danger);
                });

                ui.collapsing("Quick Action Buttons", |ui| {
                    egui::ScrollArea::vertical()
                        .id_source("player_menu_quick_actions_scroll")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for action in &mut menu.quick_actions {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(format!("{:?}", action.action));
                                    changed |=
                                        ui.checkbox(&mut action.visible, "Visible").changed();
                                    changed |= ui.text_edit_singleline(&mut action.label).changed();
                                });
                            }
                        });
                });

                ui.collapsing("Tabs", |ui| {
                    for tab in &mut menu.tabs {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!("{:?}", tab.kind));
                            changed |= ui.checkbox(&mut tab.visible, "Visible").changed();
                            changed |= ui.text_edit_singleline(&mut tab.label).changed();
                        });
                    }
                });

                ui.separator();
                if ui.button("Reset Player Menu Defaults").clicked() {
                    reset_defaults = true;
                    changed = true;
                }
            });

        self.show_player_menu_settings = open;
        if reset_defaults {
            if let Some(manifest) = self.manifest.as_mut() {
                manifest.settings.player_menu = visual_novel_engine::PlayerMenuConfig::default();
            }
        }
        if changed {
            self.persist_player_menu_settings();
        }
    }

    fn persist_player_menu_settings(&mut self) {
        let Some(manifest) = self.manifest.as_mut() else {
            return;
        };
        manifest.settings.player_menu = manifest.settings.player_menu.normalized();
        let Some(path) = self.manifest_path.clone() else {
            self.toast = Some(ToastState::warning(
                "Player menu changed in memory; no manifest path is loaded",
            ));
            return;
        };
        match manifest.save(&path) {
            Ok(()) => {
                self.toast = Some(ToastState::success("Player menu settings saved"));
            }
            Err(err) => {
                self.toast = Some(ToastState::error(format!(
                    "Player menu settings save failed: {err}"
                )));
            }
        }
    }
}

fn combo_value<T>(ui: &mut egui::Ui, label: &str, current: &mut T, values: &[T]) -> bool
where
    T: Copy + std::fmt::Debug + PartialEq,
{
    let before = *current;
    egui::ComboBox::from_label(label)
        .selected_text(format!("{:?}", current))
        .show_ui(ui, |ui| {
            for value in values {
                ui.selectable_value(current, *value, format!("{:?}", value));
            }
        });
    before != *current
}

fn color_value(
    ui: &mut egui::Ui,
    label: &str,
    color: &mut visual_novel_engine::PlayerMenuColor,
) -> bool {
    let before = *color;
    ui.group(|ui| {
        ui.horizontal_wrapped(|ui| {
            let preview = egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
            ui.colored_label(preview, label);
            ui.label(format!(
                "#{:02X}{:02X}{:02X} / {}",
                color.r, color.g, color.b, color.a
            ));
        });
        ui.add(egui::Slider::new(&mut color.r, 0..=255).text("R"));
        ui.add(egui::Slider::new(&mut color.g, 0..=255).text("G"));
        ui.add(egui::Slider::new(&mut color.b, 0..=255).text("B"));
        ui.add(egui::Slider::new(&mut color.a, 0..=255).text("A"));
    });
    before != *color
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::*;
    use crate::VnConfig;
    use visual_novel_engine::runtime::{DialogueRaw, EventRaw, ScriptRaw};

    #[test]
    fn export_wizard_blocks_executable_on_release_blocking_diagnostics() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_root = tmp.path().join("project");
        fs::create_dir_all(&project_root).expect("project dir");
        visual_novel_engine::ProjectManifest::new("game", "studio")
            .save(&project_root.join("project.vnm"))
            .expect("manifest");
        let script = ScriptRaw::new(
            vec![EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "hello".to_string(),
            })],
            BTreeMap::from([("start".to_string(), 0)]),
        );
        fs::write(
            project_root.join("main.json"),
            script.to_json().expect("script json"),
        )
        .expect("script");

        let mut workbench = EditorWorkbench::new(VnConfig::default());
        workbench.project_root = Some(project_root.clone());
        workbench.manifest = Some(visual_novel_engine::ProjectManifest::new("game", "studio"));
        workbench.export_wizard.output_root =
            project_root.join("dist").to_string_lossy().to_string();
        workbench.export_wizard.entry_script = "main.json".to_string();
        workbench.export_wizard.runtime_artifact.clear();
        workbench.export_wizard.require_executable = true;
        workbench.export_wizard.export_kind = ExportWizardKind::ExecutableGame;
        workbench.export_wizard.dry_run = false;

        workbench.plan_export_wizard();
        let plan = workbench.export_wizard.last_plan.as_ref().expect("plan");
        assert!(plan.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "export.runtime_artifact.missing"
                && diagnostic.blocking_release
                && diagnostic.trace_id.starts_with("export-")
        }));
        assert!(workbench.export_wizard.last_error.is_none());

        workbench.execute_export_wizard();
        let error = workbench
            .export_wizard
            .last_error
            .as_deref()
            .expect("blocking error");
        assert!(
            error.contains("Executable export blocked by diagnostics"),
            "{error}"
        );
        assert!(error.contains("export.runtime_artifact.missing"), "{error}");
        assert!(error.contains("trace_id=export-"), "{error}");
        assert!(error.contains("field=runtime_artifact"), "{error}");
        assert!(
            !project_root.join("dist").exists(),
            "blocked executable export must not publish output"
        );
    }
}
