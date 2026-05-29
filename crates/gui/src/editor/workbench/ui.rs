use super::*;

impl EditorWorkbench {
    pub fn render_editor_mode(&mut self, ctx: &egui::Context) {
        self.handle_global_editor_shortcuts(ctx);
        let selected_before = self.selected_node;
        let graph_before_editor_interaction = self.node_graph.clone();
        let layout = super::layout::editor_panel_layout(
            ctx.available_rect().width(),
            ctx.available_rect().height(),
            &self.layout_overrides,
        );

        if self.show_timeline {
            let timeline_layout = super::layout::timeline_panel_layout(layout.timeline, false);
            let timeline_response =
                egui::TopBottomPanel::bottom(format!("timeline_panel_{}", self.layout_generation))
                    .resizable(true)
                    .default_height(timeline_layout.default)
                    .min_height(timeline_layout.min)
                    .max_height(timeline_layout.max)
                    .show(ctx, |ui| {
                        let mut current_time_u32 = self.current_time as u32;
                        let mut is_playing = self.is_playing;

                        TimelinePanel::new(
                            &mut self.timeline,
                            &mut current_time_u32,
                            &mut is_playing,
                        )
                        .with_selected_entity(self.selected_entity)
                        .ui(ui);

                        self.current_time = current_time_u32 as f32;
                        self.is_playing = is_playing;
                    });
            self.layout_overrides.timeline_height = super::layout::dragged_panel_override(
                self.layout_overrides.timeline_height,
                timeline_response.response.rect.height(),
                timeline_layout.min,
                timeline_layout.max,
                timeline_response.response.dragged(),
            );
        }

        let mut asset_browser_actions = Vec::new();
        let mut inspector_actions = Vec::new();

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
        let mut composer_background_fit =
            self.composer_background_fit_for_node(composer_selected_node);
        let mut composer_preview_mode = self.composer_preview_mode;
        let mut composer_actions = Vec::new();
        let stage_resolution = self
            .manifest
            .as_ref()
            .map(|manifest| manifest.settings.resolution);
        let presentation_snapshot =
            visual_novel_engine::authoring::composer::build_presentation_snapshot(
                self.node_graph.authoring_graph(),
                composer_selected_node,
                stage_resolution,
                self.engine.as_ref(),
                Some(&self.player_locale),
                Some(&self.localization_catalog),
            );

        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_rect_before_wrap();
            let visibility = super::layout::EditorDockVisibility {
                asset_browser: self.show_asset_browser,
                graph: self.show_graph && !self.node_editor_window_open,
                inspector: self.show_inspector,
            };
            let dock = super::layout::resolve_editor_dock_layout(
                available.width(),
                available.height(),
                &self.layout_overrides,
                visibility,
            );
            let origin = available.min;

            if let Some(rect) = dock.asset_browser {
                let panel_rect = dock_rect(origin, rect);
                ui.allocate_ui_at_rect(panel_rect, |ui| {
                    ui.set_clip_rect(panel_rect);
                    if let Some(manifest) = &self.manifest {
                        asset_browser_actions.extend(
                            AssetBrowserPanel::new(
                                manifest,
                                self.project_root.as_deref(),
                                &mut self.composer_image_cache,
                                &mut self.composer_image_failures,
                                &mut self.resource_service,
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
            if let Some(splitter) = dock.asset_splitter {
                let response = vertical_splitter(ui, dock_rect(origin, splitter));
                if response.dragged() {
                    let dx = ui.input(|input| input.pointer.delta().x);
                    resize_width_override(
                        &mut self.layout_overrides.asset_width,
                        &mut self.layout_overrides.dock_reference_width,
                        dock.asset_browser.map_or(0.0, |rect| rect.w),
                        dx,
                        dock.panel_sizes.asset_browser.min,
                        dock.panel_sizes.asset_browser.max,
                        available.width(),
                    );
                    ui.ctx().request_repaint();
                }
            }

            if let Some(rect) = dock.graph {
                let panel_rect = dock_rect(origin, rect);
                ui.allocate_ui_at_rect(panel_rect, |ui| {
                    ui.set_clip_rect(panel_rect);
                    ui.heading(crate::editor::node_editor::logic_graph_heading_label(
                        panel_rect.width(),
                    ));
                    let mut panel =
                        NodeEditorPanel::new(&mut self.node_graph, &mut self.undo_stack);
                    panel.ui(ui);
                });
            }
            if let Some(splitter) = dock.graph_splitter {
                let response = vertical_splitter(ui, dock_rect(origin, splitter));
                if response.dragged() {
                    let dx = ui.input(|input| input.pointer.delta().x);
                    resize_width_override(
                        &mut self.layout_overrides.graph_width,
                        &mut self.layout_overrides.dock_reference_width,
                        dock.graph.map_or(0.0, |rect| rect.w),
                        dx,
                        dock.panel_sizes.graph.min,
                        dock.panel_sizes.graph.max,
                        available.width(),
                    );
                    ui.ctx().request_repaint();
                }
            }

            let composer_rect = dock_rect(origin, dock.composer);
            ui.allocate_ui_at_rect(composer_rect, |ui| {
                ui.set_clip_rect(composer_rect);
                self.render_fragments_panel(ui);
                ui.separator();
                let mut composer = crate::editor::visual_composer::VisualComposerPanel::new(
                    crate::editor::visual_composer::VisualComposerPanelParams {
                        scene: &mut self.scene,
                        engine: &self.engine,
                        project_root: self.project_root.as_deref(),
                        stage_resolution,
                        preview_quality: &mut self.composer_preview_quality,
                        stage_fit: &mut self.composer_stage_fit,
                        background_fit: &mut composer_background_fit,
                        preview_mode: &mut composer_preview_mode,
                        image_cache: &mut self.composer_image_cache,
                        image_failures: &mut self.composer_image_failures,
                        resource_service: &mut self.resource_service,
                        selected_entity_id: &mut self.selected_entity,
                        layer_overrides: &self.composer_layer_overrides,
                        active_event_node_id,
                        selected_authoring_node_id: composer_selected_node,
                        selected_authoring_node: selected_authoring_node.as_ref(),
                        presentation_snapshot: Some(&presentation_snapshot),
                    },
                );
                if let Some(act) = composer.ui(ui, &entity_owners) {
                    composer_actions.push(act);
                }

                crate::editor::node_rendering::render_toast(ui, &mut self.toast);
            });

            if let Some(splitter) = dock.inspector_splitter {
                let response = vertical_splitter(ui, dock_rect(origin, splitter));
                if response.dragged() {
                    let dx = ui.input(|input| input.pointer.delta().x);
                    resize_width_override(
                        &mut self.layout_overrides.inspector_width,
                        &mut self.layout_overrides.dock_reference_width,
                        dock.inspector.map_or(0.0, |rect| rect.w),
                        -dx,
                        dock.panel_sizes.inspector.min,
                        dock.panel_sizes.inspector.max,
                        available.width(),
                    );
                    ui.ctx().request_repaint();
                }
            }

            if let Some(rect) = dock.inspector {
                let panel_rect = dock_rect(origin, rect);
                ui.allocate_ui_at_rect(panel_rect, |ui| {
                    ui.set_clip_rect(panel_rect);
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if self.show_validation {
                                self.render_validation_report_panel(ui);
                                ui.separator();
                            }

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
                });
            }
        });
        if self.show_validation
            && super::layout::validation_report_placement(self.show_inspector)
                == super::layout::ValidationReportPlacement::FloatingWindow
        {
            let mut open = true;
            egui::Window::new("Validation Report")
                .id(egui::Id::new("validation_report_floating"))
                .default_size(egui::vec2(520.0, 420.0))
                .resizable(true)
                .open(&mut open)
                .show(ctx, |ui| {
                    self.render_validation_report_panel(ui);
                });
            if !open {
                self.show_validation = false;
                self.validation_collapsed = false;
            }
        }
        self.composer_preview_mode = composer_preview_mode;

        // 5. Apply Deferred Actions
        self.handle_asset_browser_actions(asset_browser_actions);
        self.handle_composer_actions(composer_actions, composer_selected_node);
        self.handle_inspector_actions(inspector_actions);

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
            let floating_rect = self
                .workspace_layout_from_current_flags()
                .panel(super::layout::WorkspacePanelId::NodeEditor)
                .floating_rect;
            let floating_size = floating_rect
                .map(|rect| [rect.w, rect.h])
                .unwrap_or([1000.0, 700.0]);
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("node_editor_detached"),
                egui::ViewportBuilder::default()
                    .with_title("Node Editor")
                    .with_inner_size(floating_size),
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

    fn render_validation_report_panel(&mut self, ui: &mut egui::Ui) {
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
        let report_body_height = super::layout::validation_report_body_height(
            ui.available_height(),
            self.validation_issues.len(),
        );

        let mut close_validation = false;
        let mut toggle_validation_collapse = false;
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Validation Report | E:{} W:{} I:{}",
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
                if ui.small_button(collapse_label).clicked() {
                    toggle_validation_collapse = true;
                }
                if ui.small_button("Cerrar").clicked() {
                    close_validation = true;
                }
            });

            if self.validation_collapsed {
                return;
            }

            ui.add_space(2.0);
            if self.validation_issues.is_empty() {
                ui.colored_label(egui::Color32::GREEN, "No issues found.");
                return;
            }

            egui::ScrollArea::vertical()
                .id_source("validation_report_embedded_scroll")
                .max_height(report_body_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let lint_response = LintPanel::new(
                        &self.validation_issues,
                        &mut self.selected_node,
                        &mut self.selected_issue,
                        &mut self.diagnostic_language,
                        &self.node_graph,
                        self.last_fix_snapshot.is_some(),
                    )
                    .ui(ui);
                    self.handle_lint_panel_actions(lint_response.actions);
                });
        });

        if toggle_validation_collapse {
            self.validation_collapsed = !self.validation_collapsed;
        }
        if close_validation {
            self.show_validation = false;
            self.validation_collapsed = false;
        }
    }

    fn handle_lint_panel_actions(
        &mut self,
        actions: Vec<crate::editor::lint_panel::LintPanelAction>,
    ) {
        for action in actions {
            match action {
                crate::editor::lint_panel::LintPanelAction::ApplyFix {
                    issue_index,
                    fix_id,
                    structural,
                } => {
                    if structural {
                        match self.prepare_structural_fix_confirmation(issue_index, &fix_id) {
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
                                self.toast =
                                    Some(ToastState::success(format!("Applied fix '{fix_id}'")));
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
                        self.toast =
                            Some(ToastState::error(format!("Issue auto-fix failed: {err}")));
                    }
                },
                crate::editor::lint_panel::LintPanelAction::RevertLastFix => {
                    if self.revert_last_fix() {
                        self.toast = Some(ToastState::success("Last fix reverted successfully"));
                    } else {
                        self.toast = Some(ToastState::warning("No fix to revert"));
                    }
                }
            }
        }
    }
}

fn dock_rect(origin: egui::Pos2, rect: super::layout::WorkspacePanelRect) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(origin.x + rect.x, origin.y + rect.y),
        egui::vec2(rect.w.max(1.0), rect.h.max(1.0)),
    )
}

fn vertical_splitter(ui: &mut egui::Ui, rect: egui::Rect) -> egui::Response {
    let response = ui
        .allocate_rect(rect, egui::Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    let color = if response.dragged() || response.hovered() {
        egui::Color32::from_gray(90)
    } else {
        egui::Color32::from_gray(46)
    };
    ui.painter()
        .rect_filled(rect.shrink2(egui::vec2(3.0, 0.0)), 0.0, color);
    response
}

fn resize_width_override(
    target: &mut Option<f32>,
    reference_width: &mut Option<f32>,
    current: f32,
    delta: f32,
    min: f32,
    max: f32,
    available_width: f32,
) {
    if !delta.is_finite() || delta.abs() < 0.1 {
        return;
    }
    if available_width.is_finite() && available_width >= 360.0 {
        *reference_width = Some(available_width);
    }
    *target = Some((current + delta).clamp(min, max));
}
