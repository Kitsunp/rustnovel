use super::super::{DiagnosticTarget, OperationKind};
use super::{AuthoringCommandBus, AuthoringDelta, CommandApplyResult};

impl AuthoringCommandBus {
    pub(super) fn create_fragment(
        &mut self,
        fragment_id: &str,
        title: &str,
        node_ids: &[u32],
    ) -> CommandApplyResult {
        if !self.graph.create_fragment(
            fragment_id.to_string(),
            title.to_string(),
            node_ids.to_vec(),
        ) {
            return Err(format!("fragment '{fragment_id}' could not be created"));
        }
        let fragment = self
            .graph
            .get_fragment(fragment_id)
            .cloned()
            .ok_or_else(|| format!("fragment '{fragment_id}' missing after create"))?;
        Ok((
            AuthoringDelta::FragmentCreated { fragment },
            OperationKind::FragmentCreated,
            Some(format!("graph.fragments[{fragment_id}]")),
            Some(DiagnosticTarget::Graph),
        ))
    }

    pub(super) fn remove_fragment(&mut self, fragment_id: &str) -> CommandApplyResult {
        let before_stack = self.graph.graph_stack_for_command_bus();
        let fragment = self
            .graph
            .remove_fragment(fragment_id)
            .ok_or_else(|| format!("fragment '{fragment_id}' not found"))?;
        Ok((
            AuthoringDelta::FragmentRemoved {
                fragment,
                before_stack,
            },
            OperationKind::FragmentRemoved,
            Some(format!("graph.fragments[{fragment_id}]")),
            Some(DiagnosticTarget::Graph),
        ))
    }

    pub(super) fn enter_fragment(&mut self, fragment_id: &str) -> CommandApplyResult {
        let before_stack = self.graph.graph_stack_for_command_bus();
        if !self.graph.enter_fragment(fragment_id) {
            return Err(format!("fragment '{fragment_id}' could not be entered"));
        }
        let after_stack = self.graph.graph_stack_for_command_bus();
        Ok((
            AuthoringDelta::FragmentEntered {
                fragment_id: fragment_id.to_string(),
                before_stack,
                after_stack,
            },
            OperationKind::FragmentEntered,
            Some("graph.active_fragment".to_string()),
            Some(DiagnosticTarget::Graph),
        ))
    }

    pub(super) fn leave_fragment(&mut self) -> CommandApplyResult {
        let before_stack = self.graph.graph_stack_for_command_bus();
        if !self.graph.leave_fragment() {
            return Err("no active fragment to leave".to_string());
        }
        let after_stack = self.graph.graph_stack_for_command_bus();
        Ok((
            AuthoringDelta::FragmentLeft {
                before_stack,
                after_stack,
            },
            OperationKind::FragmentLeft,
            Some("graph.active_fragment".to_string()),
            Some(DiagnosticTarget::Graph),
        ))
    }

    pub(super) fn refresh_fragment_ports(&mut self, fragment_id: &str) -> CommandApplyResult {
        let before = self
            .graph
            .get_fragment(fragment_id)
            .cloned()
            .ok_or_else(|| format!("fragment '{fragment_id}' not found"))?;
        if !self.graph.refresh_fragment_ports(fragment_id) {
            return Err(format!("fragment '{fragment_id}' ports did not change"));
        }
        let after = self
            .graph
            .get_fragment(fragment_id)
            .cloned()
            .ok_or_else(|| format!("fragment '{fragment_id}' missing after refresh"))?;
        Ok((
            AuthoringDelta::FragmentPortsRefreshed {
                fragment_id: fragment_id.to_string(),
                before,
                after,
            },
            OperationKind::FieldEdited,
            Some(format!("graph.fragments[{fragment_id}].ports")),
            Some(DiagnosticTarget::Graph),
        ))
    }
}
