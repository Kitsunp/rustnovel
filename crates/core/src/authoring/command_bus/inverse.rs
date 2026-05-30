use super::super::composer;
use super::{AuthoringCommandBus, AuthoringDelta};

impl AuthoringCommandBus {
    pub(crate) fn apply_inverse_delta(&mut self, delta: &AuthoringDelta) -> Result<(), String> {
        match delta {
            AuthoringDelta::NodeCreated { node_id, .. } => {
                self.graph.remove_node(*node_id);
                Ok(())
            }
            AuthoringDelta::NodeRemoved {
                node_id,
                node,
                position,
                connections,
            } => {
                if !self
                    .graph
                    .add_node_with_id(*node_id, node.clone(), *position)
                {
                    return Err(format!("node {node_id} could not be restored"));
                }
                for connection in connections {
                    self.graph
                        .connect_port(connection.from, connection.from_port, connection.to);
                }
                Ok(())
            }
            AuthoringDelta::Connected {
                connection,
                replaced,
            } => {
                self.graph
                    .disconnect_port(connection.from, connection.from_port);
                for old in replaced {
                    self.graph.connect_port(old.from, old.from_port, old.to);
                }
                Ok(())
            }
            AuthoringDelta::Disconnected { removed } => {
                for connection in removed {
                    self.graph
                        .connect_port(connection.from, connection.from_port, connection.to);
                }
                Ok(())
            }
            AuthoringDelta::ChoiceOptionConnected {
                choice_id,
                option_index,
                before_node,
                connection,
            } => {
                self.graph
                    .disconnect_port(connection.from, connection.from_port);
                let node = self
                    .graph
                    .get_node_mut(*choice_id)
                    .ok_or_else(|| format!("choice node {choice_id} not found"))?;
                *node = before_node.clone();
                self.graph.disconnect_port(*choice_id, *option_index);
                Ok(())
            }
            AuthoringDelta::ChoiceOptionTargetSet {
                node_id,
                option_index,
                before,
                ..
            } => {
                self.graph.disconnect_port(*node_id, *option_index);
                for connection in before {
                    self.graph
                        .connect_port(connection.from, connection.from_port, connection.to);
                }
                Ok(())
            }
            AuthoringDelta::BranchConnected { from, from_port, .. } => Err(format!(
                "branch connection from node {from} port {from_port} cannot be reverted as a single graph delta"
            )),
            AuthoringDelta::NodeEdited {
                node_id, before, ..
            } => {
                let node = self
                    .graph
                    .get_node_mut(*node_id)
                    .ok_or_else(|| format!("node {node_id} not found"))?;
                *node = before.clone();
                Ok(())
            }
            AuthoringDelta::FragmentCreated { fragment } => self
                .graph
                .remove_fragment(&fragment.fragment_id)
                .map(|_| ())
                .ok_or_else(|| format!("fragment '{}' could not be removed", fragment.fragment_id)),
            AuthoringDelta::FragmentRemoved {
                fragment,
                before_stack,
            } => {
                if !self
                    .graph
                    .insert_fragment_for_command_bus(fragment.clone())
                {
                    return Err(format!("fragment '{}' could not be restored", fragment.fragment_id));
                }
                self.graph
                    .replace_graph_stack_for_command_bus(before_stack.clone());
                Ok(())
            }
            AuthoringDelta::FragmentEntered { before_stack, .. }
            | AuthoringDelta::FragmentLeft { before_stack, .. } => {
                self.graph
                    .replace_graph_stack_for_command_bus(before_stack.clone());
                Ok(())
            }
            AuthoringDelta::FragmentPortsRefreshed {
                fragment_id,
                before,
                ..
            } => self
                .graph
                .replace_fragment_for_command_bus(before.clone())
                .then_some(())
                .ok_or_else(|| format!("fragment '{fragment_id}' could not be restored")),
            AuthoringDelta::QuickFixApplied { fix_id, .. } => Err(format!(
                "quick-fix '{fix_id}' cannot be reverted as a single graph delta"
            )),
            AuthoringDelta::ChoiceOptionReordered {
                node_id,
                before_node,
                before_connections,
                after_connections,
                ..
            }
            | AuthoringDelta::ChoiceOptionRemoved {
                node_id,
                before_node,
                before_connections,
                after_connections,
                ..
            } => {
                for connection in after_connections {
                    self.graph
                        .disconnect_port(connection.from, connection.from_port);
                }
                let node = self
                    .graph
                    .get_node_mut(*node_id)
                    .ok_or_else(|| format!("choice node {node_id} not found"))?;
                *node = before_node.clone();
                for connection in before_connections {
                    self.graph
                        .connect_port(connection.from, connection.from_port, connection.to);
                }
                Ok(())
            }
            AuthoringDelta::AssetImported { .. } => Ok(()),
            AuthoringDelta::LayerMoved {
                object_id, before, ..
            } => {
                let (x, y, scale) = *before;
                composer::set_scene_object_pose(&mut self.graph, object_id, x, y, scale)
                    .then_some(())
                    .ok_or_else(|| format!("layer object '{object_id}' could not be reverted"))
            }
            AuthoringDelta::Reverted { reverted } => self.apply_inverse_delta(reverted),
        }
    }
}
