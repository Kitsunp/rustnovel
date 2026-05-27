use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::script::ScriptRaw;

use super::{
    build_authoring_document_report_fingerprint, build_authoring_report_fingerprint,
    AuthoringDocument, AuthoringReportFingerprint, DiagnosticEnvelopeV2, LintIssue, LintSeverity,
    NodeGraph,
};

pub const AUTHORING_VALIDATION_REPORT_SCHEMA_V2: &str = "vnengine.authoring_validation_report.v2";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthoringValidationReport {
    pub schema: String,
    pub issue_count: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    pub fingerprints: AuthoringReportFingerprint,
    pub issues: Vec<DiagnosticEnvelopeV2>,
}

impl AuthoringValidationReport {
    pub fn from_graph_and_issues(
        graph: &NodeGraph,
        script: &ScriptRaw,
        issues: &[LintIssue],
    ) -> Self {
        let fingerprints = build_authoring_report_fingerprint(graph, script);
        Self::from_fingerprints_and_issues(fingerprints, issues)
    }

    pub fn from_document_and_issues(
        document: &AuthoringDocument,
        script: &ScriptRaw,
        issues: &[LintIssue],
    ) -> Self {
        let fingerprints = build_authoring_document_report_fingerprint(document, script);
        Self::from_fingerprints_and_issues(fingerprints, issues)
    }

    fn from_fingerprints_and_issues(
        fingerprints: AuthoringReportFingerprint,
        issues: &[LintIssue],
    ) -> Self {
        let error_count = issues
            .iter()
            .filter(|issue| issue.severity == LintSeverity::Error)
            .count();
        let warning_count = issues
            .iter()
            .filter(|issue| issue.severity == LintSeverity::Warning)
            .count();
        let info_count = issues
            .iter()
            .filter(|issue| issue.severity == LintSeverity::Info)
            .count();

        Self {
            schema: AUTHORING_VALIDATION_REPORT_SCHEMA_V2.to_string(),
            issue_count: issues.len(),
            error_count,
            warning_count,
            info_count,
            fingerprints,
            issues: issues.iter().map(LintIssue::envelope_v2).collect(),
        }
    }

    pub fn from_json(source: &str) -> serde_json::Result<Self> {
        let value: Value = serde_json::from_str(source)?;
        match value.get("schema").and_then(Value::as_str) {
            Some(AUTHORING_VALIDATION_REPORT_SCHEMA_V2) => serde_json::from_value(value),
            Some(schema) => Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unsupported authoring validation report schema '{schema}'"),
            ))),
            None => Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "missing authoring validation report schema",
            ))),
        }
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn explain(&self, diagnostic_id: &str) -> Option<&DiagnosticEnvelopeV2> {
        self.issues
            .iter()
            .find(|issue| issue.diagnostic_id == diagnostic_id)
    }
}
