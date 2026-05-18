use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use visual_novel_engine::authoring::{FragmentPort, GraphFragment};

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
    pub(super) fn fragment_id(&self) -> String {
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

    pub(super) fn to_json(&self) -> PyResult<String> {
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
