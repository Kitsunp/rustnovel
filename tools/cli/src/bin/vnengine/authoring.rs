use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use visual_novel_engine::authoring::{
    build_authoring_report_fingerprint, load_authoring_document_or_script,
    validate_authoring_graph_with_project_root, AuthoringReportFingerprint, LintIssue,
    LintSeverity, NodeGraph as AuthoringGraph,
};
use visual_novel_engine::ScriptRaw;

#[derive(Serialize)]
struct AuthoringValidationReport {
    issue_count: usize,
    error_count: usize,
    warning_count: usize,
    info_count: usize,
    fingerprints: AuthoringReportFingerprint,
    issues: Vec<AuthoringIssueReport>,
}

#[derive(Serialize)]
struct AuthoringIssueReport {
    diagnostic_id: String,
    severity: String,
    phase: String,
    code: String,
    message: String,
    node_id: Option<u32>,
    event_ip: Option<u32>,
    edge_from: Option<u32>,
    edge_to: Option<u32>,
    blocked_by: Option<String>,
    asset_path: Option<String>,
}

pub fn validate_authoring_script(
    path: &Path,
    project_root: Option<&Path>,
    output: Option<&Path>,
) -> Result<()> {
    let graph = load_authoring_document_or_script(path).context("load authoring/script entry")?;
    let script = graph.to_script_lossy_for_diagnostics();
    let project_root = project_root
        .or_else(|| path.parent())
        .unwrap_or_else(|| Path::new("."));
    let issues = validate_authoring_graph_with_project_root(&graph, project_root);
    let report = AuthoringValidationReport::from_graph_and_issues(&graph, &script, &issues);

    if let Some(output) = output {
        write_report(output, &report)?;
    } else {
        print_report(&report);
    }

    if report.error_count > 0 {
        anyhow::bail!(
            "authoring validation failed with {} error(s)",
            report.error_count
        );
    }
    Ok(())
}

impl AuthoringValidationReport {
    fn from_graph_and_issues(
        graph: &AuthoringGraph,
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
            issue_count: issues.len(),
            error_count,
            warning_count,
            info_count,
            fingerprints: build_authoring_report_fingerprint(graph, script),
            issues: issues.iter().map(AuthoringIssueReport::from).collect(),
        }
    }
}

impl From<&LintIssue> for AuthoringIssueReport {
    fn from(issue: &LintIssue) -> Self {
        Self {
            diagnostic_id: issue.diagnostic_id(),
            severity: issue.severity.label().to_string(),
            phase: issue.phase.label().to_string(),
            code: issue.code.label().to_string(),
            message: issue.message.clone(),
            node_id: issue.node_id,
            event_ip: issue.event_ip,
            edge_from: issue.edge_from,
            edge_to: issue.edge_to,
            blocked_by: issue.blocked_by.clone(),
            asset_path: issue.asset_path.clone(),
        }
    }
}

fn write_report(output: &Path, report: &AuthoringValidationReport) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(output, json).with_context(|| format!("write {}", output.display()))?;
    Ok(())
}

fn print_report(report: &AuthoringValidationReport) {
    println!(
        "authoring validation => issues={} errors={} warnings={} infos={}",
        report.issue_count, report.error_count, report.warning_count, report.info_count
    );
    for issue in &report.issues {
        println!(
            "{} [{}:{}] {}",
            issue.diagnostic_id, issue.severity, issue.code, issue.message
        );
    }
}
