use visual_novel_engine::authoring::{AuthoringReportFingerprint, LintIssue, OperationKind};

pub(super) struct PythonOperationTrace {
    pub(super) fingerprint: AuthoringReportFingerprint,
    pub(super) issues: Vec<LintIssue>,
}

pub(super) struct PythonOperation {
    pub(super) kind: OperationKind,
    pub(super) details: String,
    pub(super) field_path: Option<String>,
    pub(super) before_value: Option<String>,
    pub(super) after_value: Option<String>,
    pub(super) diagnostic: Option<LintIssue>,
}

impl PythonOperation {
    pub(super) fn new(kind: OperationKind, details: impl Into<String>) -> Self {
        Self {
            kind,
            details: details.into(),
            field_path: None,
            before_value: None,
            after_value: None,
            diagnostic: None,
        }
    }

    pub(super) fn with_field_path(mut self, field_path: impl Into<String>) -> Self {
        self.field_path = Some(field_path.into());
        self
    }

    pub(super) fn with_values(
        mut self,
        before_value: Option<String>,
        after_value: Option<String>,
    ) -> Self {
        self.before_value = before_value;
        self.after_value = after_value;
        self
    }

    pub(super) fn with_diagnostic(mut self, issue: &LintIssue) -> Self {
        self.diagnostic = Some(issue.clone());
        self
    }
}
