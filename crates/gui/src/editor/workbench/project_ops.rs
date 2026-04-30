use super::*;

impl EditorWorkbench {
    pub fn load_project(&mut self, path: std::path::PathBuf) {
        let _ = self.load_project_with_status(path, true);
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
                self.project_root = Some(project_root.clone());
                self.manifest_path = Some(path.clone());
                self.composer_image_cache.clear();
                self.composer_image_failures.clear();
                self.player_audio_backend = None;
                self.player_audio_root = None;
                self.localization_catalog =
                    Self::load_localization_catalog(&project_root, &loaded_project.manifest);
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
                if self.project_root.is_none() {
                    self.project_root = path.parent().map(std::path::Path::to_path_buf);
                }
                self.manifest_path = None;
                if let Some(root) = &self.project_root {
                    self.localization_catalog = Self::discover_locales_without_manifest(root);
                    if self.player_locale.trim().is_empty() {
                        self.player_locale = self.localization_catalog.default_locale.clone();
                    }
                }
                self.apply_loaded_script(loaded_script, path, true);
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
        let mut stack = UndoStack::new();
        stack.push(self.node_graph.clone());
        self.undo_stack = stack;
        self.pending_save_path = Some(path);
        self.saved_script_snapshot = Some(self.node_graph.to_script());
        self.composer_entity_owners.clear();
        self.composer_image_cache.clear();
        self.composer_image_failures.clear();
        self.composer_layer_overrides.clear();
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
    ) -> LocalizationCatalog {
        let mut catalog = LocalizationCatalog::new(manifest.settings.default_language.clone());
        let locale_root = project_root.join("locales");
        for locale in &manifest.settings.supported_languages {
            let requested = std::path::PathBuf::from(format!("{locale}.json"));
            let Ok(Some(path)) =
                crate::editor::project_io::resolve_existing_project_path(&locale_root, &requested)
            else {
                continue;
            };
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(parsed) =
                serde_json::from_str::<std::collections::BTreeMap<String, String>>(&raw)
            else {
                continue;
            };
            catalog.insert_locale_table(locale.clone(), parsed);
        }
        catalog
    }

    fn discover_locales_without_manifest(project_root: &std::path::Path) -> LocalizationCatalog {
        let mut catalog = LocalizationCatalog::default();
        let locale_dir = project_root.join("locales");
        if !locale_dir.exists() {
            return catalog;
        }

        let Ok(entries) = std::fs::read_dir(&locale_dir) else {
            return catalog;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            let Ok(Some(path)) =
                crate::editor::project_io::resolve_existing_project_path(&locale_dir, &path)
            else {
                continue;
            };
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(parsed) =
                serde_json::from_str::<std::collections::BTreeMap<String, String>>(&raw)
            else {
                continue;
            };
            catalog.insert_locale_table(stem.to_string(), parsed);
        }

        if let Some(first) = catalog.locale_codes().first() {
            catalog.default_locale = first.clone();
        }
        catalog
    }

    pub fn execute_save(&mut self, path: &std::path::Path, _content_unused: &str) {
        if let Err(e) = crate::editor::project_io::save_script(path, &self.node_graph) {
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
