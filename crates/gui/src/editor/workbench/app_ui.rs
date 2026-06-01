use super::*;
#[path = "app_ui/export_wizard.rs"]
mod export_wizard;
#[path = "app_ui/player_menu_settings.rs"]
mod player_menu_settings;
#[path = "app_ui/theme_controls.rs"]
mod theme_controls;
#[path = "app_ui/theme_editor.rs"]
mod theme_editor;

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
}
