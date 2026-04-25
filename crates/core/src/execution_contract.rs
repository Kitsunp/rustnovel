//! Execution contract matrix shared by editor preview, runtime, and export.

use crate::authoring::StoryNode;
use crate::EventRaw;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FidelityClass {
    PreviewOnly,
    RuntimeReal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventExecutionContract {
    pub event_name: &'static str,
    pub editor_supported: bool,
    pub preview_supported: bool,
    pub runtime_supported: bool,
    pub export_supported: bool,
    pub fidelity: FidelityClass,
}

const fn runtime_real(event_name: &'static str) -> EventExecutionContract {
    EventExecutionContract {
        event_name,
        editor_supported: true,
        preview_supported: true,
        runtime_supported: true,
        export_supported: true,
        fidelity: FidelityClass::RuntimeReal,
    }
}

const START_MARKER: EventExecutionContract = EventExecutionContract {
    event_name: "Start",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: false,
    export_supported: false,
    fidelity: FidelityClass::PreviewOnly,
};

const END_MARKER: EventExecutionContract = EventExecutionContract {
    event_name: "End",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: false,
    export_supported: false,
    fidelity: FidelityClass::PreviewOnly,
};

const DIALOGUE: EventExecutionContract = runtime_real("Dialogue");
const CHOICE: EventExecutionContract = runtime_real("Choice");
const SCENE: EventExecutionContract = runtime_real("Scene");
const JUMP: EventExecutionContract = runtime_real("Jump");
const SET_VAR: EventExecutionContract = runtime_real("SetVariable");
const SCENE_PATCH: EventExecutionContract = runtime_real("ScenePatch");
const JUMP_IF: EventExecutionContract = runtime_real("JumpIf");
const AUDIO_ACTION: EventExecutionContract = runtime_real("AudioAction");
const TRANSITION: EventExecutionContract = runtime_real("Transition");
const CHARACTER_PLACEMENT: EventExecutionContract = runtime_real("SetCharacterPosition");
const EXT_CALL: EventExecutionContract = runtime_real("ExtCall");

const GENERIC_EVENT: EventExecutionContract = EventExecutionContract {
    event_name: "Generic/EventRaw",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: false,
    export_supported: false,
    fidelity: FidelityClass::PreviewOnly,
};

const CONTRACT_MATRIX: [EventExecutionContract; 14] = [
    DIALOGUE,
    CHOICE,
    SCENE,
    JUMP,
    SET_VAR,
    SCENE_PATCH,
    JUMP_IF,
    AUDIO_ACTION,
    TRANSITION,
    CHARACTER_PLACEMENT,
    EXT_CALL,
    GENERIC_EVENT,
    START_MARKER,
    END_MARKER,
];

pub fn contract_matrix() -> &'static [EventExecutionContract] {
    &CONTRACT_MATRIX
}

pub fn contract_for_authoring_node(node: &StoryNode) -> EventExecutionContract {
    match node {
        StoryNode::Dialogue { .. } => DIALOGUE,
        StoryNode::Choice { .. } => CHOICE,
        StoryNode::Scene { .. } => SCENE,
        StoryNode::Jump { .. } => JUMP,
        StoryNode::SetVariable { .. } => SET_VAR,
        StoryNode::ScenePatch(_) => SCENE_PATCH,
        StoryNode::JumpIf { .. } => JUMP_IF,
        StoryNode::Start => START_MARKER,
        StoryNode::End => END_MARKER,
        StoryNode::AudioAction { .. } => AUDIO_ACTION,
        StoryNode::Transition { .. } => TRANSITION,
        StoryNode::CharacterPlacement { .. } => CHARACTER_PLACEMENT,
        StoryNode::Generic(EventRaw::ExtCall { .. }) => EXT_CALL,
        StoryNode::Generic(_) => GENERIC_EVENT,
    }
}

pub fn contract_for_event_raw(event: &EventRaw) -> EventExecutionContract {
    match event {
        EventRaw::Dialogue(_) => DIALOGUE,
        EventRaw::Choice(_) => CHOICE,
        EventRaw::Scene(_) => SCENE,
        EventRaw::Jump { .. } => JUMP,
        EventRaw::SetFlag { .. } | EventRaw::SetVar { .. } => SET_VAR,
        EventRaw::JumpIf { .. } => JUMP_IF,
        EventRaw::Patch(_) => SCENE_PATCH,
        EventRaw::ExtCall { .. } => EXT_CALL,
        EventRaw::AudioAction(_) => AUDIO_ACTION,
        EventRaw::Transition(_) => TRANSITION,
        EventRaw::SetCharacterPosition(_) => CHARACTER_PLACEMENT,
    }
}

pub fn is_preview_only_authoring_node(node: &StoryNode) -> bool {
    matches!(
        contract_for_authoring_node(node).fidelity,
        FidelityClass::PreviewOnly
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DialogueRaw, EventRaw};

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
        assert!(is_preview_only_authoring_node(&StoryNode::Start));
        assert!(is_preview_only_authoring_node(&StoryNode::End));
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
}
