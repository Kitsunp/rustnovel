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
            operation_id: self
                .document()
                .operation_log
                .last()
                .map(|entry| entry.operation_id.clone()),
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
        self.node_graph.clear_operation_hint();
        let Some(operation) = self.pending_editor_operation.take() else {
            self.last_operation_fingerprint = self.current_authoring_fingerprint();
            return;
        };
        if let Some(operation_id) = operation.operation_id {
            self.node_graph.authoring_session.describe_latest_operation(
                &operation_id,
                operation.details,
                operation.field_path,
                operation.before_value,
                operation.after_value,
            );
        }
        self.last_operation_fingerprint = self.current_authoring_fingerprint();
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
        let Some(after) = self.current_authoring_fingerprint() else {
            return;
        };
        let operation = PendingEditorOperation {
            operation_id: None,
            kind: kind.to_string(),
            details: details.into(),
            field_path,
            before_value,
            after_value,
        };
        self.append_editor_operation(operation, before, after);
    }

    pub fn commit_modified_graph(&mut self) {
        self.node_graph.commit_drag_positions();
        if !self.node_graph.is_modified() {
            return;
        }
        self.record_pending_editor_operation();
        self.refresh_document_state();
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
        use visual_novel_engine::authoring::AuthoringDocumentCommand;
        self.node_graph.commit_drag_positions();
        self.record_pending_editor_operation();
        let available = match command {
            AuthoringDocumentCommand::UndoLast | AuthoringDocumentCommand::RevertLast => {
                self.node_graph.authoring_session.undo_delta_count()
            }
            AuthoringDocumentCommand::RedoLast => {
                self.node_graph.authoring_session.redo_delta_count()
            }
            _ => unreachable!("document history command required"),
        };
        if available == 0 {
            return false;
        }
        if let Err(error) = self.apply_authoring_document_command(command) {
            self.toast = Some(ToastState::error(error));
            return false;
        }
        self.node_graph.clear_operation_hint();
        self.node_graph.clear_modified();
        self.node_graph.reconcile_view_after_document_change();
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
                self.node_graph.authoring_session.document(),
                &script,
            ),
        )
    }

    pub fn document(&self) -> &visual_novel_engine::authoring::AuthoringDocument {
        self.node_graph.authoring_session.document()
    }

    pub fn current_authoring_document(&self) -> visual_novel_engine::authoring::AuthoringDocument {
        self.node_graph.authoring_session.document().clone()
    }

    pub fn refresh_document_state(&mut self) {
        self.node_graph
            .authoring_session
            .limit_undo_history(crate::editor::undo::MAX_UNDO_STATES);
        if self.last_fix_command_index.is_some_and(|index| {
            !self
                .node_graph
                .authoring_session
                .is_command_in_history(index)
        }) {
            self.last_fix_command_index = None;
        }
    }

    pub fn apply_authoring_document_command(
        &mut self,
        command: visual_novel_engine::authoring::AuthoringDocumentCommand,
    ) -> Result<visual_novel_engine::authoring::AuthoringDocumentCommandOutcome, String> {
        let outcome = self.node_graph.authoring_session.apply(command)?;
        self.refresh_document_state();
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
        self.node_graph
            .authoring_session
            .record_external_operation(entry, verification);
        self.last_operation_fingerprint = Some(after);
    }
}
