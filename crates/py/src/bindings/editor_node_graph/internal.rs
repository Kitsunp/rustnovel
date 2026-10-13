use visual_novel_engine::authoring::{
    AuthoringCommand, AuthoringCommandOutcome, AuthoringDocument, AuthoringDocumentCommand,
    AuthoringDocumentCommandOutcome, AuthoringDocumentDelta, AuthoringDocumentSession,
};

use super::PyNodeGraph;

impl PyNodeGraph {
    pub(super) fn from_authoring_document(document: AuthoringDocument) -> Self {
        Self {
            session: AuthoringDocumentSession::new(document),
        }
    }

    pub(super) fn doc(&self) -> &AuthoringDocument {
        self.session.document()
    }

    pub(crate) fn inner(&self) -> &visual_novel_engine::authoring::NodeGraph {
        &self.doc().graph
    }

    pub(super) fn apply_authoring_command(
        &mut self,
        command: AuthoringCommand,
    ) -> Result<AuthoringCommandOutcome, String> {
        let outcome = self
            .session
            .apply(AuthoringDocumentCommand::Graph(command))?;

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

        Ok(outcome)
    }
}
