use pyo3::{exceptions::PyValueError, prelude::*};
use std::path::Path;
use visual_novel_engine::manifest::{
    import_project_asset, AssetImportKind, ProjectManifest, DEFAULT_ASSET_IMPORT_LIMIT,
};

#[pyfunction]
#[pyo3(signature = (manifest_path, source, kind, max_bytes=DEFAULT_ASSET_IMPORT_LIMIT))]
pub fn import_project_asset_json(
    manifest_path: &str,
    source: &str,
    kind: &str,
    max_bytes: u64,
) -> PyResult<String> {
    let path = Path::new(manifest_path);
    let root = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let kind: AssetImportKind = kind.parse().map_err(PyValueError::new_err)?;
    let mut manifest =
        ProjectManifest::load(path).map_err(|error| PyValueError::new_err(error.to_string()))?;
    let result = import_project_asset(
        root,
        path,
        &mut manifest,
        Path::new(source),
        kind,
        max_bytes,
    )
    .map_err(|error| PyValueError::new_err(error.to_string()))?;
    serde_json::to_string(&result).map_err(|error| PyValueError::new_err(error.to_string()))
}
