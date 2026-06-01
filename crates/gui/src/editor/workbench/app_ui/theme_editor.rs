use super::*;

impl EditorWorkbench {
    pub(super) fn render_theme_editor_window(&mut self, ctx: &egui::Context) {
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
        let scroll_max_height = (ctx.available_rect().height() - 120.0).max(260.0);
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
                egui::ScrollArea::vertical()
                    .id_source("theme_editor_body_scroll")
                    .auto_shrink([false, false])
                    .max_height(scroll_max_height)
                    .show(ui, |ui| {
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
                                    super::theme_controls::color_code_control(ui, &key, value);
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
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Preview text");
                                ui.text_edit_singleline(&mut self.theme_preview_text);
                            });
                            let keys = theme.typography.keys().cloned().collect::<Vec<_>>();
                            for key in keys {
                                if let Some(token) = theme.typography.get_mut(&key) {
                                    super::theme_controls::typography_token_control(
                                        ui,
                                        &key,
                                        token,
                                        &self.theme_preview_text,
                                    );
                                }
                            }
                        });

                        ui.collapsing("Combined Preview", |ui| {
                            super::theme_controls::render_theme_preview(
                                ui,
                                theme,
                                &self.theme_preview_text,
                            );
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
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 120, 120),
                                "Theme invalid",
                            );
                        }
                        for warning in &validation.warnings {
                            ui.label(format!("warning: {warning}"));
                        }
                        for error in &validation.errors {
                            ui.colored_label(egui::Color32::from_rgb(255, 150, 120), error);
                        }
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

                ui.separator();
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
}
