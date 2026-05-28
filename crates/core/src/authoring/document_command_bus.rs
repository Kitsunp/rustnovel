mod apply;
mod inverse;
mod read_model;
mod replay;
mod types;

use super::{
    authoring_document_layout_sha256, authoring_document_sha256,
    build_authoring_document_report_fingerprint, validate_authoring_graph_no_io, AuthoringDocument,
    AuthoringReportFingerprint, DiagnosticTarget, LintIssue, OperationKind, OperationLogEntry,
    OperationStatus, VerificationRun,
};

pub use read_model::{
    AssetRefIndex, AuthoringReadModel, AuthoringReportStaleState, ComposerLayerIndex,
    DiagnosticsIndex, NodeIndex, RouteIndex,
};
pub use types::{
    AuthoringDirtyFlags, AuthoringDocumentCommand, AuthoringDocumentCommandOutcome,
    AuthoringDocumentDelta,
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
pub struct AuthoringDocumentSession {
    document: AuthoringDocument,
    undo_deltas: Vec<AuthoringDocumentDelta>,
    redo_deltas: Vec<AuthoringDocumentDelta>,
    recorded_commands: Vec<AuthoringDocumentCommand>,
    cached_fingerprint: Option<AuthoringReportFingerprint>,
    cached_validation: Option<Vec<LintIssue>>,
    dirty: AuthoringDirtyFlags,
    read_model: AuthoringReadModel,
}

#[derive(Clone, Debug)]
pub struct AuthoringDocumentCommandApplyResult {
    pub document: AuthoringDocument,
    pub outcome: AuthoringDocumentCommandOutcome,
}

pub fn apply_authoring_document_command_headless(
    document: AuthoringDocument,
    command: AuthoringDocumentCommand,
) -> Result<AuthoringDocumentCommandApplyResult, String> {
    let mut session = AuthoringDocumentSession::new(document);
    let outcome = session.apply(command)?;
    Ok(AuthoringDocumentCommandApplyResult {
        document: session.into_document(),
        outcome,
    })
}

impl AuthoringDocumentSession {
    pub fn new(document: AuthoringDocument) -> Self {
        let read_model = AuthoringReadModel::from_document(&document);
        Self {
            document,
            undo_deltas: Vec::new(),
            redo_deltas: Vec::new(),
            recorded_commands: Vec::new(),
            cached_fingerprint: None,
            cached_validation: None,
            dirty: AuthoringDirtyFlags::default(),
            read_model,
        }
    }

    pub fn document(&self) -> &AuthoringDocument {
        &self.document
    }

    pub fn into_document(self) -> AuthoringDocument {
        self.document
    }

    pub fn read_model(&self) -> &AuthoringReadModel {
        &self.read_model
    }

    pub fn dirty_flags(&self) -> AuthoringDirtyFlags {
        self.dirty
    }

    pub fn clear_dirty_flags(&mut self) {
        self.dirty = AuthoringDirtyFlags::default();
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

        let before_fingerprint = self.current_fingerprint();
        let before_issues = self.current_validation();
        let (delta, metadata) = self.apply_without_logging(&command)?;
        self.read_model.update_from_delta(&self.document, &delta);
        let dirty = AuthoringDirtyFlags::for_delta(&delta);
        self.dirty.include(dirty);
        let after_fingerprint = self.fingerprint_after_delta(&before_fingerprint, &delta);
        let after_issues = if dirty.validation_dirty {
            self.cached_validation = None;
            self.current_validation()
        } else {
            self.cached_validation = Some(before_issues.clone());
            before_issues.clone()
        };
        if dirty.validation_dirty {
            self.read_model.replace_diagnostics(&after_issues);
        }
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

    fn current_fingerprint(&mut self) -> AuthoringReportFingerprint {
        if let Some(fingerprint) = &self.cached_fingerprint {
            return fingerprint.clone();
        }
        let fingerprint = build_authoring_document_report_fingerprint(
            &self.document,
            &self.document.graph.to_script_lossy_for_diagnostics(),
        );
        self.cached_fingerprint = Some(fingerprint.clone());
        fingerprint
    }

    fn current_validation(&mut self) -> Vec<LintIssue> {
        if let Some(issues) = &self.cached_validation {
            return issues.clone();
        }
        let issues = validate_authoring_graph_no_io(&self.document.graph);
        self.cached_validation = Some(issues.clone());
        issues
    }

    fn fingerprint_after_delta(
        &mut self,
        before: &AuthoringReportFingerprint,
        delta: &AuthoringDocumentDelta,
    ) -> AuthoringReportFingerprint {
        if delta_preserves_semantic_fingerprint(delta) {
            let mut after = before.clone();
            after.layout_sha256 = authoring_document_layout_sha256(&self.document);
            let full_document_sha256 = authoring_document_sha256(&self.document);
            after.full_document_sha256 = full_document_sha256.clone();
            after.graph_sha256 = full_document_sha256;
            self.cached_fingerprint = Some(after.clone());
            after
        } else {
            self.cached_fingerprint = None;
            self.current_fingerprint()
        }
    }

    fn revert_last(&mut self) -> Result<AuthoringDocumentCommandOutcome, String> {
        let delta = self
            .undo_deltas
            .pop()
            .ok_or_else(|| "no document command delta to revert".to_string())?;
        let before_fingerprint = self.current_fingerprint();
        let before_issues = self.current_validation();
        if let Err(error) = self.apply_inverse_delta(&delta) {
            self.undo_deltas.push(delta);
            return Err(error);
        }
        let reverted = AuthoringDocumentDelta::Reverted {
            reverted: Box::new(delta.clone()),
        };
        self.read_model.update_from_delta(&self.document, &reverted);
        let dirty = AuthoringDirtyFlags::for_delta(&reverted);
        self.dirty.include(dirty);
        let after_fingerprint = self.fingerprint_after_delta(&before_fingerprint, &reverted);
        let after_issues = if dirty.validation_dirty {
            self.cached_validation = None;
            self.current_validation()
        } else {
            self.cached_validation = Some(before_issues.clone());
            before_issues.clone()
        };
        if dirty.validation_dirty {
            self.read_model.replace_diagnostics(&after_issues);
        }
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

#[derive(Clone, Debug)]
pub struct AuthoringDocumentCommandBus {
    session: AuthoringDocumentSession,
}

impl AuthoringDocumentCommandBus {
    pub fn new(document: AuthoringDocument) -> Self {
        Self {
            session: AuthoringDocumentSession::new(document),
        }
    }

    pub fn from_session(session: AuthoringDocumentSession) -> Self {
        Self { session }
    }

    pub fn session(&self) -> &AuthoringDocumentSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut AuthoringDocumentSession {
        &mut self.session
    }

    pub fn into_session(self) -> AuthoringDocumentSession {
        self.session
    }

    pub fn document(&self) -> &AuthoringDocument {
        self.session.document()
    }

    pub fn into_document(self) -> AuthoringDocument {
        self.session.into_document()
    }

    pub fn read_model(&self) -> &AuthoringReadModel {
        self.session.read_model()
    }

    pub fn dirty_flags(&self) -> AuthoringDirtyFlags {
        self.session.dirty_flags()
    }

    pub fn clear_dirty_flags(&mut self) {
        self.session.clear_dirty_flags();
    }

    pub fn recorded_commands(&self) -> &[AuthoringDocumentCommand] {
        self.session.recorded_commands()
    }

    pub fn undo_delta_count(&self) -> usize {
        self.session.undo_delta_count()
    }

    pub fn redo_delta_count(&self) -> usize {
        self.session.redo_delta_count()
    }

    pub fn apply(
        &mut self,
        command: AuthoringDocumentCommand,
    ) -> Result<AuthoringDocumentCommandOutcome, String> {
        self.session.apply(command)
    }
}

fn delta_preserves_semantic_fingerprint(delta: &AuthoringDocumentDelta) -> bool {
    match delta {
        AuthoringDocumentDelta::LayerVisibleChanged { .. }
        | AuthoringDocumentDelta::LayerLockedChanged { .. }
        | AuthoringDocumentDelta::BackgroundFitChanged { .. }
        | AuthoringDocumentDelta::BackgroundFitCleared { .. } => true,
        AuthoringDocumentDelta::Graph(_) => false,
        AuthoringDocumentDelta::Reverted { reverted } => {
            delta_preserves_semantic_fingerprint(reverted)
        }
    }
}
