use super::super::{AuthoringDocument, NodeGraph};
use super::{AuthoringDocumentCommand, AuthoringDocumentCommandBus, AuthoringDocumentSession};

impl AuthoringDocumentSession {
    pub fn replay(commands: &[AuthoringDocumentCommand]) -> Result<Self, String> {
        let mut session = Self::new(AuthoringDocument::new(NodeGraph::new()));
        for command in commands {
            session.apply(command.clone())?;
        }
        Ok(session)
    }
}

impl AuthoringDocumentCommandBus {
    pub fn replay(commands: &[AuthoringDocumentCommand]) -> Result<Self, String> {
        AuthoringDocumentSession::replay(commands).map(Self::from_session)
    }
}
