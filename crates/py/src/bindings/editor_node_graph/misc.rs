use pyo3::exceptions::PyValueError;
use std::collections::BTreeMap;
use visual_novel_engine::authoring::quick_fix::{apply_fix, suggest_fixes};
use visual_novel_engine::authoring::{
    load_authoring_document_or_script, validate_authoring_graph, AuthoringDocument, OperationKind,
};

use super::super::diagnostics::PyQuickFixCandidate;
use super::super::support::{apply_autofix_pass, select_fix_candidate};
use super::{PyNodeGraph, PythonOperation};

impl PyNodeGraph {
    pub(super) fn py_fix_candidates(
        &self,
        issue_index: usize,
    ) -> pyo3::PyResult<Vec<PyQuickFixCandidate>> {
        let issues = validate_authoring_graph(&self.inner);
        let issue = issues
            .get(issue_index)
            .ok_or_else(|| PyValueError::new_err(format!("invalid issue index {issue_index}")))?;
        Ok(suggest_fixes(issue, &self.inner)
            .into_iter()
            .map(PyQuickFixCandidate::from)
            .collect())
    }

    pub(super) fn py_autofix_issue(
        &mut self,
        issue_index: usize,
        include_review: bool,
    ) -> pyo3::PyResult<Option<String>> {
        let issues = validate_authoring_graph(&self.inner);
        let issue = issues
            .get(issue_index)
            .ok_or_else(|| PyValueError::new_err(format!("invalid issue index {issue_index}")))?;
        let candidate = select_fix_candidate(issue, &self.inner, include_review)
            .ok_or_else(|| PyValueError::new_err("no fix candidate available for issue"))?;
        let before = self.trace_before_mutation();
        let changed =
            apply_fix(&mut self.inner, issue, candidate.fix_id).map_err(PyValueError::new_err)?;
        if changed {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::QuickFixApplied,
                    format!(
                        "Applied quick-fix {} for diagnostic {} from Python",
                        candidate.fix_id,
                        issue.diagnostic_id()
                    ),
                )
                .with_diagnostic(issue),
                before,
            );
            Ok(Some(candidate.fix_id.to_string()))
        } else {
            Ok(None)
        }
    }

    pub(super) fn py_autofix_safe(&mut self) -> pyo3::PyResult<usize> {
        let before = self.trace_before_mutation();
        let applied = apply_autofix_pass(&mut self.inner, false).map_err(PyValueError::new_err)?;
        if applied > 0 {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::QuickFixApplied,
                    format!("Applied {applied} safe quick-fix(es) from Python"),
                ),
                before,
            );
        }
        Ok(applied)
    }

    pub(super) fn py_autofix_full(&mut self) -> pyo3::PyResult<usize> {
        let before = self.trace_before_mutation();
        let applied = apply_autofix_pass(&mut self.inner, true).map_err(PyValueError::new_err)?;
        if applied > 0 {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::QuickFixApplied,
                    format!("Applied {applied} full quick-fix(es) from Python"),
                ),
                before,
            );
        }
        Ok(applied)
    }

    pub(super) fn py_set_bookmark(&mut self, name: String, node_id: u32) -> bool {
        self.inner.set_bookmark(name, node_id)
    }

    pub(super) fn py_remove_bookmark(&mut self, name: &str) -> bool {
        self.inner.remove_bookmark(name)
    }

    pub(super) fn py_bookmark_target(&self, name: &str) -> Option<u32> {
        self.inner.bookmarked_node(name)
    }

    pub(super) fn py_list_bookmarks(&self) -> Vec<(String, u32)> {
        self.inner
            .bookmarks()
            .map(|(name, node_id)| (name.clone(), *node_id))
            .collect()
    }

    pub(super) fn py_save(&self, path: &str) -> pyo3::PyResult<()> {
        let json = self
            .to_authoring_document()
            .to_json()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        std::fs::write(path, json).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    pub(super) fn py_load(path: &str) -> pyo3::PyResult<Self> {
        let source =
            std::fs::read_to_string(path).map_err(|e| PyValueError::new_err(e.to_string()))?;
        if let Ok(document) = AuthoringDocument::from_json(&source) {
            return Ok(Self {
                inner: document.graph,
                layer_overrides: document.composer_layer_overrides,
                operation_log: document.operation_log,
                verification_runs: document.verification_runs,
            });
        }
        let inner = load_authoring_document_or_script(path)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(Self {
            inner,
            layer_overrides: BTreeMap::new(),
            operation_log: Vec::new(),
            verification_runs: Vec::new(),
        })
    }

    pub(super) fn py_repr(&self) -> String {
        format!(
            "NodeGraph(nodes={}, connections={})",
            self.inner.len(),
            self.inner.connection_count()
        )
    }
}
