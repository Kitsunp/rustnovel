use visual_novel_engine::authoring::composer::{apply_layer_overrides, list_layered_objects};
use visual_novel_engine::authoring::{
    build_authoring_document_report_fingerprint, validate_authoring_graph_no_io, AuthoringDocument,
    AuthoringReportFingerprint, OperationLogEntry, VerificationRun,
};

use super::{PyNodeGraph, PythonOperation, PythonOperationTrace};

impl PyNodeGraph {
    pub(crate) fn inner(&self) -> &visual_novel_engine::authoring::NodeGraph {
        &self.inner
    }

    pub(super) fn to_authoring_document(&self) -> AuthoringDocument {
        let mut document = AuthoringDocument::new(self.inner.clone());
        document.composer_layer_overrides = self.layer_overrides.clone();
        document.operation_log = self.operation_log.clone();
        document.verification_runs = self.verification_runs.clone();
        document
    }

    pub(super) fn current_fingerprint(&self) -> AuthoringReportFingerprint {
        let script = self.inner.to_script_lossy_for_diagnostics();
        build_authoring_document_report_fingerprint(&self.to_authoring_document(), &script)
    }

    pub(super) fn layered_object_json(&self, object_id: &str) -> Option<String> {
        let mut objects = list_layered_objects(&self.inner, None);
        apply_layer_overrides(&mut objects, &self.layer_overrides);
        objects
            .into_iter()
            .find(|object| object.object_id == object_id)
            .and_then(|object| serde_json::to_string(&object).ok())
    }

    pub(super) fn trace_before_mutation(&self) -> PythonOperationTrace {
        PythonOperationTrace {
            fingerprint: self.current_fingerprint(),
            issues: validate_authoring_graph_no_io(&self.inner),
        }
    }

    pub(super) fn record_python_operation(
        &mut self,
        operation: PythonOperation,
        before: PythonOperationTrace,
    ) {
        let after_fingerprint = self.current_fingerprint();
        let after_issues = validate_authoring_graph_no_io(&self.inner);
        let mut entry = OperationLogEntry::new_typed(operation.kind, "applied", operation.details)
            .with_before_after_fingerprints(&before.fingerprint, &after_fingerprint);
        if let Some(issue) = &operation.diagnostic {
            entry = entry.with_diagnostic(issue);
        }
        if let Some(field_path) = operation.field_path {
            entry = entry.with_field_path(field_path);
        }
        entry.before_value = operation.before_value;
        entry.after_value = operation.after_value;
        let operation_id = entry.operation_id.clone();
        self.operation_log.push(entry);
        self.verification_runs
            .push(VerificationRun::from_diagnostics(
                operation_id,
                "python-api",
                &after_fingerprint,
                &before.issues,
                &after_issues,
            ));
    }
}
