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
                let envelope_v2 = issue.envelope_v2();
                let mut payload = serde_json::to_value(&envelope_v2)
                    .unwrap_or_else(|_| json!({ "schema": "vnengine.diagnostic_envelope.v2" }));
                if let Some(object) = payload.as_object_mut() {
                    object.insert("diagnostic_id".to_string(), json!(issue.diagnostic_id()));
                    object.insert("envelope_v2".to_string(), json!(envelope_v2));
                    object.insert("message_key".to_string(), json!(en.message_key));
                    object.insert("phase".to_string(), json!(issue.phase.label()));
                    object.insert("code".to_string(), json!(issue.code.label()));
                    object.insert("severity".to_string(), json!(issue.severity.label()));
                    object.insert("node_id".to_string(), json!(issue.node_id));
                    object.insert("event_ip".to_string(), json!(issue.event_ip));
                    object.insert("edge_from".to_string(), json!(issue.edge_from));
                    object.insert("edge_to".to_string(), json!(issue.edge_to));
                    object.insert("asset_path".to_string(), json!(issue.asset_path));
                    object.insert(
                        "message_es".to_string(),
                        json!(issue.localized_message(DiagnosticLanguage::Es)),
                    );
                    object.insert(
                        "message_en".to_string(),
                        json!(issue.localized_message(DiagnosticLanguage::En)),
                    );
                    object.insert("what_happened_es".to_string(), json!(es.what_happened));
                    object.insert("what_happened_en".to_string(), json!(en.what_happened));
                    object.insert("root_cause_es".to_string(), json!(es.root_cause));
                    object.insert("root_cause_en".to_string(), json!(en.root_cause));
                    object.insert("why_failed_es".to_string(), json!(es.why_failed));
                    object.insert("why_failed_en".to_string(), json!(en.why_failed));
                    object.insert("consequence_es".to_string(), json!(es.consequence));
                    object.insert("consequence_en".to_string(), json!(en.consequence));
                    object.insert("how_to_fix_es".to_string(), json!(es.how_to_fix));
                    object.insert("how_to_fix_en".to_string(), json!(en.how_to_fix));
                    object.insert("action_steps_es".to_string(), json!(es.action_steps));
                    object.insert("action_steps_en".to_string(), json!(en.action_steps));
                    object.insert("expected_es".to_string(), json!(es.expected));
                    object.insert("expected_en".to_string(), json!(en.expected));
                    object.insert("actual".to_string(), json!(issue.message.clone()));
                    object.insert("docs_ref".to_string(), json!(es.docs_ref));
                }
                payload
            })
            .collect::<Vec<_>>();

        let quick_fix_audit = self
            .quick_fix_audit
            .iter()
            .map(|entry| {
                json!({
                    "operation_id": entry.operation_id,
                    "diagnostic_id": entry.diagnostic_id,
                    "fix_id": entry.fix_id,
                    "node_id": entry.node_id,
                    "event_ip": entry.event_ip,
                    "before_sha256": entry.before_sha256,
                    "after_sha256": entry.after_sha256,
                })
            })
            .collect::<Vec<_>>();

        let dry_run = self.last_dry_run_report.as_ref().map(|report| {
            json!({
                "max_steps": report.max_steps,
                "executed_steps": report.executed_steps,
                "routes_discovered": report.routes_discovered,
                "routes_executed": report.routes_executed,
                "route_limit_hit": report.route_limit_hit,
                "depth_limit_hit": report.depth_limit_hit,
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
                            "simulation_note": step.simulation_note,
                            "visual_background": step.visual_background,
                            "visual_music": step.visual_music,
                            "character_count": step.character_count,
                        })
                    })
                    .collect::<Vec<_>>(),
            })
        });
        let report_script = self.node_graph.to_script();
        let fingerprints = visual_novel_engine::authoring::build_authoring_report_fingerprint(
            self.node_graph.authoring_graph(),
            &report_script,
        );
        let verification_run = visual_novel_engine::authoring::VerificationRun::from_diagnostics(
            format!("diagnostic_report:{}", now_unix_ms()),
            "gui.current_report",
            &fingerprints,
            &[],
            &self.validation_issues,
        );

        let payload = json!({
            "schema": "vnengine.authoring_validation_report.v2",
            "legacy_schema": "vneditor.diagnostic_report.v1",
            "generated_unix_ms": now_unix_ms(),
            "fingerprints": fingerprints,
            "verification_run": verification_run,
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
            "operation_log": &self.operation_log,
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
                    self.toast = if self.imported_report_untrusted {
                        Some(ToastState::warning(
                            "Diagnostic report imported without trusted fingerprint; fixes blocked",
                        ))
                    } else if self.imported_report_stale {
                        Some(ToastState::warning(
                            "Diagnostic report imported as stale; automatic fixes blocked",
                        ))
                    } else {
                        Some(ToastState::success("Diagnostic report imported"))
                    };
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
        if schema != "vneditor.diagnostic_report.v1"
            && schema != "vneditor.diagnostic_report.v2"
            && schema != "vnengine.authoring_validation_report.v2"
        {
            return Err(format!("unsupported report schema '{schema}'"));
        }
        let current_fingerprints = current_fingerprints_value(self)?;
        let imported_fingerprints = parsed.get("fingerprints");
        self.imported_report_untrusted = imported_fingerprints.is_none();
        self.imported_report_stale = imported_fingerprints.is_none_or(|fingerprints| {
            !visual_novel_engine::authoring::authoring_fingerprints_semantically_match(
                fingerprints,
                &current_fingerprints,
            )
        });

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
            let envelope = issue_json.get("envelope_v2").unwrap_or(issue_json);
            let phase = parse_validation_phase(
                issue_json
                    .get("phase")
                    .or_else(|| envelope.get("phase"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("GRAPH"),
            )?;
            let code = parse_lint_code(
                issue_json
                    .get("code")
                    .or_else(|| envelope.get("code"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("CMP_SCRIPT_ERROR"),
            )?;
            let severity = parse_severity(
                issue_json
                    .get("severity")
                    .or_else(|| envelope.get("severity"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("warning"),
            )?;
            let location = envelope.get("location").unwrap_or(issue_json);
            let node_id = as_u32_field(
                issue_json
                    .get("node_id")
                    .or_else(|| location.get("node_id")),
            );
            let event_ip = as_u32_field(
                issue_json
                    .get("event_ip")
                    .or_else(|| location.get("event_ip")),
            );
            let edge_from = as_u32_field(
                issue_json
                    .get("edge_from")
                    .or_else(|| location.get("edge_from")),
            );
            let edge_to = as_u32_field(
                issue_json
                    .get("edge_to")
                    .or_else(|| location.get("edge_to")),
            );
            let asset_path = issue_json
                .get("asset_path")
                .or_else(|| location.get("asset_path"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let message = localized_issue_message(issue_json, self.diagnostic_language);

            let mut issue = LintIssue::new(node_id, severity, phase, code, message)
                .with_event_ip(event_ip)
                .with_edge(edge_from, edge_to)
                .with_asset_path(asset_path);
            if let Some(field_path) = envelope
                .get("field_path")
                .or_else(|| location.get("field_path"))
                .and_then(|value| {
                    value
                        .get("value")
                        .and_then(serde_json::Value::as_str)
                        .or_else(|| value.as_str())
                })
            {
                issue = issue.with_field_path(field_path.to_string());
            }
            if let Some(target) = envelope
                .get("target")
                .or_else(|| location.get("target"))
                .and_then(|value| {
                    serde_json::from_value::<visual_novel_engine::authoring::DiagnosticTarget>(
                        value.clone(),
                    )
                    .ok()
                })
            {
                issue = issue.with_target(target);
            }
            if let Some(values) = envelope
                .get("semantic_values")
                .and_then(serde_json::Value::as_array)
            {
                for value in values {
                    if let Ok(semantic) = serde_json::from_value::<
                        visual_novel_engine::authoring::SemanticValue,
                    >(value.clone())
                    {
                        issue = issue.with_semantic_value(semantic);
                    }
                }
            }
            if envelope.get("evidence_trace").is_some() && issue.evidence_trace.is_none() {
                issue = issue.with_evidence_trace();
            }
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

fn current_fingerprints_value(workbench: &EditorWorkbench) -> Result<serde_json::Value, String> {
    let script = workbench.node_graph.to_script();
    let fingerprints = visual_novel_engine::authoring::build_authoring_report_fingerprint(
        workbench.node_graph.authoring_graph(),
        &script,
    );
    serde_json::to_value(fingerprints).map_err(|err| format!("fingerprint serialization: {err}"))
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
        .or_else(|| {
            let text_key = match language {
                DiagnosticLanguage::Es => "text_es",
                DiagnosticLanguage::En => "text_en",
            };
            issue_json
                .get("envelope_v2")
                .and_then(|envelope| envelope.get(text_key))
                .and_then(|text| text.get("actual"))
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
