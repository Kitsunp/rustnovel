//! Execution contract matrix for editor preview, runtime, and export parity.

use crate::editor::node_types::StoryNode;
use visual_novel_engine::EventRaw;

/// Classification of whether an event is only visible in editor/preview or real at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FidelityClass {
    PreviewOnly,
    RuntimeReal,
}

/// Compatibility contract per event family across editor, preview, runtime, and export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventExecutionContract {
    pub event_name: &'static str,
    pub editor_supported: bool,
    pub preview_supported: bool,
    pub runtime_supported: bool,
    pub export_supported: bool,
    pub fidelity: FidelityClass,
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

const DIALOGUE: EventExecutionContract = EventExecutionContract {
    event_name: "Dialogue",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: true,
    export_supported: true,
    fidelity: FidelityClass::RuntimeReal,
};

const CHOICE: EventExecutionContract = EventExecutionContract {
    event_name: "Choice",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: true,
    export_supported: true,
    fidelity: FidelityClass::RuntimeReal,
};

const SCENE: EventExecutionContract = EventExecutionContract {
    event_name: "Scene",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: true,
    export_supported: true,
    fidelity: FidelityClass::RuntimeReal,
};

const JUMP: EventExecutionContract = EventExecutionContract {
    event_name: "Jump",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: true,
    export_supported: true,
    fidelity: FidelityClass::RuntimeReal,
};

const SET_VAR: EventExecutionContract = EventExecutionContract {
    event_name: "SetVariable",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: true,
    export_supported: true,
    fidelity: FidelityClass::RuntimeReal,
};

const SCENE_PATCH: EventExecutionContract = EventExecutionContract {
    event_name: "ScenePatch",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: true,
    export_supported: true,
    fidelity: FidelityClass::RuntimeReal,
};

const JUMP_IF: EventExecutionContract = EventExecutionContract {
    event_name: "JumpIf",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: true,
    export_supported: true,
    fidelity: FidelityClass::RuntimeReal,
};

const AUDIO_ACTION: EventExecutionContract = EventExecutionContract {
    event_name: "AudioAction",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: true,
    export_supported: true,
    fidelity: FidelityClass::RuntimeReal,
};

const TRANSITION: EventExecutionContract = EventExecutionContract {
    event_name: "Transition",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: true,
    export_supported: true,
    fidelity: FidelityClass::RuntimeReal,
};

const CHARACTER_PLACEMENT: EventExecutionContract = EventExecutionContract {
    event_name: "SetCharacterPosition",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: true,
    export_supported: true,
    fidelity: FidelityClass::RuntimeReal,
};

const EXT_CALL: EventExecutionContract = EventExecutionContract {
    event_name: "ExtCall",
    editor_supported: true,
    preview_supported: true,
    runtime_supported: true,
    export_supported: true,
    fidelity: FidelityClass::RuntimeReal,
};

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

/// Returns the immutable compatibility matrix for editor/runtime/export.
pub fn contract_matrix() -> &'static [EventExecutionContract] {
    &CONTRACT_MATRIX
}

/// Returns the compatibility contract for a graph node.
pub fn contract_for_node(node: &StoryNode) -> EventExecutionContract {
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

/// Returns the compatibility contract for a raw script event.
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

/// Returns true when this node is preview/editor-only and not exported to runtime.
pub fn is_preview_only_node(node: &StoryNode) -> bool {
    matches!(contract_for_node(node).fidelity, FidelityClass::PreviewOnly)
}

#[cfg(test)]
mod tests {
    use super::*;
    use visual_novel_engine::{DialogueRaw, EventRaw};

    #[test]
    fn matrix_contains_preview_and_runtime_contracts() {
        let matrix = contract_matrix();
        assert!(matrix
            .iter()
            .any(|entry| entry.fidelity == FidelityClass::PreviewOnly));
        assert!(matrix
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
        let c = contract_for_event_raw(&EventRaw::Dialogue(DialogueRaw {
            speaker: "A".to_string(),
            text: "B".to_string(),
        }));
        assert_eq!(c.event_name, "Dialogue");
        assert_eq!(c.fidelity, FidelityClass::RuntimeReal);
        assert!(c.export_supported);
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

    #[test]
    fn unsupported_generic_node_remains_preview_only() {
        let contract = contract_for_node(&StoryNode::Generic(EventRaw::Jump {
            target: "node_1".to_string(),
        }));
        assert!(!contract.export_supported);
        assert_eq!(contract.fidelity, FidelityClass::PreviewOnly);
    }
}
