use super::*;

impl EditorWorkbench {
    pub(crate) fn queue_editor_operation(
        &mut self,
        kind: impl Into<String>,
        details: impl Into<String>,
        field_path: Option<String>,
    ) {
        self.queue_editor_operation_with_values(kind, details, field_path, None, None);
    }

    pub(crate) fn queue_editor_operation_with_values(
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

    pub(crate) fn refresh_operation_fingerprint(&mut self) {
        self.last_operation_fingerprint = self.current_authoring_fingerprint();
    }

    pub(crate) fn record_pending_editor_operation(&mut self) {
        let after = match self.current_authoring_fingerprint() {
            Some(after) => after,
            None => return,
        };
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
        self.append_editor_operation(operation, self.last_operation_fingerprint.clone(), after);
    }

    pub(crate) fn record_editor_operation_now(
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
            kind: kind.to_string(),
            details: details.into(),
            field_path,
            before_value,
            after_value,
        };
        self.append_editor_operation(operation, before, after);
    }

    pub(crate) fn commit_modified_graph(&mut self, mut undo_snapshot: NodeGraph) {
        if !self.node_graph.is_modified() {
            return;
        }
        let push_undo = self.node_graph.operation_hint_pushes_undo();
        self.record_pending_editor_operation();
        if push_undo {
            undo_snapshot.clear_operation_hint();
            self.undo_stack.push(undo_snapshot);
        }
        self.node_graph.clear_modified();
        let _ = self.sync_graph_to_script();
    }

    pub(crate) fn apply_graph_undo(&mut self) -> bool {
        let before = self.node_graph.clone();
        let Some(previous) = self.undo_stack.undo(self.node_graph.clone()) else {
            return false;
        };
        self.node_graph = previous;
        self.node_graph.queue_operation_hint(
            "undo",
            "Undo graph editor mutation",
            Some("graph".to_string()),
            false,
        );
        self.node_graph.mark_modified();
        self.commit_modified_graph(before);
        true
    }

    pub(crate) fn apply_graph_redo(&mut self) -> bool {
        let before = self.node_graph.clone();
        let Some(next) = self.undo_stack.redo(self.node_graph.clone()) else {
            return false;
        };
        self.node_graph = next;
        self.node_graph.queue_operation_hint(
            "redo",
            "Redo graph editor mutation",
            Some("graph".to_string()),
            false,
        );
        self.node_graph.mark_modified();
        self.commit_modified_graph(before);
        true
    }

    pub(crate) fn handle_global_editor_shortcuts(&mut self, ctx: &egui::Context) -> bool {
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

    pub(crate) fn current_authoring_fingerprint(
        &self,
    ) -> Option<visual_novel_engine::authoring::AuthoringReportFingerprint> {
        let script = self.node_graph.to_script();
        Some(
            visual_novel_engine::authoring::build_authoring_document_report_fingerprint(
                &self.current_authoring_document(),
                &script,
            ),
        )
    }

    pub(crate) fn current_authoring_document(
        &self,
    ) -> visual_novel_engine::authoring::AuthoringDocument {
        let mut document = visual_novel_engine::authoring::AuthoringDocument::new(
            self.node_graph.authoring_graph().clone(),
        );
        document.composer_layer_overrides = self
            .composer_layer_overrides
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect();
        document.operation_log = self.operation_log.clone();
        document.verification_runs = self.verification_runs.clone();
        document
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
        other => OperationKind::Legacy(other.to_string()),
    }
}

impl EditorWorkbench {
    fn append_editor_operation(
        &mut self,
        operation: PendingEditorOperation,
        before: Option<visual_novel_engine::authoring::AuthoringReportFingerprint>,
        after: visual_novel_engine::authoring::AuthoringReportFingerprint,
    ) {
        let operation_kind = operation_kind_from_label(&operation.kind);
        let mut entry = visual_novel_engine::authoring::OperationLogEntry::new_typed(
            operation_kind,
            "applied",
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
        self.verification_runs.push(
            visual_novel_engine::authoring::VerificationRun::from_diagnostics(
                entry.operation_id.clone(),
                "editor_no_io",
                &after,
                &before_issues,
                &after_issues,
            ),
        );
        self.last_operation_fingerprint = Some(after);
        self.operation_log.push(entry);
    }
}
