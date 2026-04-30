use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::{AuthoringReportFingerprint, DiagnosticTarget, FieldPath, LintIssue};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationLogEntry {
    pub schema: String,
    pub operation_id: String,
    pub created_unix_ms: u64,
    pub operation_kind: String,
    pub diagnostic_id: Option<String>,
    pub semantic_fingerprint_sha256: Option<String>,
    #[serde(default)]
    pub before_fingerprint_sha256: Option<String>,
    #[serde(default)]
    pub after_fingerprint_sha256: Option<String>,
    #[serde(default)]
    pub field_paths: Vec<FieldPath>,
    #[serde(default)]
    pub diagnostic_target: Option<DiagnosticTarget>,
    #[serde(default)]
    pub before_value: Option<String>,
    #[serde(default)]
    pub after_value: Option<String>,
    pub status: String,
    pub details: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationRun {
    pub schema: String,
    pub operation_id: String,
    pub created_unix_ms: u64,
    pub validation_profile: String,
    pub semantic_fingerprint_sha256: String,
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
        let resolved_diagnostic_ids = before_ids
            .difference(&after_ids)
            .cloned()
            .collect::<Vec<_>>();
        let introduced_diagnostic_ids = after_ids
            .difference(&before_ids)
            .cloned()
            .collect::<Vec<_>>();

        Self {
            schema: "vnengine.verification_run.v1".to_string(),
            operation_id: operation_id.into(),
            created_unix_ms: now_unix_ms(),
            validation_profile: validation_profile.into(),
            semantic_fingerprint_sha256: fingerprint.story_semantic_sha256.clone(),
            diagnostic_ids: after_ids.into_iter().collect(),
            resolved_diagnostic_ids,
            introduced_diagnostic_ids,
        }
    }
}

impl OperationLogEntry {
    pub fn new(
        operation_id: impl Into<String>,
        operation_kind: impl Into<String>,
        status: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self {
            schema: "vnengine.operation_log.v1".to_string(),
            operation_id: operation_id.into(),
            created_unix_ms: now_unix_ms(),
            operation_kind: operation_kind.into(),
            diagnostic_id: None,
            semantic_fingerprint_sha256: None,
            before_fingerprint_sha256: None,
            after_fingerprint_sha256: None,
            field_paths: Vec::new(),
            diagnostic_target: None,
            before_value: None,
            after_value: None,
            status: status.into(),
            details: details.into(),
        }
    }

    pub fn with_diagnostic(mut self, issue: &LintIssue) -> Self {
        self.diagnostic_id = Some(issue.diagnostic_id());
        self.diagnostic_target = issue.target.clone();
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

    pub fn with_values(mut self, before: impl Into<String>, after: impl Into<String>) -> Self {
        self.before_value = Some(before.into());
        self.after_value = Some(after.into());
        self
    }
}

fn diagnostic_id_set(issues: &[LintIssue]) -> BTreeSet<String> {
    issues.iter().map(LintIssue::diagnostic_id).collect()
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
