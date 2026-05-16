use pyo3::prelude::*;
use visual_novel_engine::authoring::{
    DiagnosticLanguage, LintIssue, LintSeverity, QuickFixCandidate,
};

/// Quick-fix candidate metadata exposed to Python.
#[pyclass(name = "QuickFixCandidate")]
#[derive(Clone)]
pub struct PyQuickFixCandidate {
    #[pyo3(get)]
    fix_id: String,
    #[pyo3(get)]
    risk: String,
    #[pyo3(get)]
    structural: bool,
    #[pyo3(get)]
    title_es: String,
    #[pyo3(get)]
    title_en: String,
    #[pyo3(get)]
    preconditions_es: String,
    #[pyo3(get)]
    preconditions_en: String,
    #[pyo3(get)]
    postconditions_es: String,
    #[pyo3(get)]
    postconditions_en: String,
}

#[pymethods]
impl PyQuickFixCandidate {
    fn __repr__(&self) -> String {
        format!(
            "QuickFixCandidate(fix_id='{}', risk='{}', structural={})",
            self.fix_id, self.risk, self.structural
        )
    }
}

impl From<QuickFixCandidate> for PyQuickFixCandidate {
    fn from(candidate: QuickFixCandidate) -> Self {
        Self {
            fix_id: candidate.fix_id.to_string(),
            risk: candidate.risk.label().to_string(),
            structural: candidate.structural,
            title_es: candidate.title_es.to_string(),
            title_en: candidate.title_en.to_string(),
            preconditions_es: candidate.preconditions_es.to_string(),
            preconditions_en: candidate.preconditions_en.to_string(),
            postconditions_es: candidate.postconditions_es.to_string(),
            postconditions_en: candidate.postconditions_en.to_string(),
        }
    }
}

/// Severity level for lint issues.
#[pyclass(name = "LintSeverity")]
#[derive(Clone)]
pub struct PyLintSeverity {
    inner: LintSeverity,
}

#[pymethods]
impl PyLintSeverity {
    #[classattr]
    #[pyo3(name = "Error")]
    fn error() -> Self {
        Self {
            inner: LintSeverity::Error,
        }
    }

    #[classattr]
    #[pyo3(name = "Warning")]
    fn warning() -> Self {
        Self {
            inner: LintSeverity::Warning,
        }
    }

    #[classattr]
    #[pyo3(name = "Info")]
    fn info() -> Self {
        Self {
            inner: LintSeverity::Info,
        }
    }

    fn __repr__(&self) -> String {
        match self.inner {
            LintSeverity::Error => "LintSeverity.Error".to_string(),
            LintSeverity::Warning => "LintSeverity.Warning".to_string(),
            LintSeverity::Info => "LintSeverity.Info".to_string(),
        }
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

/// A validation issue found in the graph.
#[pyclass(name = "LintIssue")]
#[derive(Clone)]
pub struct PyLintIssue {
    #[pyo3(get)]
    pub(crate) severity: PyLintSeverity,
    #[pyo3(get)]
    pub(crate) message: String,
    #[pyo3(get)]
    pub(crate) node_id: Option<u32>,
    #[pyo3(get)]
    pub(crate) event_ip: Option<u32>,
    #[pyo3(get)]
    pub(crate) edge_from: Option<u32>,
    #[pyo3(get)]
    pub(crate) edge_to: Option<u32>,
    #[pyo3(get)]
    pub(crate) asset_path: Option<String>,
    #[pyo3(get)]
    pub(crate) phase: String,
    #[pyo3(get)]
    pub(crate) code: String,
    #[pyo3(get)]
    pub(crate) diagnostic_id: String,
    #[pyo3(get)]
    pub(crate) message_es: String,
    #[pyo3(get)]
    pub(crate) message_en: String,
    #[pyo3(get)]
    pub(crate) message_key: String,
    #[pyo3(get)]
    pub(crate) what_happened_es: String,
    #[pyo3(get)]
    pub(crate) what_happened_en: String,
    #[pyo3(get)]
    pub(crate) root_cause_es: String,
    #[pyo3(get)]
    pub(crate) root_cause_en: String,
    #[pyo3(get)]
    pub(crate) why_failed_es: String,
    #[pyo3(get)]
    pub(crate) why_failed_en: String,
    #[pyo3(get)]
    pub(crate) how_to_fix_es: String,
    #[pyo3(get)]
    pub(crate) how_to_fix_en: String,
    #[pyo3(get)]
    pub(crate) consequence_es: String,
    #[pyo3(get)]
    pub(crate) consequence_en: String,
    #[pyo3(get)]
    pub(crate) action_steps_es: Vec<String>,
    #[pyo3(get)]
    pub(crate) action_steps_en: Vec<String>,
    #[pyo3(get)]
    pub(crate) expected_es: String,
    #[pyo3(get)]
    pub(crate) expected_en: String,
    #[pyo3(get)]
    pub(crate) docs_ref: String,
    #[pyo3(get)]
    pub(crate) target: Option<String>,
    #[pyo3(get)]
    pub(crate) field_path: Option<String>,
    #[pyo3(get)]
    pub(crate) trace_id: Option<String>,
    #[pyo3(get)]
    pub(crate) operation_id: Option<String>,
    #[pyo3(get)]
    pub(crate) blocked_by: Option<String>,
    #[pyo3(get)]
    pub(crate) semantic_values: Vec<String>,
}

#[pymethods]
impl PyLintIssue {
    fn __repr__(&self) -> String {
        format!(
            "LintIssue({}, {}, node={:?}, ip={:?}, diag={})",
            self.severity.__repr__(),
            self.message,
            self.node_id,
            self.event_ip,
            self.diagnostic_id
        )
    }

    fn localized(&self, locale: Option<&str>) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            let use_es = locale.unwrap_or("en").trim().eq_ignore_ascii_case("es");
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("schema", "vnengine.diagnostic_envelope.v2")?;
            dict.set_item("diagnostic_id", &self.diagnostic_id)?;
            dict.set_item("message_key", &self.message_key)?;
            dict.set_item("code", &self.code)?;
            dict.set_item("phase", &self.phase)?;
            dict.set_item("severity", self.severity.__repr__())?;
            dict.set_item(
                "message",
                if use_es {
                    &self.message_es
                } else {
                    &self.message_en
                },
            )?;
            dict.set_item(
                "what_happened",
                if use_es {
                    &self.what_happened_es
                } else {
                    &self.what_happened_en
                },
            )?;
            dict.set_item(
                "root_cause",
                if use_es {
                    &self.root_cause_es
                } else {
                    &self.root_cause_en
                },
            )?;
            dict.set_item(
                "why_failed",
                if use_es {
                    &self.why_failed_es
                } else {
                    &self.why_failed_en
                },
            )?;
            dict.set_item(
                "how_to_fix",
                if use_es {
                    &self.how_to_fix_es
                } else {
                    &self.how_to_fix_en
                },
            )?;
            dict.set_item(
                "consequence",
                if use_es {
                    &self.consequence_es
                } else {
                    &self.consequence_en
                },
            )?;
            dict.set_item(
                "action_steps",
                if use_es {
                    &self.action_steps_es
                } else {
                    &self.action_steps_en
                },
            )?;
            dict.set_item("docs_ref", &self.docs_ref)?;
            dict.set_item("target", &self.target)?;
            dict.set_item("field_path", &self.field_path)?;
            dict.set_item("trace_id", &self.trace_id)?;
            dict.set_item("operation_id", &self.operation_id)?;
            dict.set_item("blocked_by", &self.blocked_by)?;
            dict.set_item("semantic_values", &self.semantic_values)?;
            Ok(dict.into())
        })
    }
}

impl From<LintIssue> for PyLintIssue {
    fn from(issue: LintIssue) -> Self {
        let es = issue.explanation(DiagnosticLanguage::Es);
        let en = issue.explanation(DiagnosticLanguage::En);
        Self {
            severity: PyLintSeverity {
                inner: issue.severity,
            },
            message: issue.message.clone(),
            node_id: issue.node_id,
            event_ip: issue.event_ip,
            edge_from: issue.edge_from,
            edge_to: issue.edge_to,
            asset_path: issue.asset_path.clone(),
            phase: issue.phase.label().to_string(),
            code: issue.code.label().to_string(),
            diagnostic_id: issue.diagnostic_id(),
            message_es: issue.localized_message(DiagnosticLanguage::Es),
            message_en: issue.localized_message(DiagnosticLanguage::En),
            message_key: en.message_key,
            what_happened_es: es.what_happened,
            what_happened_en: en.what_happened,
            root_cause_es: es.root_cause,
            root_cause_en: en.root_cause,
            why_failed_es: es.why_failed,
            why_failed_en: en.why_failed,
            how_to_fix_es: es.how_to_fix,
            how_to_fix_en: en.how_to_fix,
            consequence_es: es.consequence,
            consequence_en: en.consequence,
            action_steps_es: es.action_steps,
            action_steps_en: en.action_steps,
            expected_es: es.expected,
            expected_en: en.expected,
            docs_ref: en.docs_ref,
            target: issue
                .target
                .as_ref()
                .and_then(|target| serde_json::to_string(target).ok()),
            field_path: issue.field_path.as_ref().map(|path| path.value.clone()),
            trace_id: issue
                .evidence_trace
                .as_ref()
                .map(|trace| trace.trace_id.clone()),
            operation_id: issue.operation_id.clone(),
            blocked_by: issue.blocked_by.clone(),
            semantic_values: issue
                .semantic_values
                .iter()
                .filter_map(|value| serde_json::to_string(value).ok())
                .collect(),
        }
    }
}
