use serde::Serialize;
use visual_novel_engine::authoring::AuthoringValidationReport;

pub(super) fn print_report(report: &AuthoringValidationReport) {
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

#[derive(Serialize)]
pub(super) struct ReportCompareSummary {
    pub(super) before_issue_count: usize,
    pub(super) after_issue_count: usize,
    pub(super) before_error_count: usize,
    pub(super) after_error_count: usize,
    pub(super) semantic_changed: bool,
    pub(super) layout_changed: bool,
    pub(super) assets_changed: bool,
}

pub(super) fn sarif_from_report(report: &AuthoringValidationReport) -> serde_json::Value {
    let results = report
        .issues
        .iter()
        .map(|issue| {
            serde_json::json!({
                "ruleId": issue.code,
                "level": sarif_level(&issue.severity),
                "message": { "text": issue.text_en.actual },
                "partialFingerprints": {
                    "diagnosticId": issue.diagnostic_id,
                    "traceId": issue.trace_id,
                    "storySemanticSha256": report.fingerprints.story_semantic_sha256,
                },
                "properties": {
                    "target": issue.target,
                    "fieldPath": issue.field_path,
                    "typedMessageArgs": issue.typed_message_args,
                    "evidenceTrace": issue.evidence_trace,
                }
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "vnengine authoring",
                    "informationUri": "https://github.com/Kitsunp/rustnovel"
                }
            },
            "results": results
        }]
    })
}

fn sarif_level(severity: &str) -> &'static str {
    match severity.to_ascii_lowercase().as_str() {
        "error" => "error",
        "warning" => "warning",
        _ => "note",
    }
}
