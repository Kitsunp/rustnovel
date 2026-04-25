use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use visual_novel_gui::editor::quick_fix::{apply_fix, suggest_fixes};
use visual_novel_gui::editor::{validate_graph, NodeGraph};

use super::diagnostics::{PyLintIssue, PyQuickFixCandidate};
use super::story_node::PyStoryNode;
use super::support::{apply_autofix_pass, select_fix_candidate};

/// A graph of story nodes with connections.
#[pyclass(name = "NodeGraph")]
pub struct PyNodeGraph {
    inner: NodeGraph,
}

impl PyNodeGraph {
    pub(super) fn inner(&self) -> &NodeGraph {
        &self.inner
    }
}

#[pymethods]
impl PyNodeGraph {
    #[new]
    fn new() -> Self {
        Self {
            inner: NodeGraph::new(),
        }
    }

    fn add_node(&mut self, node: PyStoryNode, x: f32, y: f32) -> u32 {
        let pos = eframe::egui::pos2(x, y);
        self.inner.add_node(node.into_inner(), pos)
    }

    fn connect(&mut self, from_id: u32, to_id: u32) {
        self.inner.connect(from_id, to_id);
    }

    fn remove_node(&mut self, node_id: u32) {
        self.inner.remove_node(node_id);
    }

    fn node_count(&self) -> usize {
        self.inner.len()
    }

    fn connection_count(&self) -> usize {
        self.inner.connection_count()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn to_script_json(&self) -> PyResult<String> {
        let script = self.inner.to_script();
        serde_json::to_string_pretty(&script).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn search_nodes(&self, query: &str) -> Vec<u32> {
        self.inner.search_nodes(query)
    }

    fn validate(&self) -> Vec<PyLintIssue> {
        validate_graph(&self.inner)
            .into_iter()
            .map(PyLintIssue::from)
            .collect()
    }

    fn fix_candidates(&self, issue_index: usize) -> PyResult<Vec<PyQuickFixCandidate>> {
        let issues = validate_graph(&self.inner);
        let issue = issues
            .get(issue_index)
            .ok_or_else(|| PyValueError::new_err(format!("invalid issue index {issue_index}")))?;
        Ok(suggest_fixes(issue, &self.inner)
            .into_iter()
            .map(PyQuickFixCandidate::from)
            .collect())
    }

    #[pyo3(signature = (issue_index, include_review=false))]
    fn autofix_issue(
        &mut self,
        issue_index: usize,
        include_review: bool,
    ) -> PyResult<Option<String>> {
        let issues = validate_graph(&self.inner);
        let issue = issues
            .get(issue_index)
            .ok_or_else(|| PyValueError::new_err(format!("invalid issue index {issue_index}")))?;
        let candidate = select_fix_candidate(issue, &self.inner, include_review)
            .ok_or_else(|| PyValueError::new_err("no fix candidate available for issue"))?;
        let changed =
            apply_fix(&mut self.inner, issue, candidate.fix_id).map_err(PyValueError::new_err)?;
        if changed {
            Ok(Some(candidate.fix_id.to_string()))
        } else {
            Ok(None)
        }
    }

    fn autofix_safe(&mut self) -> PyResult<usize> {
        apply_autofix_pass(&mut self.inner, false).map_err(PyValueError::new_err)
    }

    fn autofix_full(&mut self) -> PyResult<usize> {
        apply_autofix_pass(&mut self.inner, true).map_err(PyValueError::new_err)
    }

    fn set_bookmark(&mut self, name: String, node_id: u32) -> bool {
        self.inner.set_bookmark(name, node_id)
    }

    fn remove_bookmark(&mut self, name: &str) -> bool {
        self.inner.remove_bookmark(name)
    }

    fn bookmark_target(&self, name: &str) -> Option<u32> {
        self.inner.bookmarked_node(name)
    }

    fn list_bookmarks(&self) -> Vec<(String, u32)> {
        self.inner
            .bookmarks()
            .map(|(name, node_id)| (name.clone(), *node_id))
            .collect()
    }

    fn save(&self, path: &str) -> PyResult<()> {
        let script = self.inner.to_script();
        let json = serde_json::to_string_pretty(&script)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        std::fs::write(path, json).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[staticmethod]
    fn load(path: &str) -> PyResult<Self> {
        let content =
            std::fs::read_to_string(path).map_err(|e| PyValueError::new_err(e.to_string()))?;
        let script: visual_novel_engine::ScriptRaw =
            serde_json::from_str(&content).map_err(|e| PyValueError::new_err(e.to_string()))?;
        let graph = NodeGraph::from_script(&script);
        Ok(Self { inner: graph })
    }

    fn __repr__(&self) -> String {
        format!(
            "NodeGraph(nodes={}, connections={})",
            self.inner.len(),
            self.inner.connection_count()
        )
    }
}
