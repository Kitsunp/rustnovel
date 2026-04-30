use serde::Serialize;

use crate::ScriptRaw;

use super::{
    build_authoring_report_fingerprint, AuthoringReportFingerprint, DiagnosticEnvelopeV2,
    LintIssue, LintSeverity, NodeGraph,
};

#[derive(Clone, Debug, Serialize)]
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
            schema: "vnengine.authoring_validation_report.v2".to_string(),
            issue_count: issues.len(),
            error_count,
            warning_count,
            info_count,
            fingerprints: build_authoring_report_fingerprint(graph, script),
            issues: issues.iter().map(LintIssue::envelope_v2).collect(),
        }
    }
}
