use super::*;

#[derive(Clone, Copy)]
enum ValidationPanelMode {
    OpenIfIssues,
    PreserveManualCloseUnlessErrors,
}

impl EditorWorkbench {
    fn append_localization_issues(&mut self, script: &visual_novel_engine::ScriptRaw) {
        if self.localization_catalog.locales.is_empty() {
            return;
        }

        let required = visual_novel_engine::collect_script_localization_keys(script);
        if required.is_empty() {
            return;
        }

        let issues = self
            .localization_catalog
            .validate_keys(required.iter().map(std::string::String::as_str));
        for issue in issues {
            let message = match issue.kind {
                visual_novel_engine::LocalizationIssueKind::MissingKey => format!(
                    "[i18n] Missing key '{}' in locale '{}'",
                    issue.key, issue.locale
                ),
                visual_novel_engine::LocalizationIssueKind::OrphanKey => format!(
                    "[i18n] Orphan key '{}' in locale '{}'",
                    issue.key, issue.locale
                ),
            };
            self.validation_issues.push(LintIssue::warning(
                None,
                ValidationPhase::Graph,
                LintCode::CompileError,
                message,
            ));
        }
    }

    fn apply_compilation_state(
        &mut self,
        script: &visual_novel_engine::ScriptRaw,
        dry_run_report: &Option<crate::editor::compiler::DryRunReport>,
        issues: &[LintIssue],
        phase_trace: &[crate::editor::compiler::PhaseTrace],
        panel_mode: ValidationPanelMode,
    ) -> bool {
        self.current_script = Some(script.clone());
        self.last_dry_run_report = dry_run_report.clone();
        self.validation_issues = issues.to_vec();
        Self::append_phase_trace_issues(&mut self.validation_issues, phase_trace);
        if let Some(script) = self.current_script.as_ref().cloned() {
            self.append_localization_issues(&script);
        }
        if self
            .selected_issue
            .is_some_and(|idx| idx >= self.validation_issues.len())
        {
            self.selected_issue = None;
        }

        let has_errors = self
            .validation_issues
            .iter()
            .any(|issue| issue.severity == LintSeverity::Error);

        match panel_mode {
            ValidationPanelMode::OpenIfIssues => {
                self.show_validation = !self.validation_issues.is_empty();
                self.validation_collapsed = false;
            }
            ValidationPanelMode::PreserveManualCloseUnlessErrors => {
                if self.validation_issues.is_empty() {
                    self.show_validation = false;
                    self.validation_collapsed = false;
                } else if has_errors {
                    // Keep critical diagnostics visible automatically.
                    self.show_validation = true;
                }
            }
        }

        has_errors
    }

    pub fn run_dry_validation(&mut self) -> bool {
        let result = self.compile_current_graph();
        let has_errors = self.apply_compilation_state(
            &result.script,
            &result.dry_run_report,
            &result.issues,
            &result.phase_trace,
            ValidationPanelMode::OpenIfIssues,
        );
        if has_errors {
            self.toast = Some(ToastState::error("Validation found blocking errors"));
            return false;
        }

        match result.engine_result {
            Ok(engine) => {
                self.engine = Some(engine);
                self.refresh_scene_from_engine_preview();
                self.toast = Some(ToastState::success("Dry Run completed"));
                true
            }
            Err(e) => {
                self.validation_issues.push(LintIssue::error(
                    None,
                    ValidationPhase::Runtime,
                    LintCode::RuntimeInitError,
                    format!("Runtime initialization failed: {}", e),
                ));
                self.show_validation = true;
                self.toast = Some(ToastState::error("Validation failed at runtime init"));
                false
            }
        }
    }

    pub fn compile_preview(&mut self) -> bool {
        let ok = self.run_dry_validation();
        if ok {
            self.toast = Some(ToastState::success("Compilation preview successful"));
        }
        ok
    }

    pub fn export_compiled_project(&mut self) {
        if !self.run_dry_validation() {
            return;
        }

        let Some(script) = self.current_script.as_ref() else {
            self.toast = Some(ToastState::error("No script to export"));
            return;
        };

        let compiled = match script.compile() {
            Ok(compiled) => compiled,
            Err(e) => {
                self.toast = Some(ToastState::error(format!("Compile failed: {}", e)));
                return;
            }
        };

        let bytes = match compiled.to_binary() {
            Ok(bytes) => bytes,
            Err(e) => {
                self.toast = Some(ToastState::error(format!("Binary export failed: {}", e)));
                return;
            }
        };

        let path = rfd::FileDialog::new()
            .add_filter("VN Project", &["vnproject"])
            .set_file_name("game.vnproject")
            .save_file();

        if let Some(path) = path {
            match std::fs::write(&path, bytes) {
                Ok(_) => {
                    self.toast = Some(ToastState::success("Exported .vnproject successfully"));
                }
                Err(e) => {
                    self.toast = Some(ToastState::error(format!("Export failed: {}", e)));
                }
            }
        } else {
            self.toast = Some(ToastState::warning("Export cancelled"));
        }
    }

    pub fn package_bundle_native(&mut self) {
        let project_root = self.project_root.clone().or_else(|| {
            self.pending_save_path
                .as_ref()
                .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
        });
        let Some(project_root) = project_root else {
            self.toast = Some(ToastState::error(
                "Package failed: load/save a project first so project_root is known",
            ));
            return;
        };

        let Some(output_root) = rfd::FileDialog::new()
            .set_directory(&project_root)
            .pick_folder()
        else {
            self.toast = Some(ToastState::warning("Package cancelled"));
            return;
        };

        let entry_script = self
            .manifest
            .as_ref()
            .map(|manifest| std::path::PathBuf::from(&manifest.settings.entry_point));

        let target = if cfg!(target_os = "windows") {
            visual_novel_engine::ExportTargetPlatform::Windows
        } else if cfg!(target_os = "macos") {
            visual_novel_engine::ExportTargetPlatform::Macos
        } else {
            visual_novel_engine::ExportTargetPlatform::Linux
        };

        match visual_novel_engine::export_bundle(visual_novel_engine::ExportBundleSpec {
            project_root,
            output_root,
            target_platform: target,
            entry_script,
            runtime_artifact: None,
            integrity: visual_novel_engine::BundleIntegrity::None,
            output_layout_version: 1,
            hmac_key: None,
        }) {
            Ok(report) => {
                self.toast = Some(ToastState::success(format!(
                    "Bundle packaged: assets={} launcher={}",
                    report.assets_copied, report.launcher
                )));
            }
            Err(err) => {
                self.toast = Some(ToastState::error(format!("Package failed: {err}")));
            }
        }
    }

    pub fn export_dry_run_repro(&mut self) {
        let result = self.compile_current_graph();
        let repro = result.minimal_repro_script();
        self.apply_compilation_state(
            &result.script,
            &result.dry_run_report,
            &result.issues,
            &result.phase_trace,
            ValidationPanelMode::OpenIfIssues,
        );

        let Some(repro) = repro else {
            self.toast = Some(ToastState::warning(
                "No se pudo generar un repro fiel para el Dry Run actual",
            ));
            return;
        };

        let Ok(payload) = repro.to_json() else {
            self.toast = Some(ToastState::error("Failed to serialize dry-run repro"));
            return;
        };

        let path = rfd::FileDialog::new()
            .add_filter("Script JSON", &["json"])
            .set_file_name("dry_run_repro.json")
            .save_file();

        if let Some(path) = path {
            match std::fs::write(&path, payload) {
                Ok(_) => {
                    self.toast = Some(ToastState::success("Dry-run repro exported"));
                }
                Err(e) => {
                    self.toast = Some(ToastState::error(format!("Repro export failed: {}", e)));
                }
            }
        } else {
            self.toast = Some(ToastState::warning("Repro export cancelled"));
        }
    }

    pub fn sync_graph_to_script(&mut self) -> Result<(), String> {
        let result = self.compile_current_graph();

        self.apply_compilation_state(
            &result.script,
            &result.dry_run_report,
            &result.issues,
            &result.phase_trace,
            ValidationPanelMode::PreserveManualCloseUnlessErrors,
        );

        match result.engine_result {
            Ok(engine) => {
                self.engine = Some(engine);
                self.refresh_scene_from_engine_preview();
                Ok(())
            }
            Err(e) => {
                self.validation_issues.push(LintIssue::error(
                    None,
                    ValidationPhase::Runtime,
                    LintCode::RuntimeInitError,
                    format!("Engine Error: {}", e),
                ));
                self.show_validation = true;
                Err(e)
            }
        }
    }
}
