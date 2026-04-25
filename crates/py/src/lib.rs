mod bindings;

use pyo3::prelude::*;

pub use bindings::{
    register_editor_classes, vn_error_to_py, PyAudio, PyEngine, PyGraphEdge, PyGraphNode,
    PyGraphStats, PyKeyframe, PyLintIssue, PyLintSeverity, PyNodeGraph, PyQuickFixCandidate,
    PyResourceConfig, PyScriptBuilder, PyStoryGraph, PyStoryNode, PyTimeline, PyTrack, PyVnConfig,
    StepResult,
};

#[pymodule]
fn visual_novel_engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEngine>()?;
    m.add_class::<StepResult>()?;
    m.add_class::<PyAudio>()?;
    m.add_class::<PyResourceConfig>()?;
    m.add_class::<PyScriptBuilder>()?;
    m.add_class::<PyVnConfig>()?;
    // Phase 2: Timeline classes
    m.add_class::<PyTimeline>()?;
    m.add_class::<PyTrack>()?;
    m.add_class::<PyKeyframe>()?;
    // Phase 3: Graph classes
    m.add_class::<PyStoryGraph>()?;
    m.add_class::<PyGraphNode>()?;
    m.add_class::<PyGraphEdge>()?;
    m.add_class::<PyGraphStats>()?;
    // Phase 7: Editor classes
    register_editor_classes(m)?;
    m.add_function(wrap_pyfunction!(run_visual_novel, m)?)?;
    m.add("PyEngine", m.getattr("Engine")?)?;
    Ok(())
}

#[pyfunction]
fn run_visual_novel(script_json: String, _config: Option<PyVnConfig>) -> PyResult<()> {
    serde_json::from_str::<::visual_novel_engine::ScriptRaw>(&script_json)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
    Err(pyo3::exceptions::PyRuntimeError::new_err(
        "GUI launch is not available in the headless Python extension; use the Rust GUI binary.",
    ))
}
