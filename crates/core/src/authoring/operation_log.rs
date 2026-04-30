use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::{AuthoringReportFingerprint, LintIssue};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationLogEntry {
    pub schema: String,
    pub operation_id: String,
    pub created_unix_ms: u64,
    pub operation_kind: String,
    pub diagnostic_id: Option<String>,
    pub semantic_fingerprint_sha256: Option<String>,
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
            semantic_fingerprint_sha256: fingerprint.semantic_sha256.clone(),
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
            status: status.into(),
            details: details.into(),
        }
    }

    pub fn with_diagnostic(mut self, issue: &LintIssue) -> Self {
        self.diagnostic_id = Some(issue.diagnostic_id());
        self
    }

    pub fn with_fingerprint(mut self, fingerprint: &AuthoringReportFingerprint) -> Self {
        self.semantic_fingerprint_sha256 = Some(fingerprint.semantic_sha256.clone());
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
