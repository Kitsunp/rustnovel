//! GUI adapter for the core execution contract matrix.

use crate::editor::node_types::StoryNode;

pub use visual_novel_engine::{
    contract_for_event_raw, contract_matrix, EventExecutionContract, FidelityClass,
};

pub fn contract_for_node(node: &StoryNode) -> EventExecutionContract {
    visual_novel_engine::contract_for_authoring_node(node)
}

pub fn is_preview_only_node(node: &StoryNode) -> bool {
    matches!(contract_for_node(node).fidelity, FidelityClass::PreviewOnly)
}

#[cfg(test)]
mod tests {
    use super::*;
    use visual_novel_engine::{DialogueRaw, EventRaw};

    #[test]
    fn matrix_contains_preview_and_runtime_contracts() {
        assert!(contract_matrix()
            .iter()
            .any(|entry| entry.fidelity == FidelityClass::PreviewOnly));
        assert!(contract_matrix()
            .iter()
            .any(|entry| entry.fidelity == FidelityClass::RuntimeReal));
    }

    #[test]
    fn story_markers_are_preview_only() {
        assert!(is_preview_only_node(&StoryNode::Start));
        assert!(is_preview_only_node(&StoryNode::End));
    }

    #[test]
    fn raw_dialogue_is_runtime_real() {
        let contract = contract_for_event_raw(&EventRaw::Dialogue(DialogueRaw {
            speaker: "A".to_string(),
            text: "B".to_string(),
        }));
        assert_eq!(contract.event_name, "Dialogue");
        assert_eq!(contract.fidelity, FidelityClass::RuntimeReal);
        assert!(contract.export_supported);
    }

    #[test]
    fn extcall_generic_node_is_export_supported() {
        let contract = contract_for_node(&StoryNode::Generic(EventRaw::ExtCall {
            command: "hook".to_string(),
            args: vec!["x".to_string()],
        }));
        assert!(contract.export_supported);
        assert_eq!(contract.fidelity, FidelityClass::RuntimeReal);
    }
}
