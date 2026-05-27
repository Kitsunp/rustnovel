use super::*;

impl EditorWorkbench {
    pub fn handle_asset_browser_actions(
        &mut self,
        actions: Vec<crate::editor::AssetBrowserAction>,
    ) {
        for action in actions {
            match action {
                crate::editor::AssetBrowserAction::Import(kind) => self.import_asset_dialog(kind),
                crate::editor::AssetBrowserAction::AssignToSelected { kind, name, path } => {
                    match self.assign_manifest_asset_to_selected_node(kind, &name, &path) {
                        Ok(node_id) => {
                            self.toast = Some(ToastState::success(format!(
                                "{} assigned to node #{node_id}: {path}",
                                kind.label()
                            )));
                        }
                        Err(err) => {
                            self.toast = Some(ToastState::error(format!(
                                "{} assignment failed: {err}",
                                kind.label()
                            )));
                        }
                    }
                }
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
    }

    pub fn handle_composer_actions(
        &mut self,
        actions: Vec<crate::editor::visual_composer::VisualComposerAction>,
        composer_selected_node: Option<u32>,
    ) {
        for action in actions {
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
                } => self.handle_layer_visibility_changed(object_id, visible),
                crate::editor::visual_composer::VisualComposerAction::LayerLockChanged {
                    object_id,
                    locked,
                } => self.handle_layer_lock_changed(object_id, locked),
                crate::editor::visual_composer::VisualComposerAction::BackgroundFitChanged {
                    node_id,
                    fit,
                } => self.handle_background_fit_changed(node_id, fit),
                crate::editor::visual_composer::VisualComposerAction::PreviewModeChanged(mode) => {
                    self.composer_preview_mode = mode;
                    self.refresh_scene_from_engine_preview();
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
    }

    pub fn handle_inspector_actions(&mut self, actions: Vec<crate::editor::InspectorAction>) {
        for action in actions {
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
    }

    fn handle_layer_visibility_changed(&mut self, object_id: String, visible: bool) {
        self.apply_document_command(
            visual_novel_engine::authoring::AuthoringDocumentCommand::SetLayerVisible {
                object_id,
                visible,
            },
        );
    }

    fn handle_layer_lock_changed(&mut self, object_id: String, locked: bool) {
        self.apply_document_command(
            visual_novel_engine::authoring::AuthoringDocumentCommand::SetLayerLocked {
                object_id,
                locked,
            },
        );
    }

    fn handle_background_fit_changed(
        &mut self,
        node_id: Option<u32>,
        fit: crate::editor::BackgroundFit,
    ) {
        if let Some(node_id) = node_id {
            self.apply_document_command(
                visual_novel_engine::authoring::AuthoringDocumentCommand::SetBackgroundFitOverride {
                    node_id,
                    fit,
                },
            );
        } else {
            self.composer_default_background_fit = fit;
        }
    }

    fn apply_document_command(
        &mut self,
        command: visual_novel_engine::authoring::AuthoringDocumentCommand,
    ) {
        let mut bus = visual_novel_engine::authoring::AuthoringDocumentCommandBus::new(
            self.current_authoring_document(),
        );
        let Ok(outcome) = bus.apply(command) else {
            return;
        };
        let document = bus.into_document();
        self.node_graph.replace_authoring_graph(document.graph);
        self.composer_layer_overrides = document.composer_layer_overrides.into_iter().collect();
        self.composer_background_fit_overrides = document
            .composer_background_fit_overrides
            .into_iter()
            .collect();
        self.operation_log = document.operation_log;
        self.verification_runs = document.verification_runs;
        self.last_operation_fingerprint = Some(outcome.after_fingerprint);
        self.node_graph.mark_modified();
    }
}
