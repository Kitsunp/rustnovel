use visual_novel_engine::{
    authoring::StoryNode,
    runtime::{
        contract_for_authoring_node, contract_for_event_raw, contract_matrix, DialogueRaw,
        EventRaw, FidelityClass,
    },
};

#[test]
fn matrix_contains_preview_and_runtime_contracts() {
    assert!(contract_matrix()
        .iter()
        .any(|entry| entry.fidelity == FidelityClass::PreviewOnly));
    assert!(contract_matrix()
        .iter()
        .any(|entry| entry.fidelity == FidelityClass::RuntimeReal));
    assert!(contract_matrix()
        .iter()
        .any(|entry| entry.fidelity == FidelityClass::FallbackDegraded));
}

#[test]
fn story_markers_are_preview_only() {
    let start = contract_for_authoring_node(&StoryNode::Start);
    let end = contract_for_authoring_node(&StoryNode::End);
    assert_eq!(start.fidelity, FidelityClass::PreviewOnly);
    assert_eq!(end.fidelity, FidelityClass::PreviewOnly);
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
    let contract = contract_for_authoring_node(&StoryNode::Generic(EventRaw::ExtCall {
        command: "hook".to_string(),
        args: vec!["x".to_string()],
    }));
    assert!(contract.export_supported);
    assert_eq!(contract.fidelity, FidelityClass::RuntimeReal);
}
