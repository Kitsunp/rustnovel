use visual_novel_engine::authoring::AuthoringCommand;

use super::super::api_v2::{PyFragmentPort, PyGraphFragment};
use super::super::diagnostics::PyLintIssue;
use super::PyNodeGraph;

impl PyNodeGraph {
    pub(super) fn py_create_fragment(
        &mut self,
        fragment_id: String,
        title: String,
        node_ids: Vec<u32>,
    ) -> bool {
        self.apply_authoring_command(AuthoringCommand::CreateFragment {
            fragment_id,
            title,
            node_ids,
        })
        .is_ok()
    }

    pub(super) fn py_remove_fragment(&mut self, fragment_id: &str) -> bool {
        self.apply_authoring_command(AuthoringCommand::RemoveFragment {
            fragment_id: fragment_id.to_string(),
        })
        .is_ok()
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
        self.apply_authoring_command(AuthoringCommand::EnterFragment {
            fragment_id: fragment_id.to_string(),
        })
        .is_ok()
    }

    pub(super) fn py_leave_fragment(&mut self) -> bool {
        self.apply_authoring_command(AuthoringCommand::LeaveFragment)
            .is_ok()
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
        self.apply_authoring_command(AuthoringCommand::RefreshFragmentPorts {
            fragment_id: fragment_id.to_string(),
        })
        .is_ok()
    }

    pub(super) fn py_validate_fragments(&self) -> Vec<PyLintIssue> {
        self.inner
            .validate_fragments()
            .into_iter()
            .map(PyLintIssue::from)
            .collect()
    }
}
