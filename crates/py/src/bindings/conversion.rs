use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
use visual_novel_engine::runtime::{CmpOp, UiState, UiView};

pub fn ui_state_to_python(ui: &UiState, py: Python<'_>) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    match &ui.view {
        UiView::Dialogue { speaker, text } => {
            dict.set_item("type", "dialogue")?;
            dict.set_item("speaker", speaker)?;
            dict.set_item("text", text)?;
        }
        UiView::Choice { prompt, options } => {
            dict.set_item("type", "choice")?;
            dict.set_item("prompt", prompt)?;
            let list = PyList::empty(py);
            for option in options {
                list.append(option)?;
            }
            dict.set_item("options", list)?;
        }
        UiView::Scene { description } => {
            dict.set_item("type", "scene")?;
            dict.set_item("description", description)?;
        }
        UiView::System { message } => {
            dict.set_item("type", "system")?;
            dict.set_item("message", message)?;
        }
    }
    Ok(dict.into())
}

pub(super) fn parse_cmp_op(op: &str) -> PyResult<CmpOp> {
    match op {
        "eq" => Ok(CmpOp::Eq),
        "ne" => Ok(CmpOp::Ne),
        "lt" => Ok(CmpOp::Lt),
        "le" => Ok(CmpOp::Le),
        "gt" => Ok(CmpOp::Gt),
        "ge" => Ok(CmpOp::Ge),
        _ => Err(PyValueError::new_err(format!(
            "Unknown comparison op '{op}'"
        ))),
    }
}
