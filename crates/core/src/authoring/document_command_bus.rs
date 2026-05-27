mod apply;
mod inverse;
mod replay;
mod types;

use super::{
    build_authoring_document_report_fingerprint, validate_authoring_graph_no_io, AuthoringDocument,
    DiagnosticTarget, OperationKind, OperationLogEntry, OperationStatus, VerificationRun,
};

pub use types::{
    AuthoringDocumentCommand, AuthoringDocumentCommandOutcome, AuthoringDocumentDelta,
};

pub(super) struct DocumentApplyMetadata {
    pub(super) kind: OperationKind,
    pub(super) field_paths: Vec<String>,
    pub(super) targets: Vec<DiagnosticTarget>,
    pub(super) before_value: Option<String>,
    pub(super) after_value: Option<String>,
}

pub(super) type DocumentApplyResult =
    Result<(AuthoringDocumentDelta, DocumentApplyMetadata), String>;

#[derive(Clone, Debug)]
pub struct AuthoringDocumentCommandBus {
    document: AuthoringDocument,
    undo_deltas: Vec<AuthoringDocumentDelta>,
    redo_deltas: Vec<AuthoringDocumentDelta>,
    recorded_commands: Vec<AuthoringDocumentCommand>,
}

impl AuthoringDocumentCommandBus {
    pub fn new(document: AuthoringDocument) -> Self {
        Self {
            document,
            undo_deltas: Vec::new(),
            redo_deltas: Vec::new(),
            recorded_commands: Vec::new(),
        }
    }

    pub fn document(&self) -> &AuthoringDocument {
        &self.document
    }

    pub fn into_document(self) -> AuthoringDocument {
        self.document
    }

    pub fn recorded_commands(&self) -> &[AuthoringDocumentCommand] {
        &self.recorded_commands
    }

    pub fn undo_delta_count(&self) -> usize {
        self.undo_deltas.len()
    }

    pub fn redo_delta_count(&self) -> usize {
        self.redo_deltas.len()
    }

    pub fn apply(
        &mut self,
        command: AuthoringDocumentCommand,
    ) -> Result<AuthoringDocumentCommandOutcome, String> {
        if matches!(command, AuthoringDocumentCommand::RevertLast) {
            return self.revert_last();
        }

        let before_issues = validate_authoring_graph_no_io(&self.document.graph);
        let before_fingerprint = self.current_fingerprint();
        let (delta, metadata) = self.apply_without_logging(&command)?;
        let after_fingerprint = self.current_fingerprint();
        let after_issues = validate_authoring_graph_no_io(&self.document.graph);
        let mut operation = OperationLogEntry::new_typed(
            metadata.kind,
            OperationStatus::Applied,
            format!("authoring document command applied: {command:?}"),
        )
        .with_before_after_fingerprints(&before_fingerprint, &after_fingerprint);
        for field_path in metadata.field_paths {
            operation = operation.with_field_path(field_path);
        }
        for target in metadata.targets {
            operation = operation.with_target(target);
        }
        operation.before_value = metadata.before_value;
        operation.after_value = metadata.after_value;
        let verification = VerificationRun::from_diagnostics(
            operation.operation_id.clone(),
            "document_command_bus",
            &after_fingerprint,
            &before_issues,
            &after_issues,
        );

        self.undo_deltas.push(delta.clone());
        self.redo_deltas.clear();
        self.recorded_commands.push(command);
        self.document.operation_log.push(operation.clone());
        self.document.verification_runs.push(verification.clone());
        Ok(AuthoringDocumentCommandOutcome {
            delta,
            operation,
            verification,
            before_fingerprint,
            after_fingerprint,
        })
    }

    fn current_fingerprint(&self) -> super::AuthoringReportFingerprint {
        build_authoring_document_report_fingerprint(
            &self.document,
            &self.document.graph.to_script_lossy_for_diagnostics(),
        )
    }

    fn revert_last(&mut self) -> Result<AuthoringDocumentCommandOutcome, String> {
        let delta = self
            .undo_deltas
            .pop()
            .ok_or_else(|| "no document command delta to revert".to_string())?;
        let before_issues = validate_authoring_graph_no_io(&self.document.graph);
        let before_fingerprint = self.current_fingerprint();
        self.apply_inverse_delta(&delta)?;
        let after_fingerprint = self.current_fingerprint();
        let after_issues = validate_authoring_graph_no_io(&self.document.graph);
        let operation = OperationLogEntry::new_typed(
            OperationKind::Revert,
            OperationStatus::Applied,
            "reverted last document command delta",
        )
        .with_before_after_fingerprints(&before_fingerprint, &after_fingerprint);
        let verification = VerificationRun::from_diagnostics(
            operation.operation_id.clone(),
            "document_command_bus",
            &after_fingerprint,
            &before_issues,
            &after_issues,
        );
        let reverted = AuthoringDocumentDelta::Reverted {
            reverted: Box::new(delta.clone()),
        };
        self.redo_deltas.push(delta);
        self.recorded_commands
            .push(AuthoringDocumentCommand::RevertLast);
        self.document.operation_log.push(operation.clone());
        self.document.verification_runs.push(verification.clone());
        Ok(AuthoringDocumentCommandOutcome {
            delta: reverted,
            operation,
            verification,
            before_fingerprint,
            after_fingerprint,
        })
    }
}
