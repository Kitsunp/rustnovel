mod bindings;

use pyo3::prelude::*;

pub use bindings::{
    register_editor_classes, vn_error_to_py, PyAudio, PyAuthoringValidationReport,
    PyComposerPreviewSession, PyComposerSnapshot, PyDiagnosticTarget, PyEngine, PyEvidenceTrace,
    PyFieldPath, PyFragmentPort, PyGraphEdge, PyGraphFragment, PyGraphNode, PyGraphStats,
    PyKeyframe, PyLayeredSceneObject, PyLintIssue, PyLintSeverity, PyNodeGraph,
    PyOperationLogEntry, PyOperationStatus, PyQuickFixCandidate, PyResourceConfig, PyScriptBuilder,
    PySemanticValue, PyStoryGraph, PyStoryNode, PyTimeline, PyTraceAtom, PyTraceEdge, PyTrack,
    PyVerificationRun, PyVnConfig, StepResult,
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
    m.add_function(wrap_pyfunction!(default_player_menu_config, m)?)?;
    m.add_function(wrap_pyfunction!(validate_player_menu_config, m)?)?;
    m.add("PyEngine", m.getattr("Engine")?)?;
    Ok(())
}

#[pyfunction]
fn run_visual_novel(script_json: String, config: Option<PyVnConfig>) -> PyResult<()> {
    serde_json::from_str::<::visual_novel_engine::runtime::ScriptRaw>(&script_json)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
    if let Some(config) = config {
        if let Some(menu_json) = config.player_menu_json {
            let menu: ::visual_novel_engine::PlayerMenuConfig = serde_json::from_str(&menu_json)
                .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
            menu.normalized()
                .validate()
                .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
        }
    }
    Err(pyo3::exceptions::PyRuntimeError::new_err(
        "GUI launch is not available in the headless Python extension; use the Rust GUI binary.",
    ))
}

#[pyfunction]
#[pyo3(signature = (project_root, output_root, entry_script=None, target="windows", runtime_artifact=None, require_executable=false))]
fn export_bundle(
    project_root: String,
    output_root: String,
    entry_script: Option<String>,
    target: &str,
    runtime_artifact: Option<String>,
    require_executable: bool,
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
    let spec = ::visual_novel_engine::ExportBundleSpec {
        project_root: project_root.into(),
        output_root: output_root.into(),
        target_platform,
        entry_script: entry_script.map(Into::into),
        runtime_artifact: runtime_artifact.map(Into::into),
        integrity: ::visual_novel_engine::BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    };
    let report = if require_executable {
        ::visual_novel_engine::export_executable_bundle(spec)
    } else {
        ::visual_novel_engine::export_bundle(spec)
    }
    .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
    serde_json::to_string_pretty(&report)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))
}

#[pyfunction]
fn default_player_menu_config() -> PyResult<String> {
    serde_json::to_string_pretty(&::visual_novel_engine::PlayerMenuConfig::default())
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))
}

#[pyfunction]
fn validate_player_menu_config(config_json: String) -> PyResult<String> {
    let config: ::visual_novel_engine::PlayerMenuConfig = serde_json::from_str(&config_json)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
    let normalized = config.normalized();
    normalized
        .validate()
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
    serde_json::to_string_pretty(&normalized)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))
}
