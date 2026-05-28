use visual_novel_engine::authoring::{
    AuthoringCommand, AuthoringCommandOutcome, AuthoringDocument, AuthoringDocumentCommand,
    AuthoringDocumentCommandOutcome, AuthoringDocumentDelta, AuthoringDocumentSession,
};

use super::PyNodeGraph;

impl PyNodeGraph {
    pub(super) fn from_authoring_document(document: AuthoringDocument) -> Self {
        Self {
            inner: document.graph.clone(),
            layer_overrides: document.composer_layer_overrides.clone(),
            background_fit_overrides: document.composer_background_fit_overrides.clone(),
            operation_log: document.operation_log.clone(),
            verification_runs: document.verification_runs.clone(),
            session: AuthoringDocumentSession::new(document),
        }
    }

    pub(crate) fn inner(&self) -> &visual_novel_engine::authoring::NodeGraph {
        &self.inner
    }

    pub(super) fn to_authoring_document(&self) -> AuthoringDocument {
        self.session.document().clone()
    }

    pub(super) fn fields_to_authoring_document(&self) -> AuthoringDocument {
        let mut document = AuthoringDocument::new(self.inner.clone());
        document.composer_layer_overrides = self.layer_overrides.clone();
        document.composer_background_fit_overrides = self.background_fit_overrides.clone();
        document.operation_log = self.operation_log.clone();
        document.verification_runs = self.verification_runs.clone();
        document
    }

    pub(super) fn rebuild_session_from_fields(&mut self) {
        self.session = AuthoringDocumentSession::new(self.fields_to_authoring_document());
    }

    fn sync_fields_from_session_document(&mut self) {
        let document = self.session.document().clone();
        self.inner = document.graph;
        self.layer_overrides = document.composer_layer_overrides;
        self.background_fit_overrides = document.composer_background_fit_overrides;
        self.operation_log = document.operation_log;
        self.verification_runs = document.verification_runs;
    }

    pub(super) fn apply_authoring_command(
        &mut self,
        command: AuthoringCommand,
    ) -> Result<AuthoringCommandOutcome, String> {
        let outcome = self
            .session
            .apply(AuthoringDocumentCommand::Graph(command))?;
        self.sync_fields_from_session_document();
        let AuthoringDocumentDelta::Graph(delta) = outcome.delta else {
            return Err("document session returned an unexpected graph delta".to_string());
        };
        Ok(AuthoringCommandOutcome {
            delta: *delta,
            operation: outcome.operation,
            verification: outcome.verification,
        })
    }

    pub(super) fn apply_document_command(
        &mut self,
        command: AuthoringDocumentCommand,
    ) -> Result<AuthoringDocumentCommandOutcome, String> {
        let outcome = self.session.apply(command)?;
        self.sync_fields_from_session_document();
        Ok(outcome)
    }
}
