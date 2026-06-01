#[path = "ui/layout_helpers.rs"]
mod layout_helpers;
use layout_helpers::{dock_rect, resize_width_override, vertical_splitter};
#[path = "ui/validation_panel.rs"]
mod validation_panel;

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
        let mut composer_preview_mode = self.composer_preview_mode;
        let mut composer_selected_node_for_actions =
            self.node_graph.selected.or(self.selected_node);
        let mut composer_actions = Vec::new();
        let stage_resolution = self
            .manifest
            .as_ref()
            .map(|manifest| manifest.settings.resolution);

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
                    self.apply_pending_graph_view(panel_rect.size());
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
                let active_event_node_id = self.engine.as_ref().and_then(|engine| {
                    self.node_graph
                        .authoring_graph()
                        .node_for_event_ip(engine.state().position)
                });
                let composer_selected_node = self.node_graph.selected.or(self.selected_node);
                composer_selected_node_for_actions = composer_selected_node;
                let selected_authoring_node = composer_selected_node
                    .and_then(|node_id| self.node_graph.get_node(node_id).cloned());
                let mut composer_background_fit =
                    self.composer_background_fit_for_node(composer_selected_node);
                let presentation_snapshot =
                    visual_novel_engine::authoring::composer::build_presentation_snapshot(
                        self.node_graph.authoring_graph(),
                        composer_selected_node,
                        stage_resolution,
                        self.engine.as_ref(),
                        Some(&self.player_locale),
                        Some(&self.localization_catalog),
                    );
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
                        .id_source("workbench_inspector_panel_scroll")
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
        self.handle_composer_actions(composer_actions, composer_selected_node_for_actions);
        self.handle_inspector_actions(inspector_actions);

        if self.reconcile_editor_selection_for_frame(selected_before) {
            ctx.request_repaint();
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
                                self.apply_pending_graph_view(ui.available_size());
                                let mut panel = NodeEditorPanel::new(
                                    &mut self.node_graph,
                                    &mut self.undo_stack,
                                );
                                panel.ui(ui);
                            });
                    }
                    egui::ViewportClass::Immediate | egui::ViewportClass::Root => {
                        egui::CentralPanel::default().show(viewport_ctx, |ui| {
                            self.apply_pending_graph_view(ui.available_size());
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

    pub fn reconcile_editor_selection_for_frame(&mut self, selected_before: Option<u32>) -> bool {
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
            self.selected_entity = None;
        }
        let changed = self.selected_node != selected_before;
        if changed {
            self.refresh_scene_from_engine_preview();
        }
        changed
    }

    fn apply_pending_graph_view(&mut self, viewport_size: egui::Vec2) {
        if self.pending_graph_fit {
            self.node_graph.zoom_to_fit_viewport(viewport_size);
            self.pending_graph_fit = false;
            self.pending_graph_focus = None;
            self.last_graph_viewport_size = Some(viewport_size);
            return;
        }
        if crate::editor::workbench::graph_viewport_resize_requires_fit(
            self.last_graph_viewport_size,
            viewport_size,
        ) && !self.node_graph.is_empty()
        {
            self.node_graph.zoom_to_fit_viewport(viewport_size);
            self.last_graph_viewport_size = Some(viewport_size);
            self.pending_graph_focus = None;
            return;
        }
        self.last_graph_viewport_size = Some(viewport_size);
        let Some(node_id) = self.pending_graph_focus.take() else {
            return;
        };
        if self.node_graph.get_node(node_id).is_some() {
            self.node_graph
                .pan_node_into_viewport(node_id, viewport_size);
        }
    }
}
