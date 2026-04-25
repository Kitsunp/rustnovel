//! Headless authoring model shared by GUI, Python and CLI clients.
//!
//! This module keeps semantic story editing independent from egui/eframe.

pub mod compiler;
mod diagnostics;
mod graph;
mod lint;
pub mod quick_fix;
mod scene_profile;
mod script_sync;
mod types;
mod validation;

pub use diagnostics::{DiagnosticExplanation, DiagnosticLanguage};
pub use graph::{CharacterPoseBinding, GraphConnection, NodeGraph, SceneLayer, SceneProfile};
pub use lint::{LintCode, LintIssue, LintSeverity, ValidationPhase};
pub use quick_fix::{QuickFixCandidate, QuickFixRisk};
pub use types::{AuthoringPosition, StoryNode, NODE_VERTICAL_SPACING};
pub use validation::{
    validate as validate_authoring_graph,
    validate_with_asset_probe as validate_authoring_graph_with_probe,
};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
