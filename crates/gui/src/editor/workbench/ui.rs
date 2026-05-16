use super::*;

impl EditorWorkbench {
    pub(super) fn render_player_mode(&mut self, ctx: &egui::Context) {
        self.ensure_player_audio_backend();
        let stage_resolution = self
            .manifest
            .as_ref()
            .map(|manifest| manifest.settings.resolution);
        let mut visual_context = crate::editor::player_ui::PlayerVisualContext {
            project_root: self.project_root.as_deref(),
            stage_resolution,
            preview_quality: self.composer_preview_quality,
            stage_fit: crate::editor::StageFit::Fill,
            image_cache: &mut self.composer_image_cache,
            image_failures: &mut self.composer_image_failures,
        };
        let audio_commands = crate::editor::player_ui::render_player_ui(
            &mut self.engine,
            &mut self.toast,
            &mut self.player_state,
            &mut self.player_locale,
            &self.localization_catalog,
            ctx,
            &mut visual_context,
        );
        self.apply_player_audio_commands(audio_commands);
    }

    pub(super) fn render_editor_mode(&mut self, ctx: &egui::Context) {
        self.handle_global_editor_shortcuts(ctx);
        let selected_before = self.selected_node;
        let graph_before_editor_interaction = self.node_graph.clone();
        let layout = super::layout::editor_panel_layout(
            ctx.available_rect().width(),
            ctx.available_rect().height(),
            &self.layout_overrides,
        );

        // 1. Bottom Panels (Validation & Timeline)
        if self.show_validation {
            let mut close_validation = false;
            let mut toggle_validation_collapse = false;
            let validation_layout = super::layout::validation_panel_layout(
                ctx.available_rect().height(),
                self.validation_collapsed,
                &self.layout_overrides,
            );
            egui::TopBottomPanel::bottom(format!("validation_panel_{}", self.layout_generation))
                .resizable(true)
                .default_height(validation_layout.default)
                .min_height(validation_layout.min)
                .max_height(validation_layout.max)
                .show(ctx, |ui| {
                    let error_count = self
                        .validation_issues
                        .iter()
                        .filter(|issue| issue.severity == LintSeverity::Error)
                        .count();
                    let warning_count = self
                        .validation_issues
                        .iter()
                        .filter(|issue| issue.severity == LintSeverity::Warning)
                        .count();
                    let info_count = self
                        .validation_issues
                        .iter()
                        .filter(|issue| issue.severity == LintSeverity::Info)
                        .count();

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "Validation Report  |  E:{} W:{} I:{}",
                                error_count, warning_count, info_count
                            ))
                            .strong(),
                        );
                        ui.separator();
                        let collapse_label = if self.validation_collapsed {
                            "Expandir"
                        } else {
                            "Minimizar"
                        };
                        if ui.button(collapse_label).clicked() {
                            toggle_validation_collapse = true;
                        }
                        if ui.button("Cerrar").clicked() {
                            close_validation = true;
                        }
                    });

                    if self.validation_collapsed {
                        return;
                    }

                    let lint_response = LintPanel::new(
                        &self.validation_issues,
                        &mut self.selected_node,
                        &mut self.selected_issue,
                        &mut self.diagnostic_language,
                        &self.node_graph,
                        self.last_fix_snapshot.is_some(),
                    )
                    .ui(ui);

                    for action in lint_response.actions {
                        match action {
                            crate::editor::lint_panel::LintPanelAction::ApplyFix {
                                issue_index,
                                fix_id,
                                structural,
                            } => {
                                if structural {
                                    match self
                                        .prepare_structural_fix_confirmation(issue_index, &fix_id)
                                    {
                                        Ok(()) => {
                                            self.toast = Some(ToastState::warning(format!(
                                                "Review diff and confirm structural fix '{fix_id}'"
                                            )));
                                        }
                                        Err(err) => {
                                            self.toast = Some(ToastState::error(format!(
                                                "Fix '{fix_id}' preview failed: {err}"
                                            )));
                                        }
                                    }
                                } else {
                                    match self.apply_issue_fix(issue_index, &fix_id) {
                                        Ok(()) => {
                                            self.toast = Some(ToastState::success(format!(
                                                "Applied fix '{fix_id}'"
                                            )));
                                        }
                                        Err(err) => {
                                            self.toast = Some(ToastState::error(format!(
                                                "Fix '{fix_id}' failed: {err}"
                                            )));
                                        }
                                    }
                                }
                            }
                            crate::editor::lint_panel::LintPanelAction::ApplyAllSafeFixes => {
                                let applied = self.apply_all_safe_fixes();
                                if applied > 0 {
                                    self.toast = Some(ToastState::success(format!(
                                        "Applied {applied} safe fix(es)"
                                    )));
                                } else {
                                    self.toast = Some(ToastState::warning(
                                        "No safe fixes available for current diagnostics",
                                    ));
                                }
                            }
                            crate::editor::lint_panel::LintPanelAction::PrepareAutoFixBatch {
                                include_review,
                            } => match self.prepare_autofix_batch_confirmation(include_review) {
                                Ok(planned) => {
                                    self.toast = Some(ToastState::warning(format!(
                                        "Review horizontal diff and confirm auto-fix batch ({planned} planned)"
                                    )));
                                }
                                Err(err) => {
                                    self.toast = Some(ToastState::warning(format!(
                                        "Auto-fix batch not prepared: {err}"
                                    )));
                                }
                            },
                            crate::editor::lint_panel::LintPanelAction::AutoFixIssue {
                                issue_index,
                                include_review,
                            } => match self.apply_best_fix_for_issue(issue_index, include_review) {
                                Ok(outcome) => {
                                    self.toast = Some(ToastState::success(outcome));
                                }
                                Err(err) => {
                                    self.toast = Some(ToastState::error(format!(
                                        "Issue auto-fix failed: {err}"
                                    )));
                                }
                            },
                            crate::editor::lint_panel::LintPanelAction::RevertLastFix => {
                                if self.revert_last_fix() {
                                    self.toast =
                                        Some(ToastState::success("Last fix reverted successfully"));
                                } else {
                                    self.toast = Some(ToastState::warning("No fix to revert"));
                                }
                            }
                        }
                    }
                });
            if toggle_validation_collapse {
                self.validation_collapsed = !self.validation_collapsed;
            }
            if close_validation {
                self.show_validation = false;
                self.validation_collapsed = false;
            }
        }

        if self.show_timeline && !self.show_validation {
            egui::TopBottomPanel::bottom(format!("timeline_panel_{}", self.layout_generation))
                .resizable(true)
                .default_height(layout.timeline.default)
                .min_height(layout.timeline.min)
                .max_height(layout.timeline.max)
                .show(ctx, |ui| {
                    let mut current_time_u32 = self.current_time as u32;
                    let mut is_playing = self.is_playing;

                    TimelinePanel::new(&mut self.timeline, &mut current_time_u32, &mut is_playing)
                        .ui(ui);

                    self.current_time = current_time_u32 as f32;
                    self.is_playing = is_playing;
                });
        }

        // 2. Left Panel (Asset Browser)
        let mut asset_browser_actions = Vec::new();
        if self.show_asset_browser {
            egui::SidePanel::left(format!(
                "asset_browser_panel_{}_{}",
                self.layout_generation, layout.id_suffix
            ))
            .resizable(true)
            .min_width(layout.asset_browser.min)
            .default_width(layout.asset_browser.default)
            .max_width(layout.asset_browser.max)
            .show(ctx, |ui| {
                if let Some(manifest) = &self.manifest {
                    asset_browser_actions.extend(
                        AssetBrowserPanel::new(
                            manifest,
                            self.project_root.as_deref(),
                            &mut self.composer_image_cache,
                            &mut self.composer_image_failures,
                            &mut self.audio_duration_cache,
                        )
                        .ui(ui),
                    );
                } else {
                    ui.label("No project loaded.");
                    if ui.button("Open Project").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("VN Manifest", &["vnm"])
                            .add_filter("Legacy Manifest", &["toml"])
                            .pick_file()
                        {
                            self.load_project(path);
                        }
                    }
                }
            });
        }

        let mut inspector_actions = Vec::new();

        for action in asset_browser_actions {
            match action {
                crate::editor::AssetBrowserAction::Import(kind) => self.import_asset_dialog(kind),
                crate::editor::AssetBrowserAction::Remove { kind, name } => {
                    match self.remove_asset_from_manifest(kind, &name) {
                        Ok(()) => {
                            self.toast = Some(ToastState::success(format!(
                                "{} removed from manifest: {}",
                                kind.label(),
                                name
                            )));
                        }
                        Err(err) => {
                            self.toast = Some(ToastState::error(format!(
                                "{} removal failed: {err}",
                                kind.label()
                            )));
                        }
                    }
                }
                crate::editor::AssetBrowserAction::PreviewAudio { path, offset_ms } => {
                    self.play_editor_audio_preview_from_offset(
                        "bgm",
                        &path,
                        None,
                        true,
                        std::time::Duration::from_millis(offset_ms),
                    );
                }
                crate::editor::AssetBrowserAction::StopAudio => {
                    self.stop_editor_audio_preview("bgm")
                }
            }
        }

        // 3. Docked Graph/Inspector Panels (context-level to avoid nested layout clipping)
        if self.show_inspector {
            egui::SidePanel::right(format!(
                "inspector_docked_panel_{}_{}",
                self.layout_generation, layout.id_suffix
            ))
            .resizable(true)
            .min_width(layout.inspector.min)
            .default_width(layout.inspector.default)
            .max_width(layout.inspector.max)
            .show(ctx, |ui| {
                let selected = self.node_graph.selected;
                if let Some(action) = InspectorPanel::new(
                    &self.scene,
                    &mut self.node_graph,
                    selected,
                    self.selected_entity,
                )
                .ui(ui)
                {
                    inspector_actions.push(action);
                }
            });
        }

        if !self.node_editor_window_open && self.show_graph {
            {
                egui::SidePanel::left(format!(
                    "logic_graph_docked_panel_{}_{}",
                    self.layout_generation, layout.id_suffix
                ))
                .resizable(true)
                .min_width(layout.graph.min)
                .default_width(layout.graph.default)
                .max_width(layout.graph.max)
                .show(ctx, |ui| {
                    ui.heading("Logic Graph");
                    let mut panel =
                        NodeEditorPanel::new(&mut self.node_graph, &mut self.undo_stack);
                    panel.ui(ui);
                });
            }
        }

        // 4. Central Area (Composer + detached inspector logic)

        // Prepare Data for decoupled rendering to avoid simultaneous mutable borrows
        let entity_owners = if self.composer_entity_owners.is_empty() {
            self.build_entity_node_map()
        } else {
            self.composer_entity_owners.clone()
        };
        let active_event_node_id = self.engine.as_ref().and_then(|engine| {
            self.node_graph
                .authoring_graph()
                .node_for_event_ip(engine.state().position)
        });
        let composer_selected_node = self.node_graph.selected.or(self.selected_node);
        let selected_authoring_node =
            composer_selected_node.and_then(|node_id| self.node_graph.get_node(node_id).cloned());
        let mut composer_actions = Vec::new();

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_fragments_panel(ui);
            ui.separator();
            let stage_resolution = self
                .manifest
                .as_ref()
                .map(|manifest| manifest.settings.resolution);
            let mut composer = crate::editor::visual_composer::VisualComposerPanel::new(
                crate::editor::visual_composer::VisualComposerPanelParams {
                    scene: &mut self.scene,
                    engine: &self.engine,
                    project_root: self.project_root.as_deref(),
                    stage_resolution,
                    preview_quality: &mut self.composer_preview_quality,
                    stage_fit: &mut self.composer_stage_fit,
                    image_cache: &mut self.composer_image_cache,
                    image_failures: &mut self.composer_image_failures,
                    selected_entity_id: &mut self.selected_entity,
                    layer_overrides: &mut self.composer_layer_overrides,
                    active_event_node_id,
                    selected_authoring_node_id: composer_selected_node,
                    selected_authoring_node: selected_authoring_node.as_ref(),
                },
            );
            if let Some(act) = composer.ui(ui, &entity_owners) {
                composer_actions.push(act);
            }

            crate::editor::node_rendering::render_toast(ui, &mut self.toast);
        });

        // 5. Apply Deferred Actions
        for action in composer_actions {
            match action {
                crate::editor::visual_composer::VisualComposerAction::SelectNode(nid) => {
                    self.node_graph.set_single_selection(Some(nid));
                    self.selected_node = Some(nid);
                    self.selected_entity = None;
                }
                crate::editor::visual_composer::VisualComposerAction::CreateNode { node, pos } => {
                    self.add_composer_created_node(node, pos);
                    self.queue_editor_operation(
                        "composer_create_node",
                        "Created node from Visual Composer drag/drop",
                        Some("graph.nodes[]".to_string()),
                    );
                }
                crate::editor::visual_composer::VisualComposerAction::MutateNode {
                    node_id,
                    mutation,
                } => {
                    if self.apply_composer_node_mutation(node_id, mutation) {
                        self.node_graph.set_single_selection(Some(node_id));
                        self.selected_node = Some(node_id);
                        self.node_graph.mark_modified();
                    }
                }
                crate::editor::visual_composer::VisualComposerAction::AssignAssetToNode {
                    node_id,
                    target,
                    asset,
                } => match self.apply_imported_asset_to_node(node_id, target, asset.clone()) {
                    Ok(()) => {
                        self.node_graph.set_single_selection(Some(node_id));
                        self.selected_node = Some(node_id);
                        self.toast = Some(ToastState::success(format!(
                            "Assigned asset to selected node: {asset}"
                        )));
                    }
                    Err(err) => {
                        self.toast =
                            Some(ToastState::error(format!("Asset assignment failed: {err}")));
                    }
                },
                crate::editor::visual_composer::VisualComposerAction::AddCharacterToNode {
                    node_id,
                    name,
                    asset,
                    x,
                    y,
                } => match self.add_character_asset_to_node(node_id, name, asset.clone(), x, y) {
                    Ok(()) => {
                        self.node_graph.set_single_selection(Some(node_id));
                        self.selected_node = Some(node_id);
                        self.toast = Some(ToastState::success(format!(
                            "Added character asset to selected scene: {asset}"
                        )));
                    }
                    Err(err) => {
                        self.toast = Some(ToastState::error(format!(
                            "Character assignment failed: {err}"
                        )));
                    }
                },
                crate::editor::visual_composer::VisualComposerAction::LayerVisibilityChanged {
                    object_id,
                    visible,
                } => {
                    self.node_graph.mark_modified();
                    self.queue_editor_operation_with_values(
                        "layer_visibility_changed",
                        format!("Set layer {object_id} visible={visible}"),
                        Some(format!("composer.layers[{object_id}].visible")),
                        Some((!visible).to_string()),
                        Some(visible.to_string()),
                    );
                }
                crate::editor::visual_composer::VisualComposerAction::LayerLockChanged {
                    object_id,
                    locked,
                } => {
                    self.node_graph.mark_modified();
                    self.queue_editor_operation_with_values(
                        "layer_lock_changed",
                        format!("Set layer {object_id} locked={locked}"),
                        Some(format!("composer.layers[{object_id}].locked")),
                        Some((!locked).to_string()),
                        Some(locked.to_string()),
                    );
                }
                crate::editor::visual_composer::VisualComposerAction::TestFromSelection => {
                    self.start_composer_runtime_preview_from_node(composer_selected_node);
                }
                crate::editor::visual_composer::VisualComposerAction::TestRestart => {
                    self.restart_composer_runtime_preview();
                }
                crate::editor::visual_composer::VisualComposerAction::TestAdvance => {
                    self.advance_composer_runtime_preview(None);
                }
                crate::editor::visual_composer::VisualComposerAction::TestChoose(index) => {
                    if composer_selected_node
                        .and_then(|node_id| self.node_graph.get_node(node_id))
                        .is_some_and(|node| matches!(node, crate::editor::StoryNode::Choice { .. }))
                    {
                        self.start_composer_runtime_preview_from_node(composer_selected_node);
                    }
                    self.advance_composer_runtime_preview(Some(index));
                }
            }
        }
        for action in inspector_actions {
            match action {
                crate::editor::InspectorAction::PreviewAudio {
                    channel,
                    path,
                    volume,
                    loop_playback,
                } => {
                    self.play_editor_audio_preview(&channel, &path, volume, loop_playback);
                }
                crate::editor::InspectorAction::StopAudio { channel } => {
                    self.stop_editor_audio_preview(&channel);
                }
                crate::editor::InspectorAction::ImportAssetForNode {
                    node_id,
                    kind,
                    target,
                } => {
                    self.import_asset_for_node_dialog(node_id, kind, target);
                }
            }
        }

        // Common Sync
        // External panels may set selected_node directly (lint, diagnostics).
        // Apply that request only when it changed this frame, otherwise keep
        // node editor/composer selection as source of truth.
        if self.selected_node != selected_before {
            match self.selected_node {
                Some(requested) if self.node_graph.get_node(requested).is_some() => {
                    self.node_graph.set_single_selection(Some(requested));
                    self.selected_entity = None;
                }
                Some(_) => {
                    self.selected_node = self.node_graph.selected;
                }
                None => {
                    self.node_graph.set_single_selection(None);
                }
            }
        }

        if self.node_graph.selected != self.selected_node {
            self.selected_node = self.node_graph.selected;
            if self.selected_node.is_some() {
                self.selected_entity = None;
            }
        }
        if self.selected_node != selected_before {
            self.refresh_scene_from_engine_preview();
        }

        if self.node_graph.is_modified() && self.node_graph.dragging_node.is_none() {
            self.commit_modified_graph(graph_before_editor_interaction);
        }

        // 6. Floating/Detached Node Editor
        if self.node_editor_window_open && self.show_graph {
            let graph_before_detached_interaction = self.node_graph.clone();
            let mut embedded_open = self.node_editor_window_open;
            let mut detached_closed = false;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("node_editor_detached"),
                egui::ViewportBuilder::default()
                    .with_title("Node Editor")
                    .with_inner_size([1000.0, 700.0]),
                |viewport_ctx, class| match class {
                    egui::ViewportClass::Embedded => {
                        egui::Window::new("Node Editor")
                            .open(&mut embedded_open)
                            .resizable(true)
                            .show(viewport_ctx, |ui| {
                                let mut panel = NodeEditorPanel::new(
                                    &mut self.node_graph,
                                    &mut self.undo_stack,
                                );
                                panel.ui(ui);
                            });
                    }
                    egui::ViewportClass::Immediate | egui::ViewportClass::Root => {
                        egui::CentralPanel::default().show(viewport_ctx, |ui| {
                            let mut panel =
                                NodeEditorPanel::new(&mut self.node_graph, &mut self.undo_stack);
                            panel.ui(ui);
                        });
                        if viewport_ctx.input(|i| i.viewport().close_requested()) {
                            detached_closed = true;
                        }
                    }
                    egui::ViewportClass::Deferred => {}
                },
            );
            self.node_editor_window_open = embedded_open && !detached_closed;

            if self.node_graph.is_modified() && self.node_graph.dragging_node.is_none() {
                self.commit_modified_graph(graph_before_detached_interaction);
            }
        }
    }
}
