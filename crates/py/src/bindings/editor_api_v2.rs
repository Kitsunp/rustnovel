use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use visual_novel_engine::authoring::composer::{
    ComposerPreviewSession, ComposerSnapshot, LayeredSceneObject, StageLayerKind,
};
use visual_novel_engine::authoring::{
    authoring_fingerprints_semantically_match, AuthoringValidationReport, DiagnosticTarget,
    EvidenceTrace, FieldPath, FragmentPort, GraphFragment, NodeGraph, OperationLogEntry,
    SemanticValue, TraceAtom, TraceEdge, VerificationRun,
};

macro_rules! json_wrapper {
    ($py_type:ident, $py_name:literal, $inner_type:ty) => {
        #[pyclass(name = $py_name)]
        #[derive(Clone)]
        pub struct $py_type {
            inner: $inner_type,
        }

        #[pymethods]
        impl $py_type {
            #[staticmethod]
            fn from_json(source: &str) -> PyResult<Self> {
                let inner = serde_json::from_str(source)
                    .map_err(|err| PyValueError::new_err(err.to_string()))?;
                Ok(Self { inner })
            }

            fn to_json(&self) -> PyResult<String> {
                serde_json::to_string_pretty(&self.inner)
                    .map_err(|err| PyValueError::new_err(err.to_string()))
            }

            fn __repr__(&self) -> String {
                format!("{}({})", $py_name, self.to_json().unwrap_or_default())
            }
        }

        impl From<$inner_type> for $py_type {
            fn from(inner: $inner_type) -> Self {
                Self { inner }
            }
        }
    };
}

json_wrapper!(PyDiagnosticTarget, "DiagnosticTarget", DiagnosticTarget);
json_wrapper!(PyFieldPath, "FieldPath", FieldPath);
json_wrapper!(PySemanticValue, "SemanticValue", SemanticValue);
json_wrapper!(PyEvidenceTrace, "EvidenceTrace", EvidenceTrace);
json_wrapper!(PyTraceAtom, "TraceAtom", TraceAtom);
json_wrapper!(PyTraceEdge, "TraceEdge", TraceEdge);
json_wrapper!(PyOperationLogEntry, "OperationLogEntry", OperationLogEntry);
json_wrapper!(PyVerificationRun, "VerificationRun", VerificationRun);

#[pyclass(name = "FragmentPort")]
#[derive(Clone)]
pub struct PyFragmentPort {
    inner: FragmentPort,
}

#[pymethods]
impl PyFragmentPort {
    #[getter]
    fn port_id(&self) -> String {
        self.inner.port_id.clone()
    }

    #[getter]
    fn label(&self) -> String {
        self.inner.label.clone()
    }

    #[getter]
    fn node_id(&self) -> Option<u32> {
        self.inner.node_id
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    #[staticmethod]
    fn from_json(source: &str) -> PyResult<Self> {
        let inner =
            serde_json::from_str(source).map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "FragmentPort({}, node={:?})",
            self.inner.port_id, self.inner.node_id
        )
    }
}

impl From<FragmentPort> for PyFragmentPort {
    fn from(inner: FragmentPort) -> Self {
        Self { inner }
    }
}

#[pyclass(name = "GraphFragment")]
#[derive(Clone)]
pub struct PyGraphFragment {
    inner: GraphFragment,
}

#[pymethods]
impl PyGraphFragment {
    #[getter]
    fn fragment_id(&self) -> String {
        self.inner.fragment_id.clone()
    }

    #[getter]
    fn title(&self) -> String {
        self.inner.title.clone()
    }

    #[getter]
    fn node_ids(&self) -> Vec<u32> {
        self.inner.node_ids.clone()
    }

    #[getter]
    fn inputs(&self) -> Vec<PyFragmentPort> {
        self.inner
            .inputs
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    #[getter]
    fn outputs(&self) -> Vec<PyFragmentPort> {
        self.inner
            .outputs
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    #[staticmethod]
    fn from_json(source: &str) -> PyResult<Self> {
        let inner =
            serde_json::from_str(source).map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "GraphFragment({}, nodes={})",
            self.inner.fragment_id,
            self.inner.node_ids.len()
        )
    }
}

impl From<GraphFragment> for PyGraphFragment {
    fn from(inner: GraphFragment) -> Self {
        Self { inner }
    }
}

#[pyclass(name = "LayeredSceneObject")]
#[derive(Clone)]
pub struct PyLayeredSceneObject {
    inner: LayeredSceneObject,
}

#[pymethods]
impl PyLayeredSceneObject {
    #[getter]
    fn object_id(&self) -> String {
        self.inner.object_id.clone()
    }

    #[getter]
    fn layer_id(&self) -> String {
        self.inner.layer_id.clone()
    }

    #[getter]
    fn source_node_id(&self) -> Option<u32> {
        self.inner.source_node_id
    }

    #[getter]
    fn source_field_path(&self) -> String {
        self.inner.source_field_path.clone()
    }

    #[getter]
    fn asset_path(&self) -> Option<String> {
        self.inner.asset_path.clone()
    }

    #[getter]
    fn character_name(&self) -> Option<String> {
        self.inner.character_name.clone()
    }

    #[getter]
    fn expression(&self) -> Option<String> {
        self.inner.expression.clone()
    }

    #[getter]
    fn x(&self) -> Option<i32> {
        self.inner.x
    }

    #[getter]
    fn y(&self) -> Option<i32> {
        self.inner.y
    }

    #[getter]
    fn scale(&self) -> Option<f32> {
        self.inner.scale
    }

    #[getter]
    fn z_index(&self) -> i32 {
        self.inner.z_index
    }

    #[getter]
    fn visible(&self) -> bool {
        self.inner.visible
    }

    #[getter]
    fn locked(&self) -> bool {
        self.inner.locked
    }

    #[getter]
    fn kind(&self) -> String {
        format!("{:?}", self.inner.kind)
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn __repr__(&self) -> String {
        format!("LayeredSceneObject({})", self.inner.object_id)
    }
}

impl From<LayeredSceneObject> for PyLayeredSceneObject {
    fn from(inner: LayeredSceneObject) -> Self {
        Self { inner }
    }
}

#[pyclass(name = "ComposerSnapshot")]
#[derive(Clone)]
pub struct PyComposerSnapshot {
    inner: ComposerSnapshot,
}

#[pymethods]
impl PyComposerSnapshot {
    #[getter]
    fn schema(&self) -> String {
        self.inner.schema.clone()
    }

    #[getter]
    fn stage_width(&self) -> u32 {
        self.inner.stage_width
    }

    #[getter]
    fn stage_height(&self) -> u32 {
        self.inner.stage_height
    }

    #[getter]
    fn objects(&self) -> Vec<PyLayeredSceneObject> {
        self.inner
            .objects
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    #[getter]
    fn overlays_json(&self) -> Vec<String> {
        self.inner
            .overlays
            .iter()
            .filter_map(|overlay| serde_json::to_string(overlay).ok())
            .collect()
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    #[staticmethod]
    fn from_json(source: &str) -> PyResult<Self> {
        let inner =
            serde_json::from_str(source).map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "ComposerSnapshot(objects={}, overlays={})",
            self.inner.objects.len(),
            self.inner.overlays.len()
        )
    }
}

impl From<ComposerSnapshot> for PyComposerSnapshot {
    fn from(inner: ComposerSnapshot) -> Self {
        Self { inner }
    }
}

#[pyclass(name = "ComposerPreviewSession")]
pub struct PyComposerPreviewSession {
    graph: NodeGraph,
    inner: ComposerPreviewSession,
}

#[pymethods]
impl PyComposerPreviewSession {
    fn advance(&mut self) -> PyResult<()> {
        self.inner
            .advance()
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn choose(&mut self, option_index: usize) -> PyResult<()> {
        self.inner
            .choose(option_index)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    #[pyo3(signature = (stage_width=None, stage_height=None, locale=None))]
    fn snapshot(
        &self,
        stage_width: Option<u32>,
        stage_height: Option<u32>,
        locale: Option<&str>,
    ) -> PyComposerSnapshot {
        let resolution = stage_width.zip(stage_height);
        self.inner
            .snapshot(&self.graph, resolution, locale, None)
            .into()
    }

    fn __repr__(&self) -> String {
        "ComposerPreviewSession()".to_string()
    }
}

impl PyComposerPreviewSession {
    pub(super) fn start(graph: &NodeGraph, node_id: u32) -> PyResult<Self> {
        let inner = ComposerPreviewSession::start_from_node(graph, node_id)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self {
            graph: graph.clone(),
            inner,
        })
    }
}

#[pyclass(name = "AuthoringValidationReport")]
#[derive(Clone)]
pub struct PyAuthoringValidationReport {
    inner: AuthoringValidationReport,
}

#[pymethods]
impl PyAuthoringValidationReport {
    #[staticmethod]
    fn from_json(source: &str) -> PyResult<Self> {
        let inner = AuthoringValidationReport::from_json(source)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        self.inner
            .to_json()
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    #[getter]
    fn schema(&self) -> String {
        self.inner.schema.clone()
    }

    #[getter]
    fn issue_count(&self) -> usize {
        self.inner.issue_count
    }

    #[getter]
    fn error_count(&self) -> usize {
        self.inner.error_count
    }

    #[getter]
    fn warning_count(&self) -> usize {
        self.inner.warning_count
    }

    #[getter]
    fn info_count(&self) -> usize {
        self.inner.info_count
    }

    #[getter]
    fn diagnostic_ids(&self) -> Vec<String> {
        self.inner
            .issues
            .iter()
            .map(|issue| issue.diagnostic_id.clone())
            .collect()
    }

    fn issues(&self) -> Vec<String> {
        self.inner
            .issues
            .iter()
            .filter_map(|issue| serde_json::to_string(issue).ok())
            .collect()
    }

    fn issues_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner.issues)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn explain(&self, diagnostic_id: &str) -> Option<String> {
        self.inner
            .explain(diagnostic_id)
            .and_then(|issue| serde_json::to_string_pretty(issue).ok())
    }

    fn fingerprints_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner.fingerprints)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn is_stale_against(&self, current_fingerprints_json: &str) -> PyResult<bool> {
        let imported = serde_json::to_value(&self.inner.fingerprints)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        let current: serde_json::Value = serde_json::from_str(current_fingerprints_json)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(!authoring_fingerprints_semantically_match(
            &imported, &current,
        ))
    }

    fn is_stale_against_report(&self, current: &Self) -> PyResult<bool> {
        let imported = serde_json::to_value(&self.inner.fingerprints)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        let current = serde_json::to_value(&current.inner.fingerprints)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(!authoring_fingerprints_semantically_match(
            &imported, &current,
        ))
    }

    fn __repr__(&self) -> String {
        format!(
            "AuthoringValidationReport(issues={}, errors={})",
            self.inner.issue_count, self.inner.error_count
        )
    }
}

impl From<AuthoringValidationReport> for PyAuthoringValidationReport {
    fn from(inner: AuthoringValidationReport) -> Self {
        Self { inner }
    }
}

pub(super) fn stage_layer_names() -> Vec<String> {
    visual_novel_engine::authoring::composer::list_stage_layers()
        .into_iter()
        .map(stage_layer_label)
        .collect()
}

fn stage_layer_label(layer: StageLayerKind) -> String {
    serde_json::to_string(&layer)
        .unwrap_or_else(|_| format!("{layer:?}"))
        .trim_matches('"')
        .to_string()
}

#[cfg(test)]
#[path = "editor_api_v2_tests.rs"]
mod tests;
