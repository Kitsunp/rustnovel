use super::*;

impl EditorWorkbench {
    pub fn load_project(&mut self, path: std::path::PathBuf) {
        if let Err(err) = self.load_project_with_status(path, true) {
            self.toast = Some(ToastState::error(format!("Project load failed: {err}")));
        }
    }

    pub fn load_project_with_status(
        &mut self,
        path: std::path::PathBuf,
        show_toasts: bool,
    ) -> Result<(), String> {
        match crate::editor::project_io::load_project(path.clone()) {
            Ok(loaded_project) => {
                let migrated_manifest = loaded_project
                    .manifest_migration_report
                    .as_ref()
                    .map(|report| report.entries.len());
                let project_root = path
                    .parent()
                    .map(std::path::Path::to_path_buf)
                    .unwrap_or(path.clone());
                let localization_catalog =
                    Self::load_localization_catalog(&project_root, &loaded_project.manifest)?;
                self.project_root = Some(project_root.clone());
                self.manifest_path = Some(path.clone());
                self.composer_image_cache.clear();
                self.composer_image_failures.clear();
                self.resource_service.clear();
                self.composer_layer_overrides.clear();
                self.composer_background_fit_overrides.clear();
                self.rebuild_authoring_session_from_fields();
                self.player_audio_backend = None;
                self.player_audio_root = None;
                self.localization_catalog = localization_catalog;
                self.player_locale = loaded_project.manifest.settings.default_language.clone();
                self.manifest = Some(loaded_project.manifest);
                if let Some((script_path, loaded_script)) = loaded_project.entry_point_script {
                    self.apply_loaded_script(loaded_script, script_path, show_toasts);
                    if show_toasts {
                        if let Some(steps) = migrated_manifest {
                            self.toast = Some(crate::editor::node_types::ToastState::warning(
                                format!("Project loaded with manifest migration ({steps} step(s))"),
                            ));
                        }
                    }
                } else if show_toasts {
                    self.toast = Some(if let Some(steps) = migrated_manifest {
                        crate::editor::node_types::ToastState::warning(format!(
                            "Project loaded without entry script (manifest migrated in {steps} step(s))"
                        ))
                    } else {
                        crate::editor::node_types::ToastState::success(
                            "Project loaded (No entry script)",
                        )
                    });
                }
                Ok(())
            }
            Err(e) => {
                let msg = format!("Failed to load project: {}", e);
                if show_toasts {
                    self.toast = Some(crate::editor::node_types::ToastState::error(msg.clone()));
                }
                tracing::error!("{}", msg);
                Err(msg)
            }
        }
    }

    pub fn load_script(&mut self, path: std::path::PathBuf) {
        match crate::editor::project_io::load_script(path.clone()) {
            Ok(loaded_script) => {
                self.project_root = path.parent().map(std::path::Path::to_path_buf);
                self.manifest_path = None;
                self.manifest = None;
                let mut locale_warnings = Vec::new();
                if let Some(root) = &self.project_root {
                    let (catalog, warnings) = Self::discover_locales_without_manifest(root);
                    self.localization_catalog = catalog;
                    self.player_locale = self.localization_catalog.default_locale.clone();
                    locale_warnings = warnings;
                }
                self.apply_loaded_script(loaded_script, path, true);
                if !locale_warnings.is_empty() {
                    self.toast = Some(crate::editor::node_types::ToastState::warning(format!(
                        "Locale discovery skipped {} file(s): {}",
                        locale_warnings.len(),
                        locale_warnings.join("; ")
                    )));
                }
            }
            Err(e) => {
                self.toast = Some(crate::editor::node_types::ToastState::error(format!(
                    "Failed to load script: {}",
                    e
                )));
                tracing::error!("Failed to load script: {}", e);
            }
        }
    }

    fn apply_loaded_script(
        &mut self,
        loaded_script: crate::editor::project_io::LoadedScript,
        path: std::path::PathBuf,
        show_toast: bool,
    ) {
        self.node_graph = loaded_script.graph;
        self.operation_log = loaded_script.operation_log;
        self.verification_runs = loaded_script.verification_runs;
        self.composer_layer_overrides = loaded_script.composer_layer_overrides;
        self.composer_background_fit_overrides = loaded_script.composer_background_fit_overrides;
        self.rebuild_authoring_session_from_fields();
        let mut stack = UndoStack::new();
        stack.push(self.node_graph.clone());
        self.undo_stack = stack;
        self.pending_save_path = Some(path);
        self.saved_script_snapshot = Some(self.node_graph.to_script());
        self.composer_entity_owners.clear();
        self.composer_image_cache.clear();
        self.composer_image_failures.clear();
        self.resource_service.clear();
        self.player_audio_backend = None;
        self.player_audio_root = None;

        let msg = if loaded_script.was_imported {
            "Imported script"
        } else {
            "Script loaded"
        };
        if show_toast {
            self.toast = Some(ToastState::success(msg));
        }

        // CRITICAL: Sync to engine
        if let Err(err) = self.sync_graph_to_script() {
            if show_toast {
                self.toast = Some(ToastState::error(format!(
                    "Project loaded but player initialization failed: {err}"
                )));
            }
        }
        self.refresh_operation_fingerprint();
    }

    fn load_localization_catalog(
        project_root: &std::path::Path,
        manifest: &visual_novel_engine::manifest::ProjectManifest,
    ) -> Result<LocalizationCatalog, String> {
        let mut catalog = LocalizationCatalog::new(manifest.settings.default_language.clone());
        let locale_root = project_root.join("locales");
        match std::fs::symlink_metadata(&locale_root) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            Ok(_) => {
                return Err(format!(
                    "locale root '{}' is not a regular directory",
                    locale_root.display()
                ));
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(catalog),
            Err(err) => {
                return Err(format!(
                    "inspect locale root '{}': {err}",
                    locale_root.display()
                ));
            }
        }
        let canonical_project_root = project_root.canonicalize().map_err(|err| {
            format!(
                "canonicalize project root '{}': {err}",
                project_root.display()
            )
        })?;
        let canonical_locale_root = locale_root.canonicalize().map_err(|err| {
            format!(
                "canonicalize locale root '{}': {err}",
                locale_root.display()
            )
        })?;
        if !canonical_locale_root.starts_with(&canonical_project_root) {
            return Err(format!(
                "locale root '{}' escapes project root",
                locale_root.display()
            ));
        }
        for locale in &manifest.settings.supported_languages {
            let requested = std::path::PathBuf::from(format!("{locale}.json"));
            let path = match crate::editor::project_io::resolve_existing_project_path(
                &locale_root,
                &requested,
            ) {
                Ok(Some(path)) => path,
                Ok(None) => continue,
                Err(err) => {
                    return Err(format!(
                        "locale '{locale}' path '{}' is invalid: {err}",
                        requested.display()
                    ));
                }
            };
            let raw = std::fs::read_to_string(&path)
                .map_err(|err| format!("read locale '{locale}' '{}': {err}", path.display()))?;
            let parsed = serde_json::from_str::<std::collections::BTreeMap<String, String>>(&raw)
                .map_err(|err| {
                format!("parse locale '{locale}' '{}': {err}", path.display())
            })?;
            catalog.insert_locale_table(locale.clone(), parsed);
        }
        Ok(catalog)
    }

    fn discover_locales_without_manifest(
        project_root: &std::path::Path,
    ) -> (LocalizationCatalog, Vec<String>) {
        let mut catalog = LocalizationCatalog::default();
        let mut warnings = Vec::new();
        let locale_dir = project_root.join("locales");
        match std::fs::symlink_metadata(&locale_dir) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            Ok(_) => {
                warnings.push(format!(
                    "locale directory '{}' is not a regular directory",
                    locale_dir.display()
                ));
                return (catalog, warnings);
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return (catalog, warnings),
            Err(err) => {
                warnings.push(format!(
                    "inspect locale directory '{}': {err}",
                    locale_dir.display()
                ));
                return (catalog, warnings);
            }
        }
        let canonical_project_root = match project_root.canonicalize() {
            Ok(root) => root,
            Err(err) => {
                warnings.push(format!(
                    "canonicalize project root '{}': {err}",
                    project_root.display()
                ));
                return (catalog, warnings);
            }
        };
        let canonical_locale_dir = match locale_dir.canonicalize() {
            Ok(root) => root,
            Err(err) => {
                warnings.push(format!(
                    "canonicalize locale directory '{}': {err}",
                    locale_dir.display()
                ));
                return (catalog, warnings);
            }
        };
        if !canonical_locale_dir.starts_with(&canonical_project_root) {
            warnings.push(format!(
                "locale directory '{}' escapes project root",
                locale_dir.display()
            ));
            return (catalog, warnings);
        }

        let entries = match std::fs::read_dir(&locale_dir) {
            Ok(entries) => entries,
            Err(err) => {
                warnings.push(format!(
                    "read locale directory '{}': {err}",
                    locale_dir.display()
                ));
                return (catalog, warnings);
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    warnings.push(format!("read locale directory entry: {err}"));
                    continue;
                }
            };
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(file_name) = path.file_name() else {
                continue;
            };
            let requested = std::path::PathBuf::from(file_name);
            let path = match crate::editor::project_io::resolve_existing_project_path(
                &locale_dir,
                &requested,
            ) {
                Ok(Some(path)) => path,
                Ok(None) => continue,
                Err(err) => {
                    warnings.push(format!(
                        "locale '{}' path '{}' is invalid: {err}",
                        stem,
                        requested.display()
                    ));
                    continue;
                }
            };
            let raw = match std::fs::read_to_string(&path) {
                Ok(raw) => raw,
                Err(err) => {
                    warnings.push(format!(
                        "read locale '{}' '{}': {err}",
                        stem,
                        path.display()
                    ));
                    continue;
                }
            };
            let parsed =
                match serde_json::from_str::<std::collections::BTreeMap<String, String>>(&raw) {
                    Ok(parsed) => parsed,
                    Err(err) => {
                        warnings.push(format!(
                            "parse locale '{}' '{}': {err}",
                            stem,
                            path.display()
                        ));
                        continue;
                    }
                };
            catalog.insert_locale_table(stem.to_string(), parsed);
        }

        if let Some(first) = catalog.locale_codes().first() {
            catalog.default_locale = first.clone();
        }
        (catalog, warnings)
    }

    pub fn execute_save(&mut self, path: &std::path::Path, _content_unused: &str) {
        if let Err(e) = crate::editor::project_io::save_authoring_document_with_metadata(
            path,
            &self.node_graph,
            &self.composer_layer_overrides,
            &self.composer_background_fit_overrides,
            &self.operation_log,
            &self.verification_runs,
        ) {
            tracing::error!("Failed to save: {}", e);
            self.toast = Some(ToastState::error(format!("Save failed: {}", e)));
        } else {
            self.saved_script_snapshot = Some(self.node_graph.to_script());
            self.node_graph.clear_modified();
        }
    }

    pub fn prepare_save_confirmation(&mut self) {
        let maybe_path = self.pending_save_path.clone().or_else(|| {
            rfd::FileDialog::new()
                .add_filter("Authoring Project", &["vnauthoring", "vnproject"])
                .add_filter("Legacy Script JSON", &["json"])
                .set_file_name("game.vnauthoring")
                .save_file()
        });

        if let Some(path) = maybe_path {
            self.pending_save_path = Some(path);
            let new_script = self.node_graph.to_script();
            self.show_save_confirm = true;
            self.diff_dialog = Some(DiffDialog::new(
                self.saved_script_snapshot.as_ref(),
                &new_script,
            ));
        } else {
            self.toast = Some(ToastState::warning("Save cancelled"));
        }
    }
}
