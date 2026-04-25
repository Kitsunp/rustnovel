use crate::editor::EditorWorkbench;
use eframe::egui;

pub fn render_menu_bar(ui: &mut egui::Ui, workbench: &mut EditorWorkbench) {
    egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Open Project...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("VN Manifest", &["vnm"])
                    .add_filter("Legacy Manifest", &["toml"])
                    .pick_file()
                {
                    workbench.load_project(path);
                    ui.close_menu();
                }
            }
            if ui.button("Import Ren'Py Project...").clicked() {
                workbench.import_renpy_project_native();
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Import Background Image...").clicked() {
                workbench.import_asset_dialog(crate::editor::AssetImportKind::Background);
                ui.close_menu();
            }
            if ui.button("Import Character Image...").clicked() {
                workbench.import_asset_dialog(crate::editor::AssetImportKind::Character);
                ui.close_menu();
            }
            if ui.button("Import Audio...").clicked() {
                workbench.import_asset_dialog(crate::editor::AssetImportKind::Audio);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Save").clicked() {
                workbench.prepare_save_confirmation();
                ui.close_menu();
            }
            if ui.button("Export Game (.vnproject)").clicked() {
                workbench.export_compiled_project();
                ui.close_menu();
            }
            if ui.button("Package Bundle...").clicked() {
                workbench.package_bundle_native();
                ui.close_menu();
            }
        });
        ui.menu_button("Tools", |ui| {
            if ui.button("Validate / Dry Run").clicked() {
                workbench.run_dry_validation();
                ui.close_menu();
            }
            if ui.button("Compile Preview").clicked() {
                workbench.compile_preview();
                ui.close_menu();
            }
            if ui.button("Export Dry Run Repro").clicked() {
                workbench.export_dry_run_repro();
                ui.close_menu();
            }
            if ui.button("Export Repro Case").clicked() {
                workbench.export_repro_case();
                ui.close_menu();
            }
            if ui.button("Import Repro Case").clicked() {
                workbench.import_repro_case();
                ui.close_menu();
            }
            if ui.button("Run Loaded Repro Case").clicked() {
                workbench.run_loaded_repro_case();
                ui.close_menu();
            }
            if ui.button("Export Diagnostic Report").clicked() {
                workbench.export_diagnostic_report();
                ui.close_menu();
            }
            if ui.button("Import Diagnostic Report").clicked() {
                workbench.import_diagnostic_report();
                ui.close_menu();
            }
            if ui.button("Auto-fix Complete (review)").clicked() {
                match workbench.prepare_autofix_batch_confirmation(true) {
                    Ok(planned) => {
                        workbench.toast = Some(crate::editor::ToastState::warning(format!(
                            "Review horizontal diff and confirm auto-fix batch ({planned} planned)"
                        )));
                    }
                    Err(err) => {
                        workbench.toast = Some(crate::editor::ToastState::warning(format!(
                            "Auto-fix batch not prepared: {err}"
                        )));
                    }
                }
                ui.close_menu();
            }
        });
        ui.menu_button("View", |ui| {
            ui.checkbox(&mut workbench.show_graph, "Graph Panel");
            ui.checkbox(&mut workbench.show_inspector, "Inspector");
            ui.checkbox(&mut workbench.show_timeline, "Timeline");
            ui.checkbox(&mut workbench.show_asset_browser, "Asset Browser");
            ui.checkbox(&mut workbench.show_validation, "Validation Report");
            if workbench.show_validation {
                ui.checkbox(&mut workbench.validation_collapsed, "Validation Minimizado");
            }
            ui.separator();
            ui.collapsing("Layout Sizes", |ui| {
                let mut changed = false;
                changed |= layout_slider(
                    ui,
                    "Assets",
                    &mut workbench.layout_overrides.asset_width,
                    80.0..=420.0,
                );
                changed |= layout_slider(
                    ui,
                    "Graph",
                    &mut workbench.layout_overrides.graph_width,
                    150.0..=760.0,
                );
                changed |= layout_slider(
                    ui,
                    "Inspector",
                    &mut workbench.layout_overrides.inspector_width,
                    150.0..=520.0,
                );
                changed |= layout_slider(
                    ui,
                    "Errors",
                    &mut workbench.layout_overrides.validation_height,
                    80.0..=640.0,
                );
                changed |= layout_slider(
                    ui,
                    "Timeline",
                    &mut workbench.layout_overrides.timeline_height,
                    80.0..=420.0,
                );
                if changed {
                    workbench.apply_layout_size_overrides();
                }
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        workbench.apply_layout_size_overrides();
                    }
                    if ui.button("Clear").clicked() {
                        workbench.clear_layout_size_overrides();
                    }
                });
            });
            ui.separator();
            ui.checkbox(
                &mut workbench.node_editor_window_open,
                "Floating Node Editor",
            );
        });
    });
}

fn layout_slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<f32>,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    let mut enabled = value.is_some();
    let mut changed = ui.checkbox(&mut enabled, label).changed();
    if enabled && value.is_none() {
        *value = Some(*range.start());
        changed = true;
    }
    if !enabled {
        if value.take().is_some() {
            changed = true;
        }
        return changed;
    }
    if let Some(current) = value {
        changed |= ui
            .add(egui::Slider::new(current, range).show_value(true))
            .changed();
    }
    changed
}
