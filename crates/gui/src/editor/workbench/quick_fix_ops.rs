use super::*;
use crate::editor::quick_fix::{apply_fix, suggest_fixes, QuickFixCandidate, QuickFixRisk};
use visual_novel_engine::ScriptRaw;

impl EditorWorkbench {
    pub fn apply_issue_fix(&mut self, issue_index: usize, fix_id: &str) -> Result<(), String> {
        self.ensure_report_allows_fix(false)?;
        let issue = self
            .validation_issues
            .get(issue_index)
            .cloned()
            .ok_or_else(|| format!("invalid issue index {issue_index}"))?;
        self.apply_issue_fix_for_issue(&issue, fix_id)
    }

    pub fn apply_all_safe_fixes(&mut self) -> usize {
        if let Err(reason) = self.ensure_report_allows_fix(false) {
            self.toast = Some(ToastState::warning(reason));
            return 0;
        }
        let mut applied = 0usize;
        let mut guard = 0usize;

        while guard < 128 {
            guard += 1;
            let issues_snapshot = self.validation_issues.clone();
            let mut applied_this_round = false;

            for issue in issues_snapshot {
                let candidates = suggest_fixes(&issue, &self.node_graph);
                let Some(candidate) = candidates
                    .iter()
                    .find(|candidate| candidate.risk == QuickFixRisk::Safe)
                else {
                    continue;
                };

                if self
                    .apply_issue_fix_for_issue(&issue, candidate.fix_id)
                    .is_ok()
                {
                    applied += 1;
                    applied_this_round = true;
                    break;
                }
            }

            if !applied_this_round {
                break;
            }
        }

        applied
    }

    pub fn prepare_autofix_batch_confirmation(
        &mut self,
        include_review: bool,
    ) -> Result<usize, String> {
        self.ensure_report_allows_fix(include_review)?;
        let plan = self.build_autofix_plan(include_review);
        if plan.is_empty() {
            return Err("no auto-fix candidates available".to_string());
        }

        let before_script = self.node_graph.to_script();
        let mut preview_graph = self.node_graph.clone();
        let mut operations = Vec::new();

        for op in plan {
            let changed = apply_fix(&mut preview_graph, &op.issue, &op.fix_id)?;
            if !changed {
                continue;
            }
            operations.push(op);
        }

        if operations.is_empty() {
            return Err("planned fixes produced no effective changes".to_string());
        }

        let after_script = preview_graph.to_script();
        self.pending_structural_fix = None;
        self.pending_auto_fix_batch = Some(PendingAutoFixBatch {
            include_review,
            operations: operations.clone(),
        });
        self.fix_diff_dialog = Some(DiffDialog::new_autofix_batch(
            Some(&before_script),
            &after_script,
            operations.len(),
            include_review,
        ));
        self.show_fix_confirm = true;
        Ok(operations.len())
    }

    pub fn apply_pending_autofix_batch(&mut self) -> Result<AutoFixBatchResult, String> {
        self.ensure_report_allows_fix(false)?;
        let Some(pending) = self.pending_auto_fix_batch.take() else {
            return Err("no pending auto-fix batch".to_string());
        };

        let mut result = AutoFixBatchResult::default();
        for op in pending.operations {
            match self.apply_issue_fix_for_issue(&op.issue, &op.fix_id) {
                Ok(()) => result.applied += 1,
                Err(_) => result.skipped += 1,
            }
        }

        Ok(result)
    }

    pub fn apply_best_fix_for_issue(
        &mut self,
        issue_index: usize,
        include_review: bool,
    ) -> Result<String, String> {
        self.ensure_report_allows_fix(include_review)?;
        let issue = self
            .validation_issues
            .get(issue_index)
            .cloned()
            .ok_or_else(|| format!("invalid issue index {issue_index}"))?;
        let candidate = select_candidate_for_issue(&issue, &self.node_graph, include_review)
            .ok_or_else(|| "no candidate fix available for selected issue".to_string())?;

        if candidate.structural {
            self.ensure_report_allows_fix(true)?;
            self.prepare_structural_fix_confirmation(issue_index, candidate.fix_id)?;
            return Ok(format!(
                "Structural fix '{}' prepared. Review diff and confirm.",
                candidate.fix_id
            ));
        }

        self.ensure_report_allows_fix(false)?;
        self.apply_issue_fix(issue_index, candidate.fix_id)?;
        Ok(format!("Applied fix '{}'", candidate.fix_id))
    }

    pub fn revert_last_fix(&mut self) -> bool {
        let Some(previous_graph) = self.last_fix_snapshot.take() else {
            return false;
        };
        let before = self.current_authoring_fingerprint();
        self.node_graph = previous_graph;
        self.node_graph.mark_modified();
        let _ = self.sync_graph_to_script();
        if let Some(after) = self.current_authoring_fingerprint() {
            let mut entry = visual_novel_engine::authoring::OperationLogEntry::new(
                format!("editor:revert_last_fix:{}", self.operation_log.len() + 1),
                "revert_last_fix",
                "applied",
                "Reverted last quick-fix snapshot",
            );
            if let Some(before) = before.as_ref() {
                entry = entry.with_before_after_fingerprints(before, &after);
            } else {
                entry = entry.with_fingerprint(&after);
            }
            self.last_operation_fingerprint = Some(after);
            self.operation_log.push(entry);
        }
        true
    }

    pub fn prepare_structural_fix_confirmation(
        &mut self,
        issue_index: usize,
        fix_id: &str,
    ) -> Result<(), String> {
        self.ensure_report_allows_fix(true)?;
        let (before_script, after_script) = self.preview_issue_fix_scripts(issue_index, fix_id)?;
        self.pending_structural_fix = Some(PendingStructuralFix {
            issue_index,
            fix_id: fix_id.to_string(),
        });
        self.fix_diff_dialog = Some(DiffDialog::new_quick_fix(
            Some(&before_script),
            &after_script,
            fix_id,
        ));
        self.show_fix_confirm = true;
        Ok(())
    }

    pub fn apply_pending_structural_fix(&mut self) -> Result<String, String> {
        self.ensure_report_allows_fix(true)?;
        let Some(pending) = self.pending_structural_fix.clone() else {
            return Err("no pending structural fix".to_string());
        };
        let issue = self
            .validation_issues
            .get(pending.issue_index)
            .cloned()
            .ok_or_else(|| format!("invalid issue index {}", pending.issue_index))?;
        self.apply_issue_fix_for_issue(&issue, &pending.fix_id)?;
        self.pending_structural_fix = None;
        Ok(pending.fix_id)
    }

    fn apply_issue_fix_for_issue(&mut self, issue: &LintIssue, fix_id: &str) -> Result<(), String> {
        let before_graph = self.node_graph.clone();
        let before_sha256 =
            visual_novel_engine::authoring::authoring_graph_sha256(before_graph.authoring_graph());
        let before_script = before_graph.to_script();
        let before_fingerprint = visual_novel_engine::authoring::build_authoring_report_fingerprint(
            before_graph.authoring_graph(),
            &before_script,
        );

        let changed = apply_fix(&mut self.node_graph, issue, fix_id)?;
        if !changed {
            return Err(format!("fix '{fix_id}' made no changes"));
        }

        let after_sha256 = visual_novel_engine::authoring::authoring_graph_sha256(
            self.node_graph.authoring_graph(),
        );
        let operation_id = format!("quickfix:{}:{}", fix_id, self.quick_fix_audit.len() + 1);
        let after_script = self.node_graph.to_script();
        let after_fingerprint = visual_novel_engine::authoring::build_authoring_report_fingerprint(
            self.node_graph.authoring_graph(),
            &after_script,
        );
        self.last_fix_snapshot = Some(before_graph);
        self.quick_fix_audit.push(QuickFixAuditEntry {
            operation_id: operation_id.clone(),
            diagnostic_id: issue.diagnostic_id(),
            fix_id: fix_id.to_string(),
            node_id: issue.node_id,
            event_ip: issue.event_ip,
            before_sha256,
            after_sha256,
        });
        self.operation_log.push(
            visual_novel_engine::authoring::OperationLogEntry::new(
                operation_id,
                "quick_fix",
                "applied",
                format!("Applied quick-fix '{fix_id}'"),
            )
            .with_diagnostic(issue)
            .with_before_after_fingerprints(&before_fingerprint, &after_fingerprint),
        );
        self.last_operation_fingerprint = Some(after_fingerprint);

        let previous_diag_id = issue.diagnostic_id();
        let _ = self.sync_graph_to_script();
        self.selected_issue = self
            .validation_issues
            .iter()
            .position(|current| current.diagnostic_id() == previous_diag_id);

        Ok(())
    }

    fn preview_issue_fix_scripts(
        &self,
        issue_index: usize,
        fix_id: &str,
    ) -> Result<(ScriptRaw, ScriptRaw), String> {
        let issue = self
            .validation_issues
            .get(issue_index)
            .ok_or_else(|| format!("invalid issue index {issue_index}"))?;
        let before_script = self.node_graph.to_script();

        let mut preview_graph = self.node_graph.clone();
        let changed = apply_fix(&mut preview_graph, issue, fix_id)?;
        if !changed {
            return Err(format!("fix '{fix_id}' made no preview changes"));
        }
        let after_script = preview_graph.to_script();
        Ok((before_script, after_script))
    }

    fn build_autofix_plan(&self, include_review: bool) -> Vec<PendingAutoFixOperation> {
        let mut plan = Vec::new();
        for issue in &self.validation_issues {
            let Some(candidate) =
                select_candidate_for_issue(issue, &self.node_graph, include_review)
            else {
                continue;
            };
            plan.push(PendingAutoFixOperation {
                issue: issue.clone(),
                fix_id: candidate.fix_id.to_string(),
            });
        }
        plan
    }

    fn ensure_report_allows_fix(&self, allow_manual_review: bool) -> Result<(), String> {
        if self.imported_report_untrusted {
            return Err(
                "imported report has no trusted fingerprint; fixes are blocked".to_string(),
            );
        }
        if self.imported_report_stale && !allow_manual_review {
            return Err("imported report is stale; automatic fixes are blocked".to_string());
        }
        Ok(())
    }
}

fn select_candidate_for_issue<'a>(
    issue: &'a LintIssue,
    graph: &'a NodeGraph,
    include_review: bool,
) -> Option<QuickFixCandidate> {
    let candidates = suggest_fixes(issue, graph);
    if include_review {
        candidates
            .iter()
            .find(|candidate| candidate.risk == QuickFixRisk::Safe)
            .cloned()
            .or_else(|| candidates.into_iter().next())
    } else {
        candidates
            .into_iter()
            .find(|candidate| candidate.risk == QuickFixRisk::Safe)
    }
}
