use super::*;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

impl EditorWorkbench {
    pub fn diagnostic_report_json(&self) -> Result<String, serde_json::Error> {
        let issues = self
            .validation_issues
            .iter()
            .map(|issue| {
                let es = issue.explanation(DiagnosticLanguage::Es);
                let en = issue.explanation(DiagnosticLanguage::En);
                json!({
                    "diagnostic_id": issue.diagnostic_id(),
                    "phase": issue.phase.label(),
                    "code": issue.code.label(),
                    "severity": issue.severity.label(),
                    "node_id": issue.node_id,
                    "event_ip": issue.event_ip,
                    "edge_from": issue.edge_from,
                    "edge_to": issue.edge_to,
                    "asset_path": issue.asset_path,
                    "message_es": issue.localized_message(DiagnosticLanguage::Es),
                    "message_en": issue.localized_message(DiagnosticLanguage::En),
                    "root_cause_es": es.root_cause,
                    "root_cause_en": en.root_cause,
                    "why_failed_es": es.why_failed,
                    "why_failed_en": en.why_failed,
                    "how_to_fix_es": es.how_to_fix,
                    "how_to_fix_en": en.how_to_fix,
                    "docs_ref": es.docs_ref,
                })
            })
            .collect::<Vec<_>>();

        let quick_fix_audit = self
            .quick_fix_audit
            .iter()
            .map(|entry| {
                json!({
                    "diagnostic_id": entry.diagnostic_id,
                    "fix_id": entry.fix_id,
                    "node_id": entry.node_id,
                    "event_ip": entry.event_ip,
                    "before_crc32": entry.before_crc32,
                    "after_crc32": entry.after_crc32,
                })
            })
            .collect::<Vec<_>>();

        let dry_run = self.last_dry_run_report.as_ref().map(|report| {
            json!({
                "max_steps": report.max_steps,
                "executed_steps": report.executed_steps,
                "stop_reason": report.stop_reason.label(),
                "stop_message": report.stop_message,
                "failing_event_ip": report.failing_event_ip,
                "steps": report
                    .steps
                    .iter()
                    .map(|step| {
                        json!({
                            "step": step.step,
                            "event_ip": step.event_ip,
                            "event_kind": step.event_kind,
                            "event_signature": step.event_signature,
                            "visual_background": step.visual_background,
                            "visual_music": step.visual_music,
                            "character_count": step.character_count,
                        })
                    })
                    .collect::<Vec<_>>(),
            })
        });

        let payload = json!({
            "schema": "vneditor.diagnostic_report.v1",
            "generated_unix_ms": now_unix_ms(),
            "language": language_code(self.diagnostic_language),
            "player_locale": self.player_locale,
            "localization": {
                "default_locale": self.localization_catalog.default_locale,
                "locales": self.localization_catalog.locale_codes(),
            },
            "selected_node": self.selected_node,
            "selected_issue": self.selected_issue,
            "issues": issues,
            "quick_fix_audit": quick_fix_audit,
            "dry_run": dry_run,
        });
        serde_json::to_string_pretty(&payload)
    }

    pub fn export_diagnostic_report(&mut self) {
        let Ok(payload) = self.diagnostic_report_json() else {
            self.toast = Some(ToastState::error("Failed to build diagnostic report"));
            return;
        };

        let default_name = "diagnostic_report.json";
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_file_name(default_name)
            .save_file()
        {
            match std::fs::write(path, payload) {
                Ok(_) => {
                    self.toast = Some(ToastState::success("Diagnostic report exported"));
                }
                Err(err) => {
                    self.toast = Some(ToastState::error(format!(
                        "Failed to export diagnostic report: {err}"
                    )));
                }
            }
        } else {
            self.toast = Some(ToastState::warning("Diagnostic report export cancelled"));
        }
    }

    pub fn import_diagnostic_report(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .pick_file()
        else {
            self.toast = Some(ToastState::warning("Diagnostic report import cancelled"));
            return;
        };

        match std::fs::read_to_string(path) {
            Ok(payload) => match self.apply_diagnostic_report_json(&payload) {
                Ok(()) => {
                    self.toast = Some(ToastState::success("Diagnostic report imported"));
                }
                Err(err) => {
                    self.toast = Some(ToastState::error(format!(
                        "Diagnostic report import failed: {err}"
                    )));
                }
            },
            Err(err) => {
                self.toast = Some(ToastState::error(format!(
                    "Failed to read diagnostic report: {err}"
                )));
            }
        }
    }

    pub fn apply_diagnostic_report_json(&mut self, payload: &str) -> Result<(), String> {
        let parsed: serde_json::Value =
            serde_json::from_str(payload).map_err(|err| format!("invalid JSON: {err}"))?;
        let schema = parsed
            .get("schema")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "missing report schema".to_string())?;
        if schema != "vneditor.diagnostic_report.v1" {
            return Err(format!("unsupported report schema '{schema}'"));
        }

        if let Some(language) = parsed
            .get("language")
            .and_then(serde_json::Value::as_str)
            .and_then(parse_language_code)
        {
            self.diagnostic_language = language;
        }
        if let Some(locale) = parsed
            .get("player_locale")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            self.player_locale = locale.to_string();
        }

        let issues_json = parsed
            .get("issues")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "missing issues array".to_string())?;

        let mut imported = Vec::with_capacity(issues_json.len());
        for issue_json in issues_json {
            let phase = parse_validation_phase(
                issue_json
                    .get("phase")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("GRAPH"),
            )?;
            let code = parse_lint_code(
                issue_json
                    .get("code")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("CMP_SCRIPT_ERROR"),
            )?;
            let severity = parse_severity(
                issue_json
                    .get("severity")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("warning"),
            )?;
            let node_id = as_u32_field(issue_json.get("node_id"));
            let event_ip = as_u32_field(issue_json.get("event_ip"));
            let edge_from = as_u32_field(issue_json.get("edge_from"));
            let edge_to = as_u32_field(issue_json.get("edge_to"));
            let asset_path = issue_json
                .get("asset_path")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let message = localized_issue_message(issue_json, self.diagnostic_language);

            let issue = LintIssue::new(node_id, severity, phase, code, message)
                .with_event_ip(event_ip)
                .with_edge(edge_from, edge_to)
                .with_asset_path(asset_path);
            imported.push(issue);
        }

        self.validation_issues = imported;
        self.selected_issue = as_usize_field(parsed.get("selected_issue"));
        self.selected_node = as_u32_field(parsed.get("selected_node"));
        if self.selected_node.is_none() {
            if let Some(issue_index) = self.selected_issue {
                if let Some(issue) = self.validation_issues.get(issue_index) {
                    self.selected_node = issue
                        .node_id
                        .or(issue.edge_from)
                        .or_else(|| {
                            issue
                                .event_ip
                                .and_then(|event_ip| self.node_graph.node_for_event_ip(event_ip))
                        })
                        .or_else(|| {
                            issue.asset_path.as_ref().and_then(|asset| {
                                self.node_graph.first_node_referencing_asset(asset)
                            })
                        });
                }
            }
        }
        self.show_validation = !self.validation_issues.is_empty();
        Ok(())
    }
}

fn language_code(language: DiagnosticLanguage) -> &'static str {
    match language {
        DiagnosticLanguage::Es => "es",
        DiagnosticLanguage::En => "en",
    }
}

fn parse_language_code(value: &str) -> Option<DiagnosticLanguage> {
    match value.trim().to_ascii_lowercase().as_str() {
        "es" => Some(DiagnosticLanguage::Es),
        "en" => Some(DiagnosticLanguage::En),
        _ => None,
    }
}

fn parse_validation_phase(value: &str) -> Result<ValidationPhase, String> {
    ValidationPhase::from_label(value)
        .ok_or_else(|| format!("unknown validation phase '{}'", value.trim()))
}

fn parse_lint_code(value: &str) -> Result<LintCode, String> {
    LintCode::from_label(value).ok_or_else(|| format!("unknown lint code '{}'", value.trim()))
}

fn parse_severity(value: &str) -> Result<LintSeverity, String> {
    LintSeverity::from_label(value)
        .ok_or_else(|| format!("unknown lint severity '{}'", value.trim()))
}

fn as_u32_field(value: Option<&serde_json::Value>) -> Option<u32> {
    value
        .and_then(serde_json::Value::as_u64)
        .and_then(|raw| u32::try_from(raw).ok())
}

fn as_usize_field(value: Option<&serde_json::Value>) -> Option<usize> {
    value
        .and_then(serde_json::Value::as_u64)
        .and_then(|raw| usize::try_from(raw).ok())
}

fn localized_issue_message(issue_json: &serde_json::Value, language: DiagnosticLanguage) -> String {
    let key = match language {
        DiagnosticLanguage::Es => "message_es",
        DiagnosticLanguage::En => "message_en",
    };
    issue_json
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            issue_json
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Imported diagnostic".to_string())
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
