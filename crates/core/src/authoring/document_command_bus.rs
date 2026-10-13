mod apply;
mod inverse;
mod read_model;
mod replay;
mod types;

use super::command_bus::CommandApplyMetadata as DocumentApplyMetadata;

use super::{
    authoring_document_layout_sha256, authoring_document_sha256,
    build_authoring_document_report_fingerprint, validate_authoring_graph_no_io, AuthoringDocument,
    AuthoringReportFingerprint, LintIssue, OperationKind, OperationLogEntry, OperationStatus,
    VerificationRun,
};

pub use read_model::{
    AssetRefIndex, AuthoringReadModel, AuthoringReportStaleState, ComposerLayerIndex,
    DiagnosticsIndex, NodeIndex, RouteIndex,
};
pub use types::{
    AuthoringDirtyFlags, AuthoringDocumentCommand, AuthoringDocumentCommandOutcome,
    AuthoringDocumentDelta,
};

pub(super) type DocumentApplyResult =
    Result<(AuthoringDocumentDelta, DocumentApplyMetadata), String>;

#[derive(Clone, Debug)]
pub struct AuthoringDocumentSession {
    document: AuthoringDocument,
    undo_deltas: Vec<(AuthoringDocumentDelta, usize)>,
    redo_command_indices: Vec<usize>,
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

#[derive(Clone, Debug)]
pub struct AuthoringDocumentCommandsApplyResult {
    pub document: AuthoringDocument,
    pub outcomes: Vec<AuthoringDocumentCommandOutcome>,
}

/// Applies a batch in one session, preserving its history and caches.
/// The owned document is returned only on success; callers must save it afterwards.
pub fn apply_authoring_document_commands_headless(
    document: AuthoringDocument,
    commands: Vec<AuthoringDocumentCommand>,
) -> Result<AuthoringDocumentCommandsApplyResult, String> {
    if commands.is_empty() {
        return Err("command batch must not be empty".to_string());
    }
    let mut session = AuthoringDocumentSession::new(document);
    let mut outcomes = Vec::with_capacity(commands.len());
    for (index, command) in commands.into_iter().enumerate() {
        outcomes.push(
            session
                .apply(command)
                .map_err(|error| format!("command {} failed: {error}", index + 1))?,
        );
    }
    Ok(AuthoringDocumentCommandsApplyResult {
        document: session.into_document(),
        outcomes,
    })
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
            redo_command_indices: Vec::new(),
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
        self.redo_command_indices.len()
    }

    pub fn last_undo_command_index(&self) -> Option<usize> {
        self.undo_deltas.last().map(|(_, index)| *index)
    }

    pub fn is_command_in_history(&self, index: usize) -> bool {
        self.undo_deltas
            .iter()
            .any(|(_, candidate)| *candidate == index)
            || self.redo_command_indices.contains(&index)
    }

    /// Limits retained inverse deltas without changing the command replay log.
    pub fn limit_undo_history(&mut self, max_steps: usize) {
        let excess = self.undo_deltas.len().saturating_sub(max_steps);
        self.undo_deltas.drain(..excess);
    }

    /// Appends evidence for an external action, such as a manifest asset import.
    /// It does not mutate document content or create a reversible command.
    pub fn record_external_operation(
        &mut self,
        operation: OperationLogEntry,
        verification: VerificationRun,
    ) {
        self.document.operation_log.push(operation);
        self.document.verification_runs.push(verification);
        self.cached_fingerprint = None;
    }

    pub fn apply(
        &mut self,
        command: AuthoringDocumentCommand,
    ) -> Result<AuthoringDocumentCommandOutcome, String> {
        if matches!(
            command,
            AuthoringDocumentCommand::RevertLast | AuthoringDocumentCommand::UndoLast
        ) {
            return self.revert_last(command);
        }
        if matches!(command, AuthoringDocumentCommand::RedoLast) {
            return self.redo_last();
        }
        self.apply_forward(command, None)
    }

    fn apply_forward(
        &mut self,
        command: AuthoringDocumentCommand,
        redo_index: Option<usize>,
    ) -> Result<AuthoringDocumentCommandOutcome, String> {
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
        let details = match &command {
            AuthoringDocumentCommand::Graph(command) => metadata.graph_details(command),
            AuthoringDocumentCommand::ReplaceGraph { details, .. } => details.clone(),
            _ => format!("authoring document command applied: {command:?}"),
        };
        let operation = metadata
            .into_operation(redo_index.map(|_| OperationKind::Redo), details)
            .with_before_after_fingerprints(&before_fingerprint, &after_fingerprint);
        let verification = VerificationRun::from_diagnostics(
            operation.operation_id.clone(),
            "document_command_bus",
            &after_fingerprint,
            &before_issues,
            &after_issues,
        );

        self.undo_deltas.push((
            delta.clone(),
            redo_index.unwrap_or(self.recorded_commands.len()),
        ));
        self.redo_command_indices.clear();
        self.recorded_commands.push(if redo_index.is_some() {
            AuthoringDocumentCommand::RedoLast
        } else {
            command
        });
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

    fn revert_last(
        &mut self,
        command: AuthoringDocumentCommand,
    ) -> Result<AuthoringDocumentCommandOutcome, String> {
        let (delta, command_index) = self
            .undo_deltas
            .pop()
            .ok_or_else(|| "no document command delta to revert".to_string())?;
        let before_fingerprint = self.current_fingerprint();
        let before_issues = self.current_validation();
        if let Err(error) = self.apply_inverse_delta(&delta) {
            self.undo_deltas.push((delta, command_index));
            return Err(error);
        }
        let reverted = AuthoringDocumentDelta::Reverted {
            reverted: Box::new(delta),
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
            if matches!(command, AuthoringDocumentCommand::UndoLast) {
                OperationKind::Undo
            } else {
                OperationKind::Revert
            },
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
        self.redo_command_indices.push(command_index);
        self.recorded_commands.push(command);
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

    fn redo_last(&mut self) -> Result<AuthoringDocumentCommandOutcome, String> {
        let index = self
            .redo_command_indices
            .pop()
            .ok_or_else(|| "no document command to redo".to_string())?;
        // A successful forward command clears the redo branch. Preserve its
        // remaining entries while reapplying exactly one command.
        let pending = std::mem::take(&mut self.redo_command_indices);
        let result = self.apply_forward(self.recorded_commands[index].clone(), Some(index));
        self.redo_command_indices = pending;
        if result.is_err() {
            self.redo_command_indices.push(index);
        }
        result
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
        | AuthoringDocumentDelta::BackgroundFitCleared { .. }
        | AuthoringDocumentDelta::ComposerOverridesReset { .. } => true,
        AuthoringDocumentDelta::Graph(_) | AuthoringDocumentDelta::GraphReplaced { .. } => false,
        AuthoringDocumentDelta::Reverted { reverted } => {
            delta_preserves_semantic_fingerprint(reverted)
        }
    }
}
