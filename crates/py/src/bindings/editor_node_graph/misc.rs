use pyo3::exceptions::PyValueError;
use visual_novel_engine::authoring::quick_fix::suggest_fixes;
use visual_novel_engine::authoring::{
    load_authoring_entry, save_authoring_document, validate_authoring_graph, AuthoringCommand,
};

use super::super::diagnostics::PyQuickFixCandidate;
use super::super::support::select_fix_candidate;
use super::{py_command_error, PyNodeGraph};

impl PyNodeGraph {
    pub(super) fn py_fix_candidates(
        &self,
        issue_index: usize,
    ) -> pyo3::PyResult<Vec<PyQuickFixCandidate>> {
        let issues = validate_authoring_graph(self.inner());
        let issue = issues
            .get(issue_index)
            .ok_or_else(|| PyValueError::new_err(format!("invalid issue index {issue_index}")))?;
        Ok(suggest_fixes(issue, self.inner())
            .into_iter()
            .map(PyQuickFixCandidate::from)
            .collect())
    }

    pub(super) fn py_autofix_issue(
        &mut self,
        issue_index: usize,
        include_review: bool,
    ) -> pyo3::PyResult<Option<String>> {
        let issues = validate_authoring_graph(self.inner());
        let issue = issues
            .get(issue_index)
            .ok_or_else(|| PyValueError::new_err(format!("invalid issue index {issue_index}")))?;
        let candidate = select_fix_candidate(issue, self.inner(), include_review)
            .ok_or_else(|| PyValueError::new_err("no fix candidate available for issue"))?;
        let fix_id = candidate.fix_id.to_string();
        self.apply_authoring_command(AuthoringCommand::ApplyQuickFix {
            issue: Box::new(issue.clone()),
            fix_id: fix_id.clone(),
        })
        .map_err(|err| py_command_error("autofix_issue failed", err))?;
        Ok(Some(fix_id))
    }

    pub(super) fn py_autofix_safe(&mut self) -> pyo3::PyResult<usize> {
        self.apply_autofix_pass_with_command_bus(false)
    }

    pub(super) fn py_autofix_full(&mut self) -> pyo3::PyResult<usize> {
        self.apply_autofix_pass_with_command_bus(true)
    }

    pub(super) fn py_set_bookmark(&mut self, name: String, node_id: u32) -> bool {
        self.session
            .edit_graph(
                visual_novel_engine::authoring::OperationKind::FieldEdited,
                "Set bookmark",
                Some("graph.bookmarks".into()),
                |graph| graph.set_bookmark(name, node_id),
            )
            .is_ok_and(|(result, _)| result)
    }

    pub(super) fn py_remove_bookmark(&mut self, name: &str) -> bool {
        self.session
            .edit_graph(
                visual_novel_engine::authoring::OperationKind::FieldEdited,
                "Remove bookmark",
                Some("graph.bookmarks".into()),
                |graph| graph.remove_bookmark(name),
            )
            .is_ok_and(|(result, _)| result)
    }

    pub(super) fn py_bookmark_target(&self, name: &str) -> Option<u32> {
        self.inner().bookmarked_node(name)
    }

    pub(super) fn py_list_bookmarks(&self) -> Vec<(String, u32)> {
        self.inner()
            .bookmarks()
            .map(|(name, node_id)| (name.clone(), *node_id))
            .collect()
    }

    pub(super) fn py_save(&self, path: &str) -> pyo3::PyResult<()> {
        save_authoring_document(path, self.doc())
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    pub(super) fn py_load(path: &str) -> pyo3::PyResult<Self> {
        load_authoring_entry(path)
            .map(Self::from_authoring_document)
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    pub(super) fn py_repr(&self) -> String {
        format!(
            "NodeGraph(nodes={}, connections={})",
            self.inner().len(),
            self.inner().connection_count()
        )
    }

    fn apply_autofix_pass_with_command_bus(
        &mut self,
        include_review: bool,
    ) -> pyo3::PyResult<usize> {
        let mut applied = 0usize;
        let mut guard = 0usize;
        while guard < 128 {
            guard += 1;
            let issues = validate_authoring_graph(self.inner());
            let mut applied_this_round = false;
            for issue in issues {
                let Some(candidate) = select_fix_candidate(&issue, self.inner(), include_review)
                else {
                    continue;
                };
                self.apply_authoring_command(AuthoringCommand::ApplyQuickFix {
                    issue: Box::new(issue),
                    fix_id: candidate.fix_id.to_string(),
                })
                .map_err(|err| py_command_error("autofix pass failed", err))?;
                applied += 1;
                applied_this_round = true;
                break;
            }
            if !applied_this_round {
                break;
            }
        }
        Ok(applied)
    }
}
