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
            if ui.button("Guardar").clicked() {
                self.prepare_save_confirmation();
            }
            if ui.button("Exportar .vnproject").clicked() {
                self.export_compiled_project();
            }
            if ui.button("Empaquetar Bundle").clicked() {
                self.package_bundle_native();
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
                    self.toast = Some(ToastState::success(format!(
                        "Auto-fix batch applied: {} applied, {} skipped",
                        result.applied, result.skipped
                    )));
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
}
