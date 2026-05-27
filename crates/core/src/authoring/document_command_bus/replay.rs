use super::super::{AuthoringDocument, NodeGraph};
use super::{AuthoringDocumentCommand, AuthoringDocumentCommandBus};

impl AuthoringDocumentCommandBus {
    pub fn replay(commands: &[AuthoringDocumentCommand]) -> Result<Self, String> {
        let mut bus = Self::new(AuthoringDocument::new(NodeGraph::new()));
        for command in commands {
            bus.apply(command.clone())?;
        }
        Ok(bus)
    }
}
