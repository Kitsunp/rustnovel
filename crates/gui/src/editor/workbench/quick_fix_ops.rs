use super::*;
use crate::editor::quick_fix::{apply_fix, suggest_fixes, QuickFixCandidate, QuickFixRisk};
use std::hash::{Hash, Hasher};
use visual_novel_engine::ScriptRaw;

impl EditorWorkbench {
    pub fn apply_issue_fix(&mut self, issue_index: usize, fix_id: &str) -> Result<(), String> {
        let issue = self
            .validation_issues
            .get(issue_index)
            .cloned()
            .ok_or_else(|| format!("invalid issue index {issue_index}"))?;
        self.apply_issue_fix_for_issue(&issue, fix_id)
    }

    pub fn apply_all_safe_fixes(&mut self) -> usize {
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
        let issue = self
            .validation_issues
            .get(issue_index)
            .cloned()
            .ok_or_else(|| format!("invalid issue index {issue_index}"))?;
        let candidate = select_candidate_for_issue(&issue, &self.node_graph, include_review)
            .ok_or_else(|| "no candidate fix available for selected issue".to_string())?;

        if candidate.structural {
            self.prepare_structural_fix_confirmation(issue_index, candidate.fix_id)?;
            return Ok(format!(
                "Structural fix '{}' prepared. Review diff and confirm.",
                candidate.fix_id
            ));
        }

        self.apply_issue_fix(issue_index, candidate.fix_id)?;
        Ok(format!("Applied fix '{}'", candidate.fix_id))
    }

    pub fn revert_last_fix(&mut self) -> bool {
        let Some(previous_graph) = self.last_fix_snapshot.take() else {
            return false;
        };
        self.node_graph = previous_graph;
        self.node_graph.mark_modified();
        let _ = self.sync_graph_to_script();
        true
    }

    pub fn prepare_structural_fix_confirmation(
        &mut self,
        issue_index: usize,
        fix_id: &str,
    ) -> Result<(), String> {
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
        let Some(pending) = self.pending_structural_fix.clone() else {
            return Err("no pending structural fix".to_string());
        };
        self.apply_issue_fix(pending.issue_index, &pending.fix_id)?;
        self.pending_structural_fix = None;
        Ok(pending.fix_id)
    }

    fn apply_issue_fix_for_issue(&mut self, issue: &LintIssue, fix_id: &str) -> Result<(), String> {
        let before_graph = self.node_graph.clone();
        let before_crc32 = crc32_graph(&before_graph);

        let changed = apply_fix(&mut self.node_graph, issue, fix_id)?;
        if !changed {
            return Err(format!("fix '{fix_id}' made no changes"));
        }

        let after_crc32 = crc32_graph(&self.node_graph);
        self.last_fix_snapshot = Some(before_graph);
        self.quick_fix_audit.push(QuickFixAuditEntry {
            diagnostic_id: issue.diagnostic_id(),
            fix_id: fix_id.to_string(),
            node_id: issue.node_id,
            event_ip: issue.event_ip,
            before_crc32,
            after_crc32,
        });

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
}

fn crc32_graph(graph: &NodeGraph) -> u32 {
    let script = graph.to_script();
    let payload = script
        .to_json()
        .unwrap_or_else(|_| format!("fallback_nodes_{}", graph.len()));
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    payload.hash(&mut hasher);
    (hasher.finish() & u32::MAX as u64) as u32
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
