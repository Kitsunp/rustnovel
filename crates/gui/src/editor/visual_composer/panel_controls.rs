use super::*;

impl VisualComposerPanel<'_> {
    pub(super) fn render_selected_transform(
        &self,
        ui: &mut egui::Ui,
        entity_owners: &HashMap<u32, u32>,
        action: &mut Option<VisualComposerAction>,
    ) {
        let Some(raw_id) = *self.selected_entity_id else {
            return;
        };
        let Some((index, entity)) = self
            .scene
            .iter_sorted()
            .enumerate()
            .find(|(_, entity)| entity.id.raw() == raw_id)
        else {
            return;
        };
        let visual_novel_engine::EntityKind::Character(character) = &entity.kind else {
            return;
        };
        let Some(node_id) = entity_owners.get(&raw_id).copied() else {
            return;
        };
        let object_id = self
            .entity_object_ids
            .get(&raw_id)
            .cloned()
            .unwrap_or_else(|| scene_entity_object_id(entity, Some(node_id), index));
        let locked = self
            .layer_overrides
            .get(&object_id)
            .is_some_and(|entry| entry.locked);
        let (mut x, mut y) = (entity.transform.x, entity.transform.y);
        let mut scale = entity.transform.scale as f32 / 1000.0;
        let mut changed = false;
        ui.add_enabled_ui(!locked, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(character.name.as_ref());
                changed |= ui.add(egui::DragValue::new(&mut x).prefix("X ")).changed();
                changed |= ui.add(egui::DragValue::new(&mut y).prefix("Y ")).changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut scale)
                            .prefix("Scale ")
                            .speed(0.01)
                            .range(0.1..=4.0),
                    )
                    .changed();
            })
        });
        if changed {
            *action = Some(VisualComposerAction::MutateNode {
                node_id,
                mutation: ComposerNodeMutation::CharacterPosition {
                    name: character.name.to_string(),
                    expression: character.expression.as_ref().map(ToString::to_string),
                    source_instance_index: crate::editor::scene_stage::character_instance_index(
                        self.scene,
                        entity_owners,
                        raw_id,
                        node_id,
                        character.name.as_ref(),
                        character.expression.as_deref(),
                    ),
                    x,
                    y,
                    scale: (scale != 1.0).then_some(scale),
                },
            });
        }
    }

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
                .wrap(),
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
        if chrome.mode == ComposerChromeMode::Narrow {
            ui.menu_button("Test", |ui| self.render_test_controls(ui, action));
        } else {
            self.render_test_controls(ui, action);
        }
        ui.menu_button("Audio", |ui| {
            if ui.button("Play scene audio").clicked() {
                *action = Some(VisualComposerAction::PreviewAudio);
                ui.close_menu();
            }
            if ui.button("Stop audio").clicked() {
                *action = Some(VisualComposerAction::StopAudio);
                ui.close_menu();
            }
        });
    }

    fn render_test_controls(&self, ui: &mut egui::Ui, action: &mut Option<VisualComposerAction>) {
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
                                let selected = object.source_node_id.is_some()
                                    && object.source_node_id == self.selected_authoring_node_id;
                                let text = egui::RichText::new(
                                    crate::player_overlay::soft_wrap_long_tokens(&source_label, 28),
                                )
                                .background_color(if selected {
                                    ui.visuals().selection.bg_fill
                                } else {
                                    egui::Color32::TRANSPARENT
                                });
                                let response = ui
                                    .add_sized(
                                        [label_width, ui.spacing().interact_size.y],
                                        egui::Label::new(text).wrap().sense(egui::Sense::click()),
                                    )
                                    .on_hover_text(format!(
                                        "{}\nAsset: {}\nPosition: ({:?}, {:?}) | Scale: {:?}",
                                        object.source_field_path,
                                        object.asset_path.as_deref().unwrap_or("—"),
                                        object.x,
                                        object.y,
                                        object.scale
                                    ));
                                if response.clicked() {
                                    if let Some(entity) =
                                        self.scene.iter_sorted().enumerate().find_map(
                                            |(index, entity)| {
                                                (self
                                                    .entity_object_ids
                                                    .get(&entity.id.raw())
                                                    .cloned()
                                                    .unwrap_or_else(|| {
                                                        scene_entity_object_id(
                                                            entity,
                                                            object.source_node_id,
                                                            index,
                                                        )
                                                    })
                                                    == object.object_id)
                                                    .then_some(entity)
                                            },
                                        )
                                    {
                                        *self.selected_entity_id = Some(entity.id.raw());
                                    }
                                    if let Some(node_id) = object.source_node_id {
                                        action = Some(VisualComposerAction::SelectNode(node_id));
                                    }
                                }
                            });
                        }
                    });
            });
        action
    }
}
