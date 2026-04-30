use super::*;

impl EditorWorkbench {
    pub fn build_repro_case_from_current_graph(
        &mut self,
    ) -> Option<visual_novel_engine::ReproCase> {
        let result = self.compile_current_graph();
        let repro_script = result
            .minimal_repro_script()
            .or_else(|| Some(result.script.clone()))?;

        let mut case = visual_novel_engine::ReproCase::new("dry_run_repro_case", repro_script);
        let fingerprints = visual_novel_engine::authoring::build_authoring_report_fingerprint(
            self.node_graph.authoring_graph(),
            &result.script,
        );
        case.semantic_fingerprint_sha256 = Some(fingerprints.semantic_sha256);
        case.operation_id = Some("gui.build_repro_case_from_current_graph".to_string());
        case.validation_profile = Some("gui.dry_run".to_string());
        case.diagnostic_id = self
            .selected_issue
            .and_then(|idx| self.validation_issues.get(idx))
            .or_else(|| {
                result
                    .issues
                    .iter()
                    .find(|issue| issue.severity == LintSeverity::Error)
            })
            .map(LintIssue::diagnostic_id);
        if let Some(report) = result.dry_run_report.as_ref() {
            case.max_steps = report.max_steps;
            case.oracle.expected_stop_reason = Some(map_dry_run_stop_reason(report.stop_reason));
            case.oracle.expected_event_ip = report
                .failing_event_ip
                .or_else(|| report.steps.last().map(|step| step.event_ip));
            if let Some(event_ip) = case.oracle.expected_event_ip {
                if let Some(step) = report.steps.iter().find(|step| step.event_ip == event_ip) {
                    case.oracle.expected_event_kind = Some(step.event_kind.clone());
                    case.oracle.monitors.push(
                        visual_novel_engine::ReproMonitor::EventSignatureContains {
                            monitor_id: "monitor_event_signature".to_string(),
                            step: step.step,
                            needle: step.event_signature.clone(),
                        },
                    );
                }
            }
            if report.stop_reason == crate::editor::compiler::DryRunStopReason::RuntimeError
                && !report.stop_message.trim().is_empty()
            {
                case.oracle
                    .monitors
                    .push(visual_novel_engine::ReproMonitor::StopMessageContains {
                        monitor_id: "monitor_stop_message".to_string(),
                        needle: report.stop_message.clone(),
                    });
            }
        }

        self.loaded_repro_case = Some(case.clone());
        self.last_repro_report = None;
        Some(case)
    }

    pub fn export_repro_case(&mut self) {
        let Some(case) = self.build_repro_case_from_current_graph() else {
            self.toast = Some(ToastState::warning(
                "No se pudo construir el repro case desde el grafo actual",
            ));
            return;
        };

        let Ok(payload) = case.to_json() else {
            self.toast = Some(ToastState::error("Failed to serialize repro case"));
            return;
        };

        let path = rfd::FileDialog::new()
            .add_filter("Repro Case JSON", &["json"])
            .set_file_name("dry_run_repro_case.json")
            .save_file();

        if let Some(path) = path {
            match std::fs::write(&path, payload) {
                Ok(_) => {
                    self.toast = Some(ToastState::success("Repro case exported"));
                }
                Err(err) => {
                    self.toast = Some(ToastState::error(format!(
                        "Repro case export failed: {err}"
                    )));
                }
            }
        } else {
            self.toast = Some(ToastState::warning("Repro case export cancelled"));
        }
    }

    pub fn import_repro_case(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Repro Case JSON", &["json"])
            .pick_file()
        else {
            self.toast = Some(ToastState::warning("Repro case import cancelled"));
            return;
        };

        match std::fs::read_to_string(&path) {
            Ok(payload) => match self.apply_repro_case_json(&payload) {
                Ok(()) => {
                    self.toast = Some(ToastState::success("Repro case imported"));
                }
                Err(err) => {
                    self.toast = Some(ToastState::error(format!(
                        "Repro case import failed: {err}"
                    )));
                }
            },
            Err(err) => {
                self.toast = Some(ToastState::error(format!(
                    "Failed to read repro case: {err}"
                )));
            }
        }
    }

    pub fn apply_repro_case_json(&mut self, payload: &str) -> Result<(), String> {
        let case = visual_novel_engine::ReproCase::from_json(payload)
            .map_err(|err| format!("invalid repro case: {err}"))?;
        self.apply_repro_case(case)
    }

    pub fn run_loaded_repro_case(&mut self) {
        let Some(case) = self.loaded_repro_case.clone() else {
            self.toast = Some(ToastState::warning("No loaded repro case"));
            return;
        };

        let report = visual_novel_engine::run_repro_case(&case);
        self.last_repro_report = Some(report.clone());
        self.append_repro_report_issues(&case, &report);

        if report.oracle_triggered {
            self.toast = Some(ToastState::warning(format!(
                "Repro triggered (reason={}, monitors={})",
                report.stop_reason.label(),
                report.matched_monitors.join(",")
            )));
        } else {
            self.toast = Some(ToastState::success(format!(
                "Repro executed without oracle trigger (reason={})",
                report.stop_reason.label()
            )));
        }
    }

    fn apply_repro_case(&mut self, case: visual_novel_engine::ReproCase) -> Result<(), String> {
        let graph = crate::editor::script_sync::from_script(&case.script);
        self.node_graph = graph;
        let mut stack = UndoStack::new();
        stack.push(self.node_graph.clone());
        self.undo_stack = stack;
        self.pending_save_path = None;
        self.current_script = Some(case.script.clone());
        self.saved_script_snapshot = Some(case.script.clone());
        self.loaded_repro_case = Some(case);
        self.last_repro_report = None;
        self.selected_node = None;
        self.selected_issue = None;
        self.sync_graph_to_script()?;
        Ok(())
    }

    fn append_repro_report_issues(
        &mut self,
        case: &visual_novel_engine::ReproCase,
        report: &visual_novel_engine::ReproRunReport,
    ) {
        self.validation_issues.push(
            LintIssue::info(
                None,
                ValidationPhase::DryRun,
                LintCode::DryRunFinished,
                format!(
                    "Repro '{}' executed: reason={} steps={}",
                    case.title,
                    report.stop_reason.label(),
                    report.executed_steps
                ),
            )
            .with_event_ip(report.failing_event_ip),
        );

        if report.signature_match {
            self.validation_issues.push(
                LintIssue::warning(
                    None,
                    ValidationPhase::DryRun,
                    LintCode::DryRunParityMismatch,
                    "Repro oracle signature matched",
                )
                .with_event_ip(report.failing_event_ip),
            );
        }

        for monitor_id in &report.matched_monitors {
            self.validation_issues.push(
                LintIssue::warning(
                    None,
                    ValidationPhase::DryRun,
                    LintCode::DryRunParityMismatch,
                    format!("Repro monitor matched: {monitor_id}"),
                )
                .with_event_ip(report.failing_event_ip),
            );
        }
        self.show_validation = !self.validation_issues.is_empty();
    }
}

fn map_dry_run_stop_reason(
    stop_reason: crate::editor::compiler::DryRunStopReason,
) -> visual_novel_engine::ReproStopReason {
    match stop_reason {
        crate::editor::compiler::DryRunStopReason::Finished => {
            visual_novel_engine::ReproStopReason::Finished
        }
        crate::editor::compiler::DryRunStopReason::StepLimit => {
            visual_novel_engine::ReproStopReason::StepLimit
        }
        crate::editor::compiler::DryRunStopReason::RuntimeError => {
            visual_novel_engine::ReproStopReason::RuntimeError
        }
    }
}
