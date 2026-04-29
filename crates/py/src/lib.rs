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
    m.add_function(wrap_pyfunction!(export_bundle, m)?)?;
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

#[pyfunction]
#[pyo3(signature = (project_root, output_root, entry_script=None, target="windows"))]
fn export_bundle(
    project_root: String,
    output_root: String,
    entry_script: Option<String>,
    target: &str,
) -> PyResult<String> {
    let target_platform = match target.trim().to_ascii_lowercase().as_str() {
        "windows" | "win" => ::visual_novel_engine::ExportTargetPlatform::Windows,
        "linux" => ::visual_novel_engine::ExportTargetPlatform::Linux,
        "macos" | "darwin" | "osx" => ::visual_novel_engine::ExportTargetPlatform::Macos,
        other => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "unknown bundle target '{other}'"
            )));
        }
    };
    let report = ::visual_novel_engine::export_bundle(::visual_novel_engine::ExportBundleSpec {
        project_root: project_root.into(),
        output_root: output_root.into(),
        target_platform,
        entry_script: entry_script.map(Into::into),
        runtime_artifact: None,
        integrity: ::visual_novel_engine::BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
    serde_json::to_string_pretty(&report)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))
}
