mod bindings;

use pyo3::prelude::*;
use visual_novel_gui::{run_app as run_gui, GuiError};

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
fn run_visual_novel(script_json: String, config: Option<PyVnConfig>) -> PyResult<()> {
    let gui_config = config.map(Into::into);
    run_gui(script_json, gui_config).map_err(|err| match err {
        GuiError::Script(script) => pyo3::exceptions::PyValueError::new_err(script.to_string()),
        _ => pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to run GUI: {err}")),
    })
}
