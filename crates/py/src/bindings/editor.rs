//! Python bindings for the visual editor components.
//!
//! Exposes NodeGraph, StoryNode, and validation to Python for scripting.

use pyo3::prelude::*;

use visual_novel_gui::editor::validate_graph;

#[path = "editor_diagnostics.rs"]
mod diagnostics;
#[path = "editor_node_graph.rs"]
mod node_graph;
#[path = "editor_story_node.rs"]
mod story_node;
#[path = "editor_support.rs"]
mod support;

pub use diagnostics::{PyLintIssue, PyLintSeverity, PyQuickFixCandidate};
pub use node_graph::PyNodeGraph;
pub use story_node::PyStoryNode;

#[cfg(test)]
use support::{apply_autofix_pass, select_fix_candidate};
#[cfg(test)]
use visual_novel_gui::editor::{LintIssue, NodeGraph, StoryNode};

#[pyfunction]
pub fn py_validate_graph(graph: &PyNodeGraph) -> Vec<PyLintIssue> {
    validate_graph(graph.inner())
        .into_iter()
        .map(PyLintIssue::from)
        .collect()
}

/// Registers editor classes with the Python module.
pub fn register_editor_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStoryNode>()?;
    m.add_class::<PyNodeGraph>()?;
    m.add_class::<PyQuickFixCandidate>()?;
    m.add_class::<PyLintSeverity>()?;
    m.add_class::<PyLintIssue>()?;
    m.add_function(wrap_pyfunction!(py_validate_graph, m)?)?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/editor_tests.rs"]
mod tests;
