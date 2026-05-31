//! Execution contract adapter shared by editor preview, runtime, and export.
//!
//! The source of truth for event/node behavior metadata lives in
//! `event_behavior`. This module preserves the existing public execution
//! contract API while projecting that broader contract into the legacy shape.

use crate::authoring::StoryNode;
use crate::event::EventRaw;
use crate::event_behavior::{
    event_spec_for_raw, node_spec_for_authoring_node, EventSpec, NodeKind, NodeSpec,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FidelityClass {
    RuntimeReal,
    HeadlessSimulated,
    HostRequired,
    PreviewOnly,
    FallbackDegraded,
}

impl FidelityClass {
    pub fn label(self) -> &'static str {
        match self {
            Self::RuntimeReal => "runtime_real",
            Self::HeadlessSimulated => "headless_simulated",
            Self::HostRequired => "host_required",
            Self::PreviewOnly => "preview_only",
            Self::FallbackDegraded => "fallback_degraded",
        }
    }
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

const CONTRACT_MATRIX: [EventExecutionContract; 16] = [
    contract_from_node_spec(NodeKind::Dialogue.spec()),
    contract_from_node_spec(NodeKind::Choice.spec()),
    contract_from_node_spec(NodeKind::Scene.spec()),
    contract_from_node_spec(NodeKind::Jump.spec()),
    contract_from_node_spec(NodeKind::SetVariable.spec()),
    contract_from_node_spec(NodeKind::SetFlag.spec()),
    contract_from_node_spec(NodeKind::ScenePatch.spec()),
    contract_from_node_spec(NodeKind::JumpIf.spec()),
    contract_from_node_spec(NodeKind::AudioAction.spec()),
    contract_from_node_spec(NodeKind::Transition.spec()),
    contract_from_node_spec(NodeKind::CharacterPlacement.spec()),
    contract_from_node_spec(NodeKind::ExtCall.spec()),
    contract_from_node_spec(NodeKind::SubgraphCall.spec()),
    contract_from_node_spec(NodeKind::GenericEvent.spec()),
    contract_from_node_spec(NodeKind::Start.spec()),
    contract_from_node_spec(NodeKind::End.spec()),
];

pub fn contract_matrix() -> &'static [EventExecutionContract] {
    &CONTRACT_MATRIX
}

pub fn contract_for_authoring_node(node: &StoryNode) -> EventExecutionContract {
    contract_from_node_spec(*node_spec_for_authoring_node(node))
}

pub fn contract_for_event_raw(event: &EventRaw) -> EventExecutionContract {
    contract_from_event_spec(*event_spec_for_raw(event))
}

pub fn headless_fidelity_for_event_raw(event: &EventRaw) -> FidelityClass {
    event_spec_for_raw(event).capabilities.fidelity
}

pub fn is_preview_only_authoring_node(node: &StoryNode) -> bool {
    matches!(
        contract_for_authoring_node(node).fidelity,
        FidelityClass::PreviewOnly
    )
}

const fn contract_from_event_spec(spec: EventSpec) -> EventExecutionContract {
    EventExecutionContract {
        event_name: spec.contract_name,
        editor_supported: spec.capabilities.editor_supported,
        preview_supported: spec.capabilities.preview_supported,
        runtime_supported: spec.capabilities.runtime_supported,
        export_supported: spec.capabilities.export_supported,
        fidelity: spec.capabilities.fidelity,
    }
}

const fn contract_from_node_spec(spec: NodeSpec) -> EventExecutionContract {
    EventExecutionContract {
        event_name: spec.contract_name,
        editor_supported: spec.capabilities.editor_supported,
        preview_supported: spec.capabilities.preview_supported,
        runtime_supported: spec.capabilities.runtime_supported,
        export_supported: spec.capabilities.export_supported,
        fidelity: spec.capabilities.fidelity,
    }
}
