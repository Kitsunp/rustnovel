use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use visual_novel_engine::{
    ExportBundleReport, ExportPlan, LayoutResolution, ResourceLimiter, RouteTree, SceneFrame,
    UiThemeValidationReport, VnError as CoreVnError,
};

create_exception!(visual_novel_engine, VnError, PyException);
create_exception!(visual_novel_engine, VnValidationError, VnError);
create_exception!(visual_novel_engine, VnSecurityPolicyError, VnError);
create_exception!(visual_novel_engine, VnResourceLimitError, VnError);
create_exception!(visual_novel_engine, VnEndOfScriptError, VnError);

pub fn register_error_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("VnError", py.get_type::<VnError>())?;
    m.add("VnValidationError", py.get_type::<VnValidationError>())?;
    m.add(
        "VnSecurityPolicyError",
        py.get_type::<VnSecurityPolicyError>(),
    )?;
    m.add(
        "VnResourceLimitError",
        py.get_type::<VnResourceLimitError>(),
    )?;
    m.add("VnEndOfScriptError", py.get_type::<VnEndOfScriptError>())?;
    Ok(())
}

pub fn vn_error_to_py(err: CoreVnError) -> PyErr {
    let kind = match &err {
        CoreVnError::InvalidScript(_)
        | CoreVnError::InvalidChoice
        | CoreVnError::Serialization { .. }
        | CoreVnError::BinaryFormat(_) => VnErrorKind::Validation,
        CoreVnError::SecurityPolicy(_) => VnErrorKind::SecurityPolicy,
        CoreVnError::ResourceLimit(_) => VnErrorKind::ResourceLimit,
        CoreVnError::EndOfScript => VnErrorKind::EndOfScript,
    };
    let report = miette::Report::new(err);
    let message = report.to_string();
    match kind {
        VnErrorKind::Validation => VnValidationError::new_err(message),
        VnErrorKind::SecurityPolicy => VnSecurityPolicyError::new_err(message),
        VnErrorKind::ResourceLimit => VnResourceLimitError::new_err(message),
        VnErrorKind::EndOfScript => VnEndOfScriptError::new_err(message),
    }
}

#[derive(Clone, Copy, Debug)]
enum VnErrorKind {
    Validation,
    SecurityPolicy,
    ResourceLimit,
    EndOfScript,
}

#[pyclass(name = "ResourceConfig")]
#[derive(Clone, Debug)]
pub struct PyResourceConfig {
    #[pyo3(get, set)]
    pub max_texture_memory: usize,
    #[pyo3(get, set)]
    pub max_script_bytes: usize,
}

#[pymethods]
impl PyResourceConfig {
    #[new]
    #[pyo3(signature = (max_texture_memory=None, max_script_bytes=None))]
    fn new(max_texture_memory: Option<usize>, max_script_bytes: Option<usize>) -> Self {
        Self {
            max_texture_memory: max_texture_memory.unwrap_or(512 * 1024 * 1024),
            max_script_bytes: max_script_bytes
                .unwrap_or(ResourceLimiter::default().max_script_bytes),
        }
    }
}

#[pyclass(name = "VnConfig")]
#[derive(Clone, Debug)]
pub struct PyVnConfig {
    #[pyo3(get, set)]
    pub title: Option<String>,
    #[pyo3(get, set)]
    pub width: Option<f32>,
    #[pyo3(get, set)]
    pub height: Option<f32>,
    #[pyo3(get, set)]
    pub fullscreen: Option<bool>,
    #[pyo3(get, set)]
    pub scale_factor: Option<f32>,
    #[pyo3(get, set)]
    pub assets_root: Option<String>,
    #[pyo3(get, set)]
    pub asset_cache_budget_mb: Option<u64>,
    #[pyo3(get, set)]
    pub security_mode: Option<String>,
    #[pyo3(get, set)]
    pub manifest_path: Option<String>,
    #[pyo3(get, set)]
    pub require_manifest: Option<bool>,
    #[pyo3(get, set)]
    pub preferences_path: Option<String>,
    #[pyo3(get, set)]
    pub player_menu_json: Option<String>,
}

#[pymethods]
impl PyVnConfig {
    #[new]
    #[pyo3(signature = (title=None, width=None, height=None, fullscreen=None, scale_factor=None, assets_root=None, asset_cache_budget_mb=None, security_mode=None, manifest_path=None, require_manifest=None, preferences_path=None, player_menu_json=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        title: Option<String>,
        width: Option<f32>,
        height: Option<f32>,
        fullscreen: Option<bool>,
        scale_factor: Option<f32>,
        assets_root: Option<String>,
        asset_cache_budget_mb: Option<u64>,
        security_mode: Option<String>,
        manifest_path: Option<String>,
        require_manifest: Option<bool>,
        preferences_path: Option<String>,
        player_menu_json: Option<String>,
    ) -> Self {
        Self {
            title,
            width,
            height,
            fullscreen,
            scale_factor,
            assets_root,
            asset_cache_budget_mb,
            security_mode,
            manifest_path,
            require_manifest,
            preferences_path,
            player_menu_json,
        }
    }
}

#[pyclass(name = "ExportPlan")]
#[derive(Clone, Debug)]
pub struct PyExportPlan {
    pub inner: ExportPlan,
}

#[pyclass(name = "ExportReport")]
#[derive(Clone, Debug)]
pub struct PyExportReport {
    pub inner: ExportBundleReport,
}

#[pyclass(name = "RouteTree")]
#[derive(Clone, Debug)]
pub struct PyRouteTree {
    pub inner: RouteTree,
}

#[pyclass(name = "SceneFrame")]
#[derive(Clone, Debug)]
pub struct PySceneFrame {
    pub inner: SceneFrame,
}

#[pyclass(name = "UiThemeValidationReport")]
#[derive(Clone, Debug)]
pub struct PyUiThemeValidationReport {
    pub inner: UiThemeValidationReport,
}

#[pyclass(name = "LayoutResolution")]
#[derive(Clone, Debug)]
pub struct PyLayoutResolution {
    pub inner: LayoutResolution,
}

macro_rules! json_backed_methods {
    ($ty:ty) => {
        #[pymethods]
        impl $ty {
            fn to_json(&self) -> PyResult<String> {
                serde_json::to_string_pretty(&self.inner)
                    .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))
            }

            fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
                let value = serde_json::to_value(&self.inner)
                    .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?;
                json_value_to_py(py, &value)
            }

            fn __repr__(&self) -> String {
                self.to_json()
                    .unwrap_or_else(|err| format!("<serialization error: {err}>"))
            }
        }
    };
}

json_backed_methods!(PyExportPlan);
json_backed_methods!(PyExportReport);
json_backed_methods!(PyRouteTree);
json_backed_methods!(PySceneFrame);
json_backed_methods!(PyUiThemeValidationReport);
json_backed_methods!(PyLayoutResolution);

impl From<ExportPlan> for PyExportPlan {
    fn from(inner: ExportPlan) -> Self {
        Self { inner }
    }
}

impl From<ExportBundleReport> for PyExportReport {
    fn from(inner: ExportBundleReport) -> Self {
        Self { inner }
    }
}

impl From<RouteTree> for PyRouteTree {
    fn from(inner: RouteTree) -> Self {
        Self { inner }
    }
}

impl From<SceneFrame> for PySceneFrame {
    fn from(inner: SceneFrame) -> Self {
        Self { inner }
    }
}

impl From<UiThemeValidationReport> for PyUiThemeValidationReport {
    fn from(inner: UiThemeValidationReport) -> Self {
        Self { inner }
    }
}

impl From<LayoutResolution> for PyLayoutResolution {
    fn from(inner: LayoutResolution) -> Self {
        Self { inner }
    }
}

fn json_value_to_py<'py>(py: Python<'py>, value: &serde_json::Value) -> PyResult<PyObject> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(value) => {
            Ok((*value).into_pyobject(py)?.to_owned().into_any().unbind())
        }
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(value.into_pyobject(py)?.into_any().unbind())
            } else if let Some(value) = value.as_u64() {
                Ok(value.into_pyobject(py)?.into_any().unbind())
            } else if let Some(value) = value.as_f64() {
                Ok(value.into_pyobject(py)?.into_any().unbind())
            } else {
                Ok(py.None())
            }
        }
        serde_json::Value::String(value) => Ok(value.into_pyobject(py)?.into_any().unbind()),
        serde_json::Value::Array(values) => {
            let list = PyList::empty(py);
            for item in values {
                list.append(json_value_to_py(py, item)?)?;
            }
            Ok(list.into())
        }
        serde_json::Value::Object(values) => {
            let dict = PyDict::new(py);
            for (key, value) in values {
                dict.set_item(key, json_value_to_py(py, value)?)?;
            }
            Ok(dict.into())
        }
    }
}
