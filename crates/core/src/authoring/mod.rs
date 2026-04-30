//! Headless authoring model shared by GUI, Python and CLI clients.
//!
//! This module keeps semantic story editing independent from egui/eframe.

pub mod compiler;
mod diagnostics;
mod document;
mod entry;
mod graph;
mod lint;
mod operation_log;
pub mod quick_fix;
mod report_fingerprint;
mod scene_profile;
mod script_sync;
mod types;
mod validation;
mod validation_report;

pub use diagnostics::{
    DiagnosticEnvelopeV2, DiagnosticExplanation, DiagnosticLanguage, DiagnosticLocation,
};
pub use document::{
    source_looks_like_authoring_document, AuthoringDocument, AuthoringDocumentError,
    AUTHORING_DOCUMENT_SCHEMA_VERSION,
};
pub use entry::{
    export_runtime_script_from_authoring, load_authoring_document_or_script,
    load_runtime_script_from_entry, parse_authoring_document_or_script,
    parse_runtime_script_from_entry,
};
pub use graph::{CharacterPoseBinding, GraphConnection, NodeGraph, SceneLayer, SceneProfile};
pub use lint::{LintCode, LintIssue, LintSeverity, ValidationPhase};
pub use operation_log::{OperationLogEntry, VerificationRun};
pub use quick_fix::{QuickFixCandidate, QuickFixRisk};
pub use report_fingerprint::{
    authoring_fingerprints_semantically_match, authoring_graph_sha256,
    build_authoring_report_fingerprint, AuthoringReportBuildInfo, AuthoringReportFingerprint,
    AuthoringSemanticFingerprint,
};
pub use types::{AuthoringPosition, StoryNode, NODE_VERTICAL_SPACING};
pub use validation::{
    asset_exists_from_project_root, default_asset_exists, is_unsafe_asset_ref,
    should_probe_asset_exists, validate as validate_authoring_graph,
    validate_no_io as validate_authoring_graph_no_io,
    validate_with_asset_probe as validate_authoring_graph_with_probe,
    validate_with_asset_resolver as validate_authoring_graph_with_resolver,
    validate_with_project_root as validate_authoring_graph_with_project_root,
};
pub use validation_report::AuthoringValidationReport;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
#[cfg(test)]
#[path = "traceability_tests.rs"]
mod traceability_tests;
