use super::*;

impl VisualComposerPanel<'_> {
    pub(super) fn render_metadata_row(
        &self,
        ui: &mut egui::Ui,
        entity_owners: &HashMap<u32, u32>,
        chrome: ComposerChromeLayout,
    ) {
        ui.horizontal_wrapped(|ui| {
            let (w, h) = self.stage_size();
            let stage_label = if chrome.show_full_labels {
                format!("Stage: {}x{}", w as u32, h as u32)
            } else {
                format!("{}x{}", w as u32, h as u32)
            };
            ui.label(stage_label);
            ui.separator();
            ui.label(format!("Entities: {}", self.scene.len()));
            ui.separator();
            let source = preview_source_label(
                self.scene,
                self.engine,
                *self.preview_mode,
                self.selected_authoring_node_id,
                self.selected_authoring_node,
                entity_owners,
            );
            ui.add(
                egui::Label::new(crate::player_overlay::soft_wrap_long_tokens(
                    source,
                    chrome.source_wrap_chars,
                ))
                .wrap(true),
            );
        });
    }

    pub(super) fn render_control_rows(
        &mut self,
        ui: &mut egui::Ui,
        chrome: ComposerChromeLayout,
        action: &mut Option<VisualComposerAction>,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            self.render_quality_view_background_controls(ui, chrome, action);
            if chrome.mode == ComposerChromeMode::Full {
                ui.separator();
                self.render_preview_run_controls(ui, chrome, action);
            }
        });
        if chrome.mode != ComposerChromeMode::Full {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                self.render_preview_run_controls(ui, chrome, action);
            });
        }
    }

    fn render_quality_view_background_controls(
        &mut self,
        ui: &mut egui::Ui,
        chrome: ComposerChromeLayout,
        action: &mut Option<VisualComposerAction>,
    ) {
        ui.label(if chrome.show_full_labels {
            "Pixels:"
        } else {
            "Px"
        });
        egui::ComboBox::from_id_source("composer_preview_quality")
            .width(chrome.quality_width)
            .selected_text(preview_quality_label(*self.preview_quality, chrome.mode))
            .show_ui(ui, |ui| {
                for quality in PreviewQuality::ALL {
                    ui.selectable_value(self.preview_quality, *quality, quality.label());
                }
            });
        ui.label(if chrome.show_full_labels {
            "View:"
        } else {
            "View"
        });
        egui::ComboBox::from_id_source("composer_stage_fit")
            .width(chrome.fit_width)
            .selected_text(self.stage_fit.label())
            .show_ui(ui, |ui| {
                for fit in StageFit::ALL {
                    ui.selectable_value(self.stage_fit, *fit, fit.label());
                }
            });
        let old_background_fit = *self.background_fit;
        ui.label("BG");
        egui::ComboBox::from_id_source("composer_background_fit")
            .width(chrome.fit_width)
            .selected_text(self.background_fit.label())
            .show_ui(ui, |ui| {
                for fit in BackgroundFit::ALL {
                    ui.selectable_value(self.background_fit, *fit, fit.label());
                }
            });
        if old_background_fit != *self.background_fit {
            *action = Some(VisualComposerAction::BackgroundFitChanged {
                node_id: self.selected_authoring_node_id,
                fit: *self.background_fit,
            });
        }
    }

    fn render_preview_run_controls(
        &mut self,
        ui: &mut egui::Ui,
        chrome: ComposerChromeLayout,
        action: &mut Option<VisualComposerAction>,
    ) {
        ui.label(if chrome.show_full_labels {
            "Preview:"
        } else {
            "Preview"
        });
        let old_preview_mode = *self.preview_mode;
        egui::ComboBox::from_id_source("composer_preview_mode")
            .width(chrome.preview_width)
            .selected_text(preview_mode_label(*self.preview_mode, chrome.mode))
            .show_ui(ui, |ui| {
                for mode in ComposerPreviewMode::ALL {
                    ui.selectable_value(self.preview_mode, *mode, mode.label());
                }
            });
        if old_preview_mode != *self.preview_mode {
            *action = Some(VisualComposerAction::PreviewModeChanged(*self.preview_mode));
        }
        if ui.small_button("Test here").clicked() {
            *action = Some(VisualComposerAction::TestFromSelection);
        }
        if ui.small_button("Restart").clicked() {
            *action = Some(VisualComposerAction::TestRestart);
        }
        overlays::render_runtime_controls(ui, self.engine, action);
    }

    pub(super) fn render_layer_panel(
        &mut self,
        ui: &mut egui::Ui,
        objects: &[LayeredSceneObject],
        max_height: f32,
    ) -> Option<VisualComposerAction> {
        let mut action = None;
        egui::CollapsingHeader::new("Layers")
            .default_open(true)
            .show(ui, |ui| {
                if objects.is_empty() {
                    ui.label("No layers");
                    return;
                }
                egui::ScrollArea::vertical()
                    .id_source("visual_composer_layers_scroll")
                    .max_height(max_height)
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width().max(1.0));
                        for object in objects.iter().rev() {
                            let entry = self
                                .layer_overrides
                                .get(&object.object_id)
                                .copied()
                                .unwrap_or(LayerOverride {
                                    visible: object.visible,
                                    locked: object.locked,
                                });
                            ui.horizontal_wrapped(|ui| {
                                let mut visible = entry.visible;
                                if ui.checkbox(&mut visible, "").changed() {
                                    action = Some(VisualComposerAction::LayerVisibilityChanged {
                                        object_id: object.object_id.clone(),
                                        visible,
                                    });
                                }
                                let mut locked = entry.locked;
                                if ui.checkbox(&mut locked, "Lock").changed() {
                                    action = Some(VisualComposerAction::LayerLockChanged {
                                        object_id: object.object_id.clone(),
                                        locked,
                                    });
                                }
                                if object.source_node_id == self.active_event_node_id {
                                    ui.label(
                                        egui::RichText::new("active")
                                            .color(egui::Color32::from_rgb(120, 220, 255)),
                                    );
                                }
                                let source_label = format!(
                                    "{} | z={} | {}",
                                    object.kind.label(),
                                    object.z_index,
                                    object.source_field_path
                                );
                                let label_width = ui.available_width().max(1.0);
                                ui.add_sized(
                                    [label_width, ui.spacing().interact_size.y],
                                    egui::Label::new(crate::player_overlay::soft_wrap_long_tokens(
                                        &source_label,
                                        28,
                                    ))
                                    .wrap(true),
                                );
                            });
                        }
                    });
            });
        action
    }
}
