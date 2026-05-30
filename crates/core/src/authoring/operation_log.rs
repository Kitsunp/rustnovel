use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::clock::now_unix_ms;

use super::{AuthoringReportFingerprint, DiagnosticTarget, FieldPath, LintIssue};

pub const OPERATION_LOG_SCHEMA_V2: &str = "vnengine.operation_log.v2";
pub const VERIFICATION_RUN_SCHEMA_V2: &str = "vnengine.verification_run.v2";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    NodeCreated,
    NodeRemoved,
    NodeMoved,
    NodeConnected,
    NodeDisconnected,
    FieldEdited,
    AssetImported,
    AssetRemoved,
    ComposerObjectMoved,
    LayerVisibilityChanged,
    LayerLockChanged,
    FragmentCreated,
    FragmentRemoved,
    FragmentEntered,
    FragmentLeft,
    SubgraphCallEdited,
    QuickFixApplied,
    Undo,
    Redo,
    Revert,
    ReportImported,
}

impl OperationKind {
    pub fn label(&self) -> String {
        match self {
            Self::NodeCreated => "node_created",
            Self::NodeRemoved => "node_removed",
            Self::NodeMoved => "node_moved",
            Self::NodeConnected => "node_connected",
            Self::NodeDisconnected => "node_disconnected",
            Self::FieldEdited => "field_edited",
            Self::AssetImported => "asset_imported",
            Self::AssetRemoved => "asset_removed",
            Self::ComposerObjectMoved => "composer_object_moved",
            Self::LayerVisibilityChanged => "layer_visibility_changed",
            Self::LayerLockChanged => "layer_lock_changed",
            Self::FragmentCreated => "fragment_created",
            Self::FragmentRemoved => "fragment_removed",
            Self::FragmentEntered => "fragment_entered",
            Self::FragmentLeft => "fragment_left",
            Self::SubgraphCallEdited => "subgraph_call_edited",
            Self::QuickFixApplied => "quick_fix_applied",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Revert => "revert",
            Self::ReportImported => "report_imported",
        }
        .to_string()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Applied,
    AppliedWithWarnings,
    Rejected,
    Failed,
    NoOp,
    Stale,
}

impl OperationStatus {
    pub fn label(&self) -> String {
        match self {
            Self::Applied => "applied",
            Self::AppliedWithWarnings => "applied_with_warnings",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
            Self::NoOp => "no_op",
            Self::Stale => "stale",
        }
        .to_string()
    }

    pub fn from_label(value: &str) -> Option<Self> {
        match value {
            "applied" => Some(Self::Applied),
            "applied_with_warnings" => Some(Self::AppliedWithWarnings),
            "rejected" => Some(Self::Rejected),
            "failed" => Some(Self::Failed),
            "no_op" => Some(Self::NoOp),
            "stale" => Some(Self::Stale),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationLogEntry {
    pub schema: String,
    pub operation_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub author_id: Option<String>,
    pub created_unix_ms: u64,
    pub operation_kind: String,
    #[serde(default)]
    pub operation_kind_v2: Option<OperationKind>,
    pub diagnostic_id: Option<String>,
    #[serde(default)]
    pub repro_id: Option<String>,
    pub semantic_fingerprint_sha256: Option<String>,
    #[serde(default)]
    pub before_fingerprint_sha256: Option<String>,
    #[serde(default)]
    pub after_fingerprint_sha256: Option<String>,
    #[serde(default)]
    pub field_paths: Vec<FieldPath>,
    #[serde(default)]
    pub affected_targets: Vec<DiagnosticTarget>,
    #[serde(default)]
    pub diagnostic_target: Option<DiagnosticTarget>,
    #[serde(default)]
    pub before_value: Option<String>,
    #[serde(default)]
    pub after_value: Option<String>,
    pub status: String,
    pub status_v2: OperationStatus,
    pub details: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationRun {
    pub schema: String,
    pub operation_id: String,
    #[serde(default)]
    pub diagnostic_id: Option<String>,
    pub created_unix_ms: u64,
    pub validation_profile: String,
    pub semantic_fingerprint_sha256: String,
    #[serde(default)]
    pub story_semantic_sha256: String,
    #[serde(default)]
    pub layout_sha256: Option<String>,
    #[serde(default)]
    pub assets_sha256: Option<String>,
    #[serde(default)]
    pub full_document_sha256: Option<String>,
    pub diagnostic_ids: Vec<String>,
    pub resolved_diagnostic_ids: Vec<String>,
    pub introduced_diagnostic_ids: Vec<String>,
}

impl VerificationRun {
    pub fn from_diagnostics(
        operation_id: impl Into<String>,
        validation_profile: impl Into<String>,
        fingerprint: &AuthoringReportFingerprint,
        before: &[LintIssue],
        after: &[LintIssue],
    ) -> Self {
        let before_ids = diagnostic_id_set(before);
        let after_ids = diagnostic_id_set(after);
        Self::from_diagnostic_id_sets(
            operation_id,
            validation_profile,
            fingerprint,
            &before_ids,
            &after_ids,
        )
    }

    pub fn from_diagnostic_id_sets(
        operation_id: impl Into<String>,
        validation_profile: impl Into<String>,
        fingerprint: &AuthoringReportFingerprint,
        before_ids: &BTreeSet<String>,
        after_ids: &BTreeSet<String>,
    ) -> Self {
        let resolved_diagnostic_ids = before_ids
            .difference(after_ids)
            .cloned()
            .collect::<Vec<_>>();
        let introduced_diagnostic_ids = after_ids
            .difference(before_ids)
            .cloned()
            .collect::<Vec<_>>();

        Self {
            schema: VERIFICATION_RUN_SCHEMA_V2.to_string(),
            operation_id: operation_id.into(),
            diagnostic_id: None,
            created_unix_ms: now_unix_ms(),
            validation_profile: validation_profile.into(),
            semantic_fingerprint_sha256: fingerprint.story_semantic_sha256.clone(),
            story_semantic_sha256: fingerprint.story_semantic_sha256.clone(),
            layout_sha256: Some(fingerprint.layout_sha256.clone()),
            assets_sha256: Some(fingerprint.assets_sha256.clone()),
            full_document_sha256: Some(fingerprint.full_document_sha256.clone()),
            diagnostic_ids: after_ids.iter().cloned().collect(),
            resolved_diagnostic_ids,
            introduced_diagnostic_ids,
        }
    }
}

impl OperationLogEntry {
    pub fn new_typed(
        operation_kind: OperationKind,
        status: OperationStatus,
        details: impl Into<String>,
    ) -> Self {
        let label = operation_kind.label();
        let status_label = status.label();
        Self {
            schema: OPERATION_LOG_SCHEMA_V2.to_string(),
            operation_id: new_operation_id(),
            session_id: None,
            author_id: None,
            created_unix_ms: now_unix_ms(),
            operation_kind: label,
            operation_kind_v2: Some(operation_kind),
            diagnostic_id: None,
            repro_id: None,
            semantic_fingerprint_sha256: None,
            before_fingerprint_sha256: None,
            after_fingerprint_sha256: None,
            field_paths: Vec::new(),
            affected_targets: Vec::new(),
            diagnostic_target: None,
            before_value: None,
            after_value: None,
            status_v2: status,
            status: status_label,
            details: details.into(),
        }
    }

    pub fn new(
        operation_id: impl Into<String>,
        operation_kind: OperationKind,
        status: OperationStatus,
        details: impl Into<String>,
    ) -> Self {
        let label = operation_kind.label();
        let status_label = status.label();
        Self {
            schema: OPERATION_LOG_SCHEMA_V2.to_string(),
            operation_id: operation_id.into(),
            session_id: None,
            author_id: None,
            created_unix_ms: now_unix_ms(),
            operation_kind: label,
            operation_kind_v2: Some(operation_kind),
            diagnostic_id: None,
            repro_id: None,
            semantic_fingerprint_sha256: None,
            before_fingerprint_sha256: None,
            after_fingerprint_sha256: None,
            field_paths: Vec::new(),
            affected_targets: Vec::new(),
            diagnostic_target: None,
            before_value: None,
            after_value: None,
            status_v2: status,
            status: status_label,
            details: details.into(),
        }
    }

    pub fn with_diagnostic(mut self, issue: &LintIssue) -> Self {
        self.diagnostic_id = Some(issue.diagnostic_id());
        self.diagnostic_target = issue.target.clone();
        if let Some(target) = &issue.target {
            self.affected_targets.push(target.clone());
        }
        if let Some(field_path) = &issue.field_path {
            self.field_paths.push(field_path.clone());
        }
        self
    }

    pub fn with_fingerprint(mut self, fingerprint: &AuthoringReportFingerprint) -> Self {
        self.semantic_fingerprint_sha256 = Some(fingerprint.story_semantic_sha256.clone());
        self.after_fingerprint_sha256 = Some(fingerprint.full_document_sha256.clone());
        self
    }

    pub fn with_before_after_fingerprints(
        mut self,
        before: &AuthoringReportFingerprint,
        after: &AuthoringReportFingerprint,
    ) -> Self {
        self.semantic_fingerprint_sha256 = Some(after.story_semantic_sha256.clone());
        self.before_fingerprint_sha256 = Some(before.full_document_sha256.clone());
        self.after_fingerprint_sha256 = Some(after.full_document_sha256.clone());
        self
    }

    pub fn with_field_path(mut self, field_path: impl Into<String>) -> Self {
        self.field_paths.push(FieldPath::new(field_path));
        self
    }

    pub fn with_target(mut self, target: DiagnosticTarget) -> Self {
        self.affected_targets.push(target);
        self
    }

    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_author(mut self, author_id: impl Into<String>) -> Self {
        self.author_id = Some(author_id.into());
        self
    }

    pub fn with_repro(mut self, repro_id: impl Into<String>) -> Self {
        self.repro_id = Some(repro_id.into());
        self
    }

    pub fn with_values(mut self, before: impl Into<String>, after: impl Into<String>) -> Self {
        self.before_value = Some(before.into());
        self.after_value = Some(after.into());
        self
    }

    pub fn with_status(mut self, status: OperationStatus) -> Self {
        self.status = status.label();
        self.status_v2 = status;
        self
    }
}

fn diagnostic_id_set(issues: &[LintIssue]) -> BTreeSet<String> {
    issues.iter().map(LintIssue::diagnostic_id).collect()
}

fn new_operation_id() -> String {
    format!("op:{}", uuid::Uuid::new_v4())
}
