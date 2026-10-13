use super::{AuthoringDocumentCommand, AuthoringDocumentCommandOutcome, AuthoringDocumentSession};
use crate::authoring::{NodeGraph, OperationKind};

impl AuthoringDocumentSession {
    /// Adds client-facing field details to a successful command's canonical audit entry.
    /// Identity, fingerprints, validation evidence and operation kind remain session-owned.
    pub fn apply_with_description(
        &mut self,
        command: AuthoringDocumentCommand,
        details: String,
        field_path: String,
        before_value: Option<String>,
        after_value: Option<String>,
    ) -> Result<AuthoringDocumentCommandOutcome, String> {
        let mut outcome = self.apply(command)?;
        self.describe_latest_operation(
            &outcome.operation.operation_id,
            details,
            Some(field_path),
            before_value,
            after_value,
        );
        outcome.operation = self
            .document
            .operation_log
            .last()
            .expect("successful command records evidence")
            .clone();
        Ok(outcome)
    }

    /// Refines presentation details for the identified latest operation without
    /// changing its identity, kind, fingerprints or verification evidence.
    pub fn describe_latest_operation(
        &mut self,
        operation_id: &str,
        details: String,
        field_path: Option<String>,
        before_value: Option<String>,
        after_value: Option<String>,
    ) -> bool {
        let Some(operation) = self
            .document
            .operation_log
            .last_mut()
            .filter(|op| op.operation_id == operation_id)
        else {
            return false;
        };
        operation.details = details;
        if let Some(path) = field_path {
            operation.field_paths = vec![crate::authoring::FieldPath::new(path)];
        }
        if before_value.is_some() {
            operation.before_value = before_value;
        }
        if after_value.is_some() {
            operation.after_value = after_value;
        }
        true
    }

    /// Publishes a compound graph edit as one undoable command. The closure works
    /// on an isolated graph; a no-op creates no evidence and preserves redo.
    pub fn edit_graph<R>(
        &mut self,
        kind: OperationKind,
        details: impl Into<String>,
        field_path: Option<String>,
        edit: impl FnOnce(&mut NodeGraph) -> R,
    ) -> Result<(R, bool), String> {
        let mut graph = self.document.graph.clone();
        let result = edit(&mut graph);
        if graph.has_same_content(&self.document.graph) {
            return Ok((result, false));
        }
        self.apply(AuthoringDocumentCommand::ReplaceGraph {
            graph: Box::new(graph),
            kind,
            details: details.into(),
            field_path,
            before_value: None,
            after_value: None,
        })?;
        Ok((result, true))
    }
}
