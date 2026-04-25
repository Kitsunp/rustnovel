use pyo3::prelude::*;
use visual_novel_engine::{ResourceLimiter, VnError};

pub fn vn_error_to_py(err: VnError) -> PyErr {
    let report = miette::Report::new(err);
    pyo3::exceptions::PyValueError::new_err(report.to_string())
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
}

#[pymethods]
impl PyVnConfig {
    #[new]
    #[pyo3(signature = (title=None, width=None, height=None, fullscreen=None, scale_factor=None, assets_root=None, asset_cache_budget_mb=None, security_mode=None, manifest_path=None, require_manifest=None))]
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
        }
    }
}
