//! GUI adapter for the core execution contract matrix.

use crate::editor::node_types::StoryNode;

pub use visual_novel_engine::runtime::{
    contract_for_event_raw, contract_matrix, EventExecutionContract, FidelityClass,
};

pub fn contract_for_node(node: &StoryNode) -> EventExecutionContract {
    visual_novel_engine::runtime::contract_for_authoring_node(node)
}

pub fn is_preview_only_node(node: &StoryNode) -> bool {
    matches!(contract_for_node(node).fidelity, FidelityClass::PreviewOnly)
}
