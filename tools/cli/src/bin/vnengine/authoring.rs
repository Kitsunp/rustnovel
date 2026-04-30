use std::path::Path;

use anyhow::{Context, Result};
use visual_novel_engine::authoring::{
    load_authoring_document_or_script, validate_authoring_graph_with_project_root,
    AuthoringValidationReport,
};

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
            issue.diagnostic_id, issue.severity, issue.code, issue.text_en.actual
        );
    }
}
