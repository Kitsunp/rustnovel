use visual_novel_engine::authoring::{
    AuthoringCommand, AuthoringCommandBus, AuthoringCommandOutcome, AuthoringDocument,
    AuthoringDocumentCommand, AuthoringDocumentCommandBus, AuthoringDocumentCommandOutcome,
};

use super::PyNodeGraph;

impl PyNodeGraph {
    pub(crate) fn inner(&self) -> &visual_novel_engine::authoring::NodeGraph {
        &self.inner
    }

    pub(super) fn to_authoring_document(&self) -> AuthoringDocument {
        let mut document = AuthoringDocument::new(self.inner.clone());
        document.composer_layer_overrides = self.layer_overrides.clone();
        document.operation_log = self.operation_log.clone();
        document.verification_runs = self.verification_runs.clone();
        document
    }

    pub(super) fn apply_authoring_command(
        &mut self,
        command: AuthoringCommand,
    ) -> Result<AuthoringCommandOutcome, String> {
        let mut bus = AuthoringCommandBus::with_history(
            self.inner.clone(),
            self.operation_log.clone(),
            self.verification_runs.clone(),
        );
        let outcome = bus.apply(command)?;
        let (graph, operation_log, verification_runs) = bus.into_parts();
        self.inner = graph;
        self.operation_log = operation_log;
        self.verification_runs = verification_runs;
        Ok(outcome)
    }

    pub(super) fn apply_document_command(
        &mut self,
        command: AuthoringDocumentCommand,
    ) -> Result<AuthoringDocumentCommandOutcome, String> {
        let mut bus = AuthoringDocumentCommandBus::new(self.to_authoring_document());
        let outcome = bus.apply(command)?;
        let document = bus.into_document();
        self.inner = document.graph;
        self.layer_overrides = document.composer_layer_overrides;
        self.operation_log = document.operation_log;
        self.verification_runs = document.verification_runs;
        Ok(outcome)
    }
}
