use super::{EventCompiled, EventRaw};
use crate::event_behavior::{event_kind_for_compiled, event_kind_for_raw};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{PyTraverseError, PyVisit};
use std::cell::RefCell;

#[derive(Clone, Debug)]
enum PyEventData {
    Raw(EventRaw),
    Compiled(EventCompiled),
}

#[pyo3::pyclass(unsendable)]
#[derive(Debug)]
pub struct PyEvent {
    data: PyEventData,
    cached_dict: RefCell<Option<Py<PyAny>>>,
}

impl PyEvent {
    pub(super) fn from_raw(event: EventRaw) -> Self {
        Self {
            data: PyEventData::Raw(event),
            cached_dict: RefCell::new(None),
        }
    }

    pub(super) fn from_compiled(event: EventCompiled) -> Self {
        Self {
            data: PyEventData::Compiled(event),
            cached_dict: RefCell::new(None),
        }
    }

    fn event_type(&self) -> &'static str {
        match &self.data {
            PyEventData::Raw(event) => event_kind_for_raw(event).stable_name(),
            PyEventData::Compiled(event) => event_kind_for_compiled(event).stable_name(),
        }
    }

    fn field(&self, py: Python<'_>, key: &str) -> PyResult<Option<Py<PyAny>>> {
        let dict = self.as_dict(py)?;
        Ok(dict
            .bind(py)
            .cast::<PyDict>()?
            .get_item(key)?
            .map(Bound::unbind))
    }
}

#[pyo3::pymethods]
impl PyEvent {
    #[getter]
    fn r#type(&self) -> &str {
        self.event_type()
    }

    #[getter]
    fn speaker(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "speaker")
    }

    #[getter]
    fn text(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "text")
    }

    #[getter]
    fn prompt(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "prompt")
    }

    #[getter]
    fn background(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "background")
    }

    #[getter]
    fn music(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "music")
    }

    #[getter]
    fn target(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "target")
    }

    #[getter]
    fn target_ip(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "target_ip")
    }

    #[getter]
    fn key(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "key")
    }

    #[getter]
    fn flag_id(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "flag_id")
    }

    #[getter]
    fn value(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "value")
    }

    #[getter]
    fn options(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "options")
    }

    #[getter]
    fn characters(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "characters")
    }

    #[getter]
    fn add(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "add")
    }

    #[getter]
    fn update(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "update")
    }

    #[getter]
    fn remove(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.field(py, "remove")
    }

    fn __getattr__(&self, py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        self.field(py, name)?.ok_or_else(|| {
            pyo3::exceptions::PyAttributeError::new_err(format!(
                "event '{}' has no attribute '{name}'",
                self.event_type()
            ))
        })
    }

    fn as_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(cached) = self.cached_dict.borrow().as_ref() {
            return Ok(cached.clone_ref(py));
        }
        let dict = match &self.data {
            PyEventData::Raw(event) => event.to_python_dict(py)?,
            PyEventData::Compiled(event) => event.to_python_dict(py)?,
        };
        *self.cached_dict.borrow_mut() = Some(dict.clone_ref(py));
        Ok(dict)
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.as_dict(py)
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        if let Some(dict) = self.cached_dict.borrow().as_ref() {
            visit.call(dict)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        *self.cached_dict.get_mut() = None;
    }
}
