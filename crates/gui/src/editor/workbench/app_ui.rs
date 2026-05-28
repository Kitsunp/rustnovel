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
