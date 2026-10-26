use super::*;

impl EditorWorkbench {
    pub(in crate::editor::workbench) fn default_export_entry(&self) -> Option<std::path::PathBuf> {
        if let Some(manifest) = &self.manifest {
            return Some(manifest.settings.entry_point.clone().into());
        }
        let current = self.pending_save_path.as_deref()?;
        let root = self.project_root.as_deref().or_else(|| current.parent())?;
        current
            .strip_prefix(root)
            .ok()
            .map(std::path::Path::to_path_buf)
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
            self.default_export_entry()
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
        // Script-only bundles must not accidentally include an old executable.
        let runtime_artifact = (self.export_wizard.export_kind == ExportWizardKind::ExecutableGame)
            .then_some(runtime_artifact)
            .flatten();
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

    pub(super) fn plan_export_wizard(&mut self) {
        self.export_wizard.last_error = None;
        self.export_wizard.last_report = None;
        self.export_wizard.last_plan = None;
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
        if let Err(error) = self.prepare_export_document(&spec, false) {
            self.export_wizard.last_error = Some(error.clone());
            self.toast = Some(ToastState::error(error));
            return;
        }
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

    pub(super) fn execute_export_wizard(&mut self) {
        self.export_wizard.last_error = None;
        self.export_wizard.last_plan = None;
        self.export_wizard.last_report = None;
        self.export_wizard.require_executable =
            self.export_wizard.export_kind == ExportWizardKind::ExecutableGame;
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
        if let Err(error) = self.prepare_export_document(&spec, !self.export_wizard.dry_run) {
            self.export_wizard.last_error = Some(error.clone());
            self.toast = Some(ToastState::error(error));
            return;
        }
        match visual_novel_engine::ExportService::new().plan_export(&spec) {
            Ok(plan) => {
                let service = visual_novel_engine::ExportService::new();
                let validation = if self.export_wizard.require_executable {
                    service.validate_executable_export_plan(&plan, spec.target_platform)
                } else {
                    service.validate_export_plan(&plan)
                };
                if let Err(error) = validation {
                    self.export_wizard.last_plan = Some(plan);
                    self.export_wizard.last_error = Some(error.to_string());
                    self.toast = Some(ToastState::error(
                        "Export blocked; see the diagnostics below",
                    ));
                    return;
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

    /// Export the currently edited entry only after accounting for its pending
    /// changes. Other explicitly selected entries remain independent documents.
    pub(in crate::editor::workbench) fn prepare_export_document(
        &mut self,
        spec: &visual_novel_engine::ExportBundleSpec,
        save: bool,
    ) -> Result<(), String> {
        let Some(current) = self.pending_save_path.clone() else {
            return Ok(());
        };
        let entry = spec
            .entry_script
            .clone()
            .or_else(|| self.default_export_entry());
        let Some(entry) = entry else {
            return Ok(());
        };
        let target = spec.project_root.join(entry);
        let same_entry = target == current
            || target
                .canonicalize()
                .ok()
                .zip(current.canonicalize().ok())
                .is_some_and(|(a, b)| a == b);
        if !same_entry {
            return Ok(());
        }
        if self.node_graph.is_empty() {
            return Err("The current document is empty".into());
        }
        let stored = visual_novel_engine::authoring::load_authoring_entry(&current).ok();
        let has_saved_layout = std::fs::read_to_string(&current)
            .ok()
            .is_some_and(|source| {
                visual_novel_engine::authoring::source_looks_like_authoring_document(&source)
            });
        let pending_drag = self.node_graph.dragging_node.is_some();
        let changed = pending_drag
            || (!has_saved_layout && !self.node_graph.authoring_session.dirty_flags().is_clean())
            || stored.as_ref().is_none_or(|document| {
                (if has_saved_layout {
                    !document
                        .graph
                        .has_same_content(self.node_graph.authoring_graph())
                } else {
                    // Import-time automatic layout is not an unsaved story edit.
                    serde_json::to_value(document.graph.to_script()).ok()
                        != serde_json::to_value(self.node_graph.to_script()).ok()
                }) || document.composer_layer_overrides != self.document().composer_layer_overrides
                    || document.composer_background_fit_overrides
                        != self.document().composer_background_fit_overrides
            });
        if !changed {
            return Ok(());
        }
        if !save {
            return Err("There are unsaved changes. Save the document to check this export, or use Save and export with Dry-run disabled.".into());
        }
        self.node_graph
            .authoring_graph()
            .to_script_strict()
            .map_err(|error| format!("Cannot export the current document: {error}"))?;
        self.save_document(&current)
            .map_err(|error| format!("Could not save before export: {error}"))?;
        Ok(())
    }
}
