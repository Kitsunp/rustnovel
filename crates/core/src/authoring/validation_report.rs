use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ScriptRaw;

use super::{
    build_authoring_document_report_fingerprint, build_authoring_report_fingerprint,
    AuthoringDocument, AuthoringReportBuildInfo, AuthoringReportFingerprint,
    AuthoringSemanticFingerprint, DiagnosticEnvelopeV2, DiagnosticExplanation, DiagnosticLocation,
    DiagnosticTarget, FieldPath, LintCode, LintIssue, LintSeverity, NodeGraph, ValidationPhase,
};

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
            schema: "vnengine.authoring_validation_report.v2".to_string(),
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
        if value
            .get("schema")
            .and_then(Value::as_str)
            .is_some_and(|schema| schema == "vnengine.authoring_validation_report.v2")
        {
            return serde_json::from_value(value);
        }
        if value
            .get("schema")
            .and_then(Value::as_str)
            .is_some_and(|schema| schema == "vneditor.diagnostic_report.v1")
        {
            return Ok(Self::from_legacy_v1(value));
        }
        serde_json::from_value(value)
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn explain(&self, diagnostic_id: &str) -> Option<&DiagnosticEnvelopeV2> {
        self.issues
            .iter()
            .find(|issue| issue.diagnostic_id == diagnostic_id)
    }

    fn from_legacy_v1(value: Value) -> Self {
        let issues = value
            .get("issues")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .enumerate()
                    .map(legacy_issue_to_envelope)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let error_count = issues
            .iter()
            .filter(|issue| issue.severity.eq_ignore_ascii_case("error"))
            .count();
        let warning_count = issues
            .iter()
            .filter(|issue| issue.severity.eq_ignore_ascii_case("warning"))
            .count();
        let info_count = issues.len().saturating_sub(error_count + warning_count);
        Self {
            schema: "vnengine.authoring_validation_report.v2".to_string(),
            issue_count: issues.len(),
            error_count,
            warning_count,
            info_count,
            fingerprints: legacy_untrusted_fingerprint(),
            issues,
        }
    }
}

fn legacy_issue_to_envelope((index, issue): (usize, &Value)) -> DiagnosticEnvelopeV2 {
    let code = issue
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("VAL_GENERIC_UNCHECKED");
    let severity = issue
        .get("severity")
        .and_then(Value::as_str)
        .unwrap_or("warning")
        .to_ascii_lowercase();
    let phase = issue
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("GRAPH");
    let message_en = issue
        .get("message_en")
        .or_else(|| issue.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("Legacy diagnostic")
        .to_string();
    let message_es = issue
        .get("message_es")
        .or_else(|| issue.get("message"))
        .and_then(Value::as_str)
        .unwrap_or(message_en.as_str())
        .to_string();
    let node_id = issue
        .get("node_id")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let event_ip = issue
        .get("event_ip")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let edge_from = issue
        .get("edge_from")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let edge_to = issue
        .get("edge_to")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let asset_path = issue
        .get("asset_path")
        .and_then(Value::as_str)
        .map(str::to_string);
    let field_path = issue
        .get("field_path")
        .and_then(Value::as_str)
        .map(FieldPath::new);
    let target = node_id
        .map(|node_id| DiagnosticTarget::Node { node_id })
        .or_else(|| {
            asset_path
                .as_ref()
                .map(|asset_path| DiagnosticTarget::AssetRef {
                    node_id,
                    field_path: field_path
                        .clone()
                        .unwrap_or_else(|| FieldPath::new("legacy.asset_path")),
                    asset_path: asset_path.clone(),
                })
        });
    let mut message_args = BTreeMap::new();
    message_args.insert("message".to_string(), message_en.clone());
    message_args.insert(
        "legacy_schema".to_string(),
        "vneditor.diagnostic_report.v1".to_string(),
    );
    if let Some(asset_path) = &asset_path {
        message_args.insert("asset_path".to_string(), asset_path.clone());
    }
    if let Some(node_id) = node_id {
        message_args.insert("node_id".to_string(), node_id.to_string());
    }
    let docs_ref = LintCode::from_label(code)
        .map(|code| {
            LintIssue::warning(None, ValidationPhase::Graph, code, "")
                .explanation(super::DiagnosticLanguage::En)
                .docs_ref
        })
        .unwrap_or_else(|| "docs/diagnostics/authoring.md#val-generic-unchecked".to_string());
    DiagnosticEnvelopeV2 {
        schema: "vnengine.diagnostic_envelope.v2".to_string(),
        diagnostic_id: format!("legacy-v1:{code}:{index}"),
        rule_version: "legacy-v1-adapter".to_string(),
        message_key: format!("legacy.{code}"),
        typed_message_args: message_args.clone(),
        message_args,
        severity,
        phase: phase.to_string(),
        code: code.to_string(),
        location: DiagnosticLocation {
            node_id,
            event_ip,
            edge_from,
            edge_to,
            asset_path,
            blocked_by: None,
            field_path: field_path.clone(),
            target: target.clone(),
        },
        target,
        field_path,
        semantic_values: Vec::new(),
        evidence_trace: None,
        trace_id: None,
        operation_id: None,
        blocked_by: None,
        docs_ref,
        text_es: legacy_explanation(message_es),
        text_en: legacy_explanation(message_en),
    }
}

fn legacy_explanation(actual: String) -> DiagnosticExplanation {
    DiagnosticExplanation {
        title: "Legacy diagnostic".to_string(),
        what_happened: "Imported from a v1 GUI diagnostic report".to_string(),
        root_cause: "The original report predates diagnostic evidence v2".to_string(),
        why_failed: "The legacy payload did not carry typed evidence".to_string(),
        consequence: "Automatic fixes must treat this diagnostic as untrusted".to_string(),
        how_to_fix: "Re-run validation to regenerate a v2 report".to_string(),
        action_steps: vec!["Re-run authoring validation".to_string()],
        expected: "v2 diagnostic evidence".to_string(),
        actual,
        docs_ref: "docs/diagnostics/authoring.md#val-generic-unchecked".to_string(),
        message_key: "legacy.diagnostic".to_string(),
    }
}

fn legacy_untrusted_fingerprint() -> AuthoringReportFingerprint {
    let semantic = AuthoringSemanticFingerprint {
        script_sha256: "legacy-untrusted".to_string(),
        graph_sha256: "legacy-untrusted".to_string(),
        story_graph_sha256: "legacy-untrusted".to_string(),
        asset_refs_sha256: "legacy-untrusted".to_string(),
        asset_refs_count: 0,
    };
    AuthoringReportFingerprint {
        fingerprint_schema_version: "vnengine.authoring.fingerprint.v2".to_string(),
        authoring_schema_version: "legacy-v1".to_string(),
        story_semantic_sha256: "legacy-untrusted".to_string(),
        layout_sha256: "legacy-untrusted".to_string(),
        assets_sha256: "legacy-untrusted".to_string(),
        full_document_sha256: "legacy-untrusted".to_string(),
        semantic_sha256: "legacy-untrusted".to_string(),
        script_sha256: semantic.script_sha256.clone(),
        graph_sha256: semantic.graph_sha256.clone(),
        asset_refs_sha256: semantic.asset_refs_sha256.clone(),
        asset_refs_count: semantic.asset_refs_count,
        semantic,
        build: AuthoringReportBuildInfo {
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            build_profile: "legacy-import".to_string(),
            target_os: std::env::consts::OS.to_string(),
            target_arch: std::env::consts::ARCH.to_string(),
        },
    }
}
