mod bindings;

use pyo3::prelude::*;

pub use bindings::{
    register_editor_classes, register_error_classes, vn_error_to_py, PyAudio,
    PyAuthoringValidationReport, PyComposerPreviewSession, PyComposerSnapshot, PyDiagnosticTarget,
    PyEngine, PyEvidenceTrace, PyExportPlan, PyExportReport, PyFieldPath, PyFragmentPort,
    PyGraphEdge, PyGraphFragment, PyGraphNode, PyGraphStats, PyKeyframe, PyLayeredSceneObject,
    PyLayoutResolution, PyLintIssue, PyLintSeverity, PyNodeGraph, PyOperationLogEntry,
    PyOperationStatus, PyQuickFixCandidate, PyResourceConfig, PyRouteTree, PySceneFrame,
    PyScriptBuilder, PySemanticValue, PyStoryGraph, PyStoryNode, PyTimeline, PyTraceAtom,
    PyTraceEdge, PyTrack, PyUiThemeValidationReport, PyVerificationRun, PyVnConfig, StepResult,
};

#[pymodule]
fn visual_novel_engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_error_classes(m)?;
    m.add_class::<PyEngine>()?;
    m.add_class::<StepResult>()?;
    m.add_class::<PyAudio>()?;
    m.add_class::<PyResourceConfig>()?;
    m.add_class::<PyExportPlan>()?;
    m.add_class::<PyExportReport>()?;
    m.add_class::<PyRouteTree>()?;
    m.add_class::<PySceneFrame>()?;
    m.add_class::<PyUiThemeValidationReport>()?;
    m.add_class::<PyLayoutResolution>()?;
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
    m.add_function(wrap_pyfunction!(validate_runtime_config, m)?)?;
    m.add_function(wrap_pyfunction!(validate_script_schema_version, m)?)?;
    m.add_function(wrap_pyfunction!(export_bundle, m)?)?;
    m.add_function(wrap_pyfunction!(export_bundle_json, m)?)?;
    m.add_function(wrap_pyfunction!(plan_export, m)?)?;
    m.add_function(wrap_pyfunction!(validate_ui_theme, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_layout, m)?)?;
    m.add_function(wrap_pyfunction!(default_player_menu_config, m)?)?;
    m.add_function(wrap_pyfunction!(validate_player_menu_config, m)?)?;
    m.add("PyEngine", m.getattr("Engine")?)?;
    Ok(())
}

#[pyfunction]
#[pyo3(signature = (script_json, config=None))]
fn validate_runtime_config(script_json: String, config: Option<PyVnConfig>) -> PyResult<()> {
    let script = ::visual_novel_engine::runtime::ScriptRaw::from_json(&script_json)
        .map_err(vn_error_to_py)?;
    ::visual_novel_engine::SecurityPolicy::default()
        .validate_raw(&script, ::visual_novel_engine::ResourceLimiter::default())
        .map_err(vn_error_to_py)?;
    if let Some(config) = config {
        if let Some(menu_json) = config.player_menu_json {
            let menu: ::visual_novel_engine::PlayerMenuConfig = serde_json::from_str(&menu_json)
                .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
            menu.normalized()
                .validate()
                .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
        }
    }
    Ok(())
}

#[pyfunction]
#[pyo3(signature = (script_schema_version=None, policy="strict_current"))]
fn validate_script_schema_version(
    script_schema_version: Option<String>,
    policy: &str,
) -> PyResult<String> {
    let policy = parse_schema_policy(policy)?;
    let report =
        ::visual_novel_engine::validate_script_schema(script_schema_version.as_deref(), policy)
            .map_err(vn_error_to_py)?;
    Ok(report.normalized_version)
}

#[pyfunction]
#[pyo3(signature = (script_json, config=None))]
fn run_visual_novel(script_json: String, config: Option<PyVnConfig>) -> PyResult<()> {
    validate_runtime_config(script_json, config)?;
    Err(pyo3::exceptions::PyRuntimeError::new_err(
        "run_visual_novel cannot launch a GUI from the headless Python extension; validate_runtime_config succeeded, use the Rust GUI binary or Engine for headless execution.",
    ))
}

#[pyfunction]
#[pyo3(signature = (project_root, output_root, entry_script=None, target="windows", runtime_artifact=None, require_executable=false, integrity="none", hmac_key=None, layout_version=1))]
#[allow(clippy::too_many_arguments)]
fn export_bundle(
    project_root: String,
    output_root: String,
    entry_script: Option<String>,
    target: &str,
    runtime_artifact: Option<String>,
    require_executable: bool,
    integrity: &str,
    hmac_key: Option<String>,
    layout_version: u16,
) -> PyResult<PyExportReport> {
    let spec = ::visual_novel_engine::ExportBundleSpec {
        project_root: project_root.into(),
        output_root: output_root.into(),
        target_platform: parse_target_platform(target)?,
        entry_script: entry_script.map(Into::into),
        runtime_artifact: runtime_artifact.map(Into::into),
        integrity: parse_integrity(integrity)?,
        output_layout_version: layout_version,
        hmac_key,
    };
    let report = if require_executable {
        ::visual_novel_engine::export_executable_bundle(spec)
    } else {
        ::visual_novel_engine::export_bundle(spec)
    }
    .map_err(vn_error_to_py)?;
    Ok(report.into())
}

#[pyfunction]
#[pyo3(signature = (project_root, output_root, entry_script=None, target="windows", runtime_artifact=None, require_executable=false, integrity="none", hmac_key=None, layout_version=1))]
#[allow(clippy::too_many_arguments)]
fn export_bundle_json(
    project_root: String,
    output_root: String,
    entry_script: Option<String>,
    target: &str,
    runtime_artifact: Option<String>,
    require_executable: bool,
    integrity: &str,
    hmac_key: Option<String>,
    layout_version: u16,
) -> PyResult<String> {
    let report = export_bundle(
        project_root,
        output_root,
        entry_script,
        target,
        runtime_artifact,
        require_executable,
        integrity,
        hmac_key,
        layout_version,
    )?;
    serde_json::to_string_pretty(&report.inner)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))
}

#[pyfunction]
#[pyo3(signature = (project_root, output_root, entry_script=None, target="windows", runtime_artifact=None, integrity="none", hmac_key=None, layout_version=1))]
#[allow(clippy::too_many_arguments)]
fn plan_export(
    project_root: String,
    output_root: String,
    entry_script: Option<String>,
    target: &str,
    runtime_artifact: Option<String>,
    integrity: &str,
    hmac_key: Option<String>,
    layout_version: u16,
) -> PyResult<PyExportPlan> {
    let spec = ::visual_novel_engine::ExportBundleSpec {
        project_root: project_root.into(),
        output_root: output_root.into(),
        target_platform: parse_target_platform(target)?,
        entry_script: entry_script.map(Into::into),
        runtime_artifact: runtime_artifact.map(Into::into),
        integrity: parse_integrity(integrity)?,
        output_layout_version: layout_version,
        hmac_key,
    };
    ::visual_novel_engine::ExportService::new()
        .plan_export(&spec)
        .map(Into::into)
        .map_err(vn_error_to_py)
}

#[pyfunction]
fn validate_ui_theme(theme_json: String) -> PyResult<PyUiThemeValidationReport> {
    let theme: ::visual_novel_engine::UiTheme = serde_json::from_str(&theme_json)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
    Ok(::visual_novel_engine::validate_ui_theme(&theme).into())
}

#[pyfunction]
#[pyo3(signature = (display_json, stage_json=None, policy_json=None))]
fn resolve_layout(
    display_json: String,
    stage_json: Option<String>,
    policy_json: Option<String>,
) -> PyResult<PyLayoutResolution> {
    let display: ::visual_novel_engine::DisplayProfile = serde_json::from_str(&display_json)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
    let stage = match stage_json {
        Some(raw) => serde_json::from_str(&raw)
            .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?,
        None => ::visual_novel_engine::StageProfile::default(),
    };
    let policy = match policy_json {
        Some(raw) => serde_json::from_str(&raw)
            .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?,
        None => ::visual_novel_engine::LayoutPolicy::default(),
    };
    Ok(::visual_novel_engine::resolve_layout(display, stage, policy).into())
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

fn parse_target_platform(target: &str) -> PyResult<::visual_novel_engine::ExportTargetPlatform> {
    match target.trim().to_ascii_lowercase().as_str() {
        "windows" | "win" => Ok(::visual_novel_engine::ExportTargetPlatform::Windows),
        "linux" => Ok(::visual_novel_engine::ExportTargetPlatform::Linux),
        "macos" | "darwin" | "osx" => Ok(::visual_novel_engine::ExportTargetPlatform::Macos),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown bundle target '{other}'"
        ))),
    }
}

fn parse_integrity(integrity: &str) -> PyResult<::visual_novel_engine::BundleIntegrity> {
    match integrity.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(::visual_novel_engine::BundleIntegrity::None),
        "hmac_sha256" | "hmac-sha256" => Ok(::visual_novel_engine::BundleIntegrity::HmacSha256),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown bundle integrity '{other}'"
        ))),
    }
}

fn parse_schema_policy(policy: &str) -> PyResult<::visual_novel_engine::SchemaPolicy> {
    match policy.trim().to_ascii_lowercase().as_str() {
        "strict_current" => Ok(::visual_novel_engine::SchemaPolicy::StrictCurrent),
        "legacy_read_only" => Ok(::visual_novel_engine::SchemaPolicy::LegacyReadOnly),
        "migrating" => Ok(::visual_novel_engine::SchemaPolicy::Migrating),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown schema policy '{other}'"
        ))),
    }
}
