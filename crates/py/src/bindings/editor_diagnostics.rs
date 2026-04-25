use pyo3::prelude::*;
use visual_novel_gui::editor::quick_fix::QuickFixCandidate;
use visual_novel_gui::editor::{DiagnosticLanguage, LintIssue, LintSeverity};

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
    pub(crate) docs_ref: String,
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
}

impl From<LintIssue> for PyLintIssue {
    fn from(issue: LintIssue) -> Self {
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
            root_cause_es: issue.explanation(DiagnosticLanguage::Es).root_cause,
            root_cause_en: issue.explanation(DiagnosticLanguage::En).root_cause,
            why_failed_es: issue.explanation(DiagnosticLanguage::Es).why_failed,
            why_failed_en: issue.explanation(DiagnosticLanguage::En).why_failed,
            how_to_fix_es: issue.explanation(DiagnosticLanguage::Es).how_to_fix,
            how_to_fix_en: issue.explanation(DiagnosticLanguage::En).how_to_fix,
            docs_ref: issue.explanation(DiagnosticLanguage::En).docs_ref,
        }
    }
}
