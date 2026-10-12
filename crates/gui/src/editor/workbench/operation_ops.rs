use super::*;

impl EditorWorkbench {
    pub fn queue_editor_operation(
        &mut self,
        kind: impl Into<String>,
        details: impl Into<String>,
        field_path: Option<String>,
    ) {
        self.queue_editor_operation_with_values(kind, details, field_path, None, None);
    }

    pub fn queue_editor_operation_with_values(
        &mut self,
        kind: impl Into<String>,
        details: impl Into<String>,
        field_path: Option<String>,
        before_value: Option<String>,
        after_value: Option<String>,
    ) {
        self.pending_editor_operation = Some(PendingEditorOperation {
            kind: kind.into(),
            details: details.into(),
            field_path,
            before_value,
            after_value,
        });
    }

    pub fn refresh_operation_fingerprint(&mut self) {
        self.last_operation_fingerprint = self.current_authoring_fingerprint();
    }

    pub fn record_pending_editor_operation(&mut self) {
        let graph_hint = self.node_graph.take_operation_hint();
        let pending = self.pending_editor_operation.take();
        let operation = if let Some(operation) = pending {
            operation
        } else if let Some(hint) = graph_hint {
            PendingEditorOperation {
                kind: hint.kind,
                details: hint.details,
                field_path: hint.field_path,
                before_value: hint.before_value,
                after_value: hint.after_value,
            }
        } else {
            PendingEditorOperation {
                kind: "editor_graph_mutation".to_string(),
                details: "Graph changed through editor UI".to_string(),
                field_path: None,
                before_value: None,
                after_value: None,
            }
        };
        if self.authoring_session_matches_metadata_fields()
            && !self
                .authoring_session
                .document()
                .graph
                .has_same_content(self.node_graph.authoring_graph())
        {
            // Keep legacy graph edits in the same reversible stream as layers
            // and typed graph commands, rather than discarding the live session.
            let command = visual_novel_engine::authoring::AuthoringDocumentCommand::ReplaceGraph {
                graph: Box::new(self.node_graph.authoring_graph().clone()),
                kind: operation_kind_from_label(&operation.kind),
                details: operation.details,
                field_path: operation.field_path,
                before_value: operation.before_value,
                after_value: operation.after_value,
            };
            match self.authoring_session.apply(command) {
                Ok(outcome) => {
                    self.sync_authoring_fields_from_session();
                    self.last_operation_fingerprint = Some(outcome.after_fingerprint);
                }
                Err(error) => self.toast = Some(ToastState::error(error)),
            }
            return;
        }
        self.rebuild_authoring_session_from_fields();
        let Some(after) = self.current_authoring_fingerprint() else {
            return;
        };
        self.append_editor_operation(operation, self.last_operation_fingerprint.clone(), after);
    }

    pub fn record_editor_operation_now(
        &mut self,
        kind: &str,
        details: impl Into<String>,
        field_path: Option<String>,
        before_value: Option<String>,
        after_value: Option<String>,
        before: Option<visual_novel_engine::authoring::AuthoringReportFingerprint>,
    ) {
        self.ensure_authoring_session_matches_fields();
        let Some(after) = self.current_authoring_fingerprint() else {
            return;
        };
        let operation = PendingEditorOperation {
            kind: kind.to_string(),
            details: details.into(),
            field_path,
            before_value,
            after_value,
        };
        self.append_editor_operation(operation, before, after);
    }

    pub fn commit_modified_graph(&mut self, mut undo_snapshot: NodeGraph) {
        if !self.node_graph.is_modified() {
            return;
        }
        let push_undo = self.node_graph.operation_hint_pushes_undo();
        if self.pending_editor_operation.is_none()
            && !self.authoring_session.recorded_commands().is_empty()
            && self.authoring_session_matches_fields()
        {
            // The core command already produced its log and verification.
            self.node_graph.clear_operation_hint();
        } else {
            self.record_pending_editor_operation();
        }
        if push_undo {
            undo_snapshot.clear_operation_hint();
            self.undo_stack.push(undo_snapshot);
        }
        self.node_graph.clear_modified();
        if let Err(err) = self.sync_graph_to_script() {
            self.toast = Some(ToastState::error(format!(
                "Graph sync failed after editor mutation: {err}"
            )));
        }
    }

    pub fn apply_graph_undo(&mut self) -> bool {
        self.apply_document_history_command(
            visual_novel_engine::authoring::AuthoringDocumentCommand::UndoLast,
        )
    }

    pub fn apply_graph_redo(&mut self) -> bool {
        self.apply_document_history_command(
            visual_novel_engine::authoring::AuthoringDocumentCommand::RedoLast,
        )
    }

    pub(super) fn apply_document_history_command(
        &mut self,
        command: visual_novel_engine::authoring::AuthoringDocumentCommand,
    ) -> bool {
        use crate::editor::node_editor::NodeEditorHistoryAction;
        use visual_novel_engine::authoring::AuthoringDocumentCommand;
        if self.authoring_session_matches_metadata_fields()
            && !self
                .authoring_session
                .document()
                .graph
                .has_same_content(self.node_graph.authoring_graph())
        {
            self.record_pending_editor_operation();
        } else {
            self.ensure_authoring_session_matches_fields();
        }
        let (action, available) = match command {
            AuthoringDocumentCommand::UndoLast | AuthoringDocumentCommand::RevertLast => (
                NodeEditorHistoryAction::Undo,
                self.authoring_session.undo_delta_count(),
            ),
            AuthoringDocumentCommand::RedoLast => (
                NodeEditorHistoryAction::Redo,
                self.authoring_session.redo_delta_count(),
            ),
            _ => unreachable!("document history command required"),
        };
        if available == 0 {
            return false;
        }
        let before = self.node_graph.clone();
        if let Err(error) = self.apply_authoring_document_command(command) {
            self.toast = Some(ToastState::error(error));
            return false;
        }
        // Keep the public graph-only stack compatible; document history is
        // authoritative for both restoration and workbench control availability.
        match action {
            NodeEditorHistoryAction::Undo => {
                self.undo_stack.undo(before);
            }
            NodeEditorHistoryAction::Redo => {
                self.undo_stack.redo(before);
            }
        }
        self.node_graph.clear_operation_hint();
        self.node_graph.clear_modified();
        let graph = self.node_graph.authoring_graph();
        let selected = self
            .node_graph
            .selected
            .filter(|id| graph.get_node(*id).is_some());
        let retained_selection = self
            .node_graph
            .selected_nodes
            .iter()
            .copied()
            .filter(|id| graph.get_node(*id).is_some())
            .collect();
        self.node_graph.selected_nodes = retained_selection;
        self.node_graph.selected =
            selected.or_else(|| self.node_graph.selected_nodes.iter().next_back().copied());
        self.node_graph.editing = None;
        self.node_graph.dragging_node = None;
        self.node_graph.connecting_from = None;
        self.node_graph.connecting_sticky = false;
        self.node_graph.marquee_start = None;
        self.node_graph.marquee_current = None;
        self.node_graph.context_menu = None;
        self.selected_node = self.node_graph.selected;
        self.selected_entity = None;
        if let Err(error) = self.sync_graph_to_script() {
            self.toast = Some(ToastState::error(format!(
                "Graph sync failed after history action: {error}"
            )));
        }
        true
    }

    pub fn handle_global_editor_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        if ctx.wants_keyboard_input() {
            return false;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::Z)) {
            return self.apply_graph_undo();
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::Y)) {
            return self.apply_graph_redo();
        }
        false
    }

    pub fn current_authoring_fingerprint(
        &self,
    ) -> Option<visual_novel_engine::authoring::AuthoringReportFingerprint> {
        let script = self.node_graph.to_script();
        Some(
            visual_novel_engine::authoring::build_authoring_document_report_fingerprint(
                self.authoring_session.document(),
                &script,
            ),
        )
    }

    pub fn current_authoring_document(&self) -> visual_novel_engine::authoring::AuthoringDocument {
        self.authoring_session.document().clone()
    }

    fn authoring_document_from_fields(&self) -> visual_novel_engine::authoring::AuthoringDocument {
        let mut document = visual_novel_engine::authoring::AuthoringDocument::new(
            self.node_graph.authoring_graph().clone(),
        );
        document.composer_layer_overrides = self
            .composer_layer_overrides
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect();
        document.composer_background_fit_overrides = self
            .composer_background_fit_overrides
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect();
        document.operation_log = self.operation_log.clone();
        document.verification_runs = self.verification_runs.clone();
        document
    }

    pub fn rebuild_authoring_session_from_fields(&mut self) {
        self.last_fix_command_index = None;
        self.last_fix_snapshot = None;
        self.authoring_session = visual_novel_engine::authoring::AuthoringDocumentSession::new(
            self.authoring_document_from_fields(),
        );
    }

    pub(super) fn authoring_session_matches_fields(&self) -> bool {
        self.authoring_session
            .document()
            .graph
            .has_same_content(self.node_graph.authoring_graph())
            && self.authoring_session_matches_metadata_fields()
    }

    fn authoring_session_matches_metadata_fields(&self) -> bool {
        let document = self.authoring_session.document();
        document.composer_layer_overrides.len() == self.composer_layer_overrides.len()
            && document
                .composer_layer_overrides
                .iter()
                .all(|(key, value)| self.composer_layer_overrides.get(key) == Some(value))
            && document.composer_background_fit_overrides.len()
                == self.composer_background_fit_overrides.len()
            && document
                .composer_background_fit_overrides
                .iter()
                .all(|(key, value)| self.composer_background_fit_overrides.get(key) == Some(value))
            && document.operation_log == self.operation_log
            && document.verification_runs == self.verification_runs
    }

    pub fn ensure_authoring_session_matches_fields(&mut self) {
        if !self.authoring_session_matches_fields() {
            if !self.authoring_session.recorded_commands().is_empty()
                && self.authoring_session_matches_metadata_fields()
            {
                self.record_pending_editor_operation();
            } else {
                self.rebuild_authoring_session_from_fields();
            }
        }
    }

    pub fn sync_authoring_fields_from_session(&mut self) {
        self.authoring_session
            .limit_undo_history(crate::editor::undo::MAX_UNDO_STATES);
        if self
            .last_fix_command_index
            .is_some_and(|index| !self.authoring_session.is_command_in_history(index))
        {
            self.last_fix_command_index = None;
            self.last_fix_snapshot = None;
        }
        let document = self.authoring_session.document();
        self.node_graph
            .replace_authoring_graph(document.graph.clone());
        self.composer_layer_overrides = document
            .composer_layer_overrides
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect();
        self.composer_background_fit_overrides = document
            .composer_background_fit_overrides
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect();
        sync_trace_entries(&mut self.operation_log, &document.operation_log);
        sync_trace_entries(&mut self.verification_runs, &document.verification_runs);
    }

    pub fn apply_authoring_document_command(
        &mut self,
        command: visual_novel_engine::authoring::AuthoringDocumentCommand,
    ) -> Result<visual_novel_engine::authoring::AuthoringDocumentCommandOutcome, String> {
        self.ensure_authoring_session_matches_fields();
        let outcome = self.authoring_session.apply(command)?;
        self.sync_authoring_fields_from_session();
        self.last_operation_fingerprint = Some(outcome.after_fingerprint.clone());
        Ok(outcome)
    }

    pub fn apply_authoring_graph_command(
        &mut self,
        command: visual_novel_engine::authoring::AuthoringCommand,
    ) -> Result<visual_novel_engine::authoring::AuthoringDelta, String> {
        let outcome = self.apply_authoring_document_command(
            visual_novel_engine::authoring::AuthoringDocumentCommand::Graph(command),
        )?;
        let visual_novel_engine::authoring::AuthoringDocumentDelta::Graph(delta) = outcome.delta
        else {
            return Err("document session returned an unexpected graph delta".to_string());
        };
        Ok(*delta)
    }
}

fn sync_trace_entries<T: Clone + PartialEq>(target: &mut Vec<T>, source: &[T]) {
    let unchanged = target
        .iter()
        .zip(source)
        .take_while(|(previous, current)| previous == current)
        .count();
    target.truncate(unchanged);
    target.extend_from_slice(&source[unchanged..]);
}

fn operation_kind_from_label(label: &str) -> visual_novel_engine::authoring::OperationKind {
    use visual_novel_engine::authoring::OperationKind;
    match label {
        "node_created" | "composer_create_node" => OperationKind::NodeCreated,
        "node_removed" => OperationKind::NodeRemoved,
        "node_moved" => OperationKind::NodeMoved,
        "node_connected" => OperationKind::NodeConnected,
        "node_disconnected" => OperationKind::NodeDisconnected,
        "field_edited" | "editor_graph_mutation" => OperationKind::FieldEdited,
        "asset_imported" => OperationKind::AssetImported,
        "asset_removed" => OperationKind::AssetRemoved,
        "composer_drag_entity" => OperationKind::ComposerObjectMoved,
        "layer_visibility_changed" => OperationKind::LayerVisibilityChanged,
        "layer_lock_changed" => OperationKind::LayerLockChanged,
        "fragment_created" => OperationKind::FragmentCreated,
        "fragment_removed" => OperationKind::FragmentRemoved,
        "fragment_entered" => OperationKind::FragmentEntered,
        "fragment_left" => OperationKind::FragmentLeft,
        "subgraph_call_edited" => OperationKind::SubgraphCallEdited,
        "quick_fix" | "quick_fix_applied" => OperationKind::QuickFixApplied,
        "undo" => OperationKind::Undo,
        "redo" => OperationKind::Redo,
        "revert" => OperationKind::Revert,
        "report_imported" => OperationKind::ReportImported,
        other => {
            tracing::warn!("unknown editor operation kind '{other}', recording as field_edited");
            OperationKind::FieldEdited
        }
    }
}

impl EditorWorkbench {
    // Approved legacy sink: queued graph/editor operations still pass through this
    // adapter until each concrete mutation has a typed core command bus contract.
    fn append_editor_operation(
        &mut self,
        operation: PendingEditorOperation,
        before: Option<visual_novel_engine::authoring::AuthoringReportFingerprint>,
        after: visual_novel_engine::authoring::AuthoringReportFingerprint,
    ) {
        let operation_kind = operation_kind_from_label(&operation.kind);
        let mut entry = visual_novel_engine::authoring::OperationLogEntry::new_typed(
            operation_kind,
            visual_novel_engine::authoring::OperationStatus::Applied,
            operation.details,
        )
        .with_session("local-editor")
        .with_author("local-user");
        if let Some(before) = before.as_ref() {
            entry = entry.with_before_after_fingerprints(before, &after);
        } else {
            entry = entry.with_fingerprint(&after);
        }
        if let Some(field_path) = operation.field_path {
            entry = entry.with_field_path(field_path);
        }
        if let (Some(before_value), Some(after_value)) =
            (operation.before_value, operation.after_value)
        {
            entry = entry.with_values(before_value, after_value);
        }
        let before_issues = self.validation_issues.clone();
        let after_issues = visual_novel_engine::authoring::validate_authoring_graph_no_io(
            self.node_graph.authoring_graph(),
        );
        let verification = visual_novel_engine::authoring::VerificationRun::from_diagnostics(
            entry.operation_id.clone(),
            "editor_no_io",
            &after,
            &before_issues,
            &after_issues,
        );
        self.authoring_session
            .record_external_operation(entry.clone(), verification.clone());
        self.verification_runs.push(verification);
        self.last_operation_fingerprint = Some(after);
        self.operation_log.push(entry);
    }
}
