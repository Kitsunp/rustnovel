use visual_novel_engine::authoring::OperationKind;

use super::super::api_v2::{PyFragmentPort, PyGraphFragment};
use super::super::diagnostics::PyLintIssue;
use super::{PyNodeGraph, PythonOperation};

impl PyNodeGraph {
    pub(super) fn py_create_fragment(
        &mut self,
        fragment_id: String,
        title: String,
        node_ids: Vec<u32>,
    ) -> bool {
        let before = self.trace_before_mutation();
        let changed = self
            .inner
            .create_fragment(fragment_id.clone(), title, node_ids.clone());
        if changed {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::FragmentCreated,
                    format!("Created fragment {fragment_id} from Python"),
                )
                .with_field_path(format!("graph.fragments[{fragment_id}]"))
                .with_values(None, serde_json::to_string(&node_ids).ok()),
                before,
            );
        }
        changed
    }

    pub(super) fn py_remove_fragment(&mut self, fragment_id: &str) -> bool {
        let before_value = self
            .inner
            .get_fragment(fragment_id)
            .and_then(|fragment| serde_json::to_string(fragment).ok());
        let before = self.trace_before_mutation();
        let changed = self.inner.remove_fragment(fragment_id).is_some();
        if changed {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::FragmentRemoved,
                    format!("Removed fragment {fragment_id} from Python"),
                )
                .with_field_path(format!("graph.fragments[{fragment_id}]"))
                .with_values(before_value, None),
                before,
            );
        }
        changed
    }

    pub(super) fn py_list_fragments(&self) -> Vec<PyGraphFragment> {
        self.inner
            .list_fragments()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub(super) fn py_get_fragment(&self, fragment_id: &str) -> Option<PyGraphFragment> {
        self.inner
            .get_fragment(fragment_id)
            .cloned()
            .map(Into::into)
    }

    pub(super) fn py_enter_fragment(&mut self, fragment_id: &str) -> bool {
        let before_value = self.inner.active_fragment().map(str::to_string);
        let before = self.trace_before_mutation();
        let changed = self.inner.enter_fragment(fragment_id);
        if changed {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::FragmentEntered,
                    format!("Entered fragment {fragment_id} from Python"),
                )
                .with_field_path("graph.active_fragment")
                .with_values(
                    before_value,
                    self.inner.active_fragment().map(str::to_string),
                ),
                before,
            );
        }
        changed
    }

    pub(super) fn py_leave_fragment(&mut self) -> bool {
        let before_value = self.inner.active_fragment().map(str::to_string);
        let before = self.trace_before_mutation();
        let changed = self.inner.leave_fragment();
        if changed {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::FragmentLeft,
                    "Left active fragment from Python",
                )
                .with_field_path("graph.active_fragment")
                .with_values(
                    before_value,
                    self.inner.active_fragment().map(str::to_string),
                ),
                before,
            );
        }
        changed
    }

    pub(super) fn py_active_fragment(&self) -> Option<String> {
        self.inner.active_fragment().map(str::to_string)
    }

    pub(super) fn py_fragment_ports(
        &self,
        fragment_id: &str,
    ) -> Option<(Vec<PyFragmentPort>, Vec<PyFragmentPort>)> {
        self.inner
            .fragment_ports(fragment_id)
            .map(|(inputs, outputs)| {
                (
                    inputs.into_iter().map(Into::into).collect(),
                    outputs.into_iter().map(Into::into).collect(),
                )
            })
    }

    pub(super) fn py_refresh_fragment_ports(&mut self, fragment_id: &str) -> bool {
        let before_value = self
            .inner
            .get_fragment(fragment_id)
            .and_then(|fragment| serde_json::to_string(fragment).ok());
        let before = self.trace_before_mutation();
        let changed = self.inner.refresh_fragment_ports(fragment_id);
        if changed {
            let after_value = self
                .inner
                .get_fragment(fragment_id)
                .and_then(|fragment| serde_json::to_string(fragment).ok());
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::FieldEdited,
                    format!("Refreshed fragment {fragment_id} ports from Python"),
                )
                .with_field_path(format!("graph.fragments[{fragment_id}].ports"))
                .with_values(before_value, after_value),
                before,
            );
        }
        changed
    }

    pub(super) fn py_validate_fragments(&self) -> Vec<PyLintIssue> {
        self.inner
            .validate_fragments()
            .into_iter()
            .map(PyLintIssue::from)
            .collect()
    }
}
