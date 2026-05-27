use super::super::{DiagnosticTarget, GraphConnection, NodeGraph, OperationKind, StoryNode};
use super::{AuthoringCommandBus, AuthoringDelta, CommandApplyResult};

impl AuthoringCommandBus {
    pub(super) fn connect(&mut self, from: u32, from_port: usize, to: u32) -> CommandApplyResult {
        let replaced = self
            .graph
            .connections()
            .filter(|conn| conn.from == from && conn.from_port == from_port)
            .cloned()
            .collect::<Vec<_>>();
        let already_connected = replaced
            .iter()
            .any(|conn| conn.from == from && conn.from_port == from_port && conn.to == to);
        self.graph.connect_port(from, from_port, to);
        let connection = GraphConnection {
            from,
            from_port,
            to,
        };
        if already_connected {
            return Err(format!(
                "node {from} port {from_port} is already connected to node {to}"
            ));
        }
        if !self.graph.connections().any(|conn| conn == &connection) {
            return Err(format!(
                "node {from} port {from_port} could not connect to node {to}"
            ));
        }
        Ok((
            AuthoringDelta::Connected {
                connection: connection.clone(),
                replaced,
            },
            OperationKind::NodeConnected,
            Some(format!("graph.connections[{from}:{from_port}]")),
            Some(DiagnosticTarget::Edge {
                from,
                from_port,
                to: Some(to),
            }),
        ))
    }

    pub(super) fn set_choice_option_target(
        &mut self,
        node_id: u32,
        option_index: usize,
        target_node_id: Option<u32>,
    ) -> CommandApplyResult {
        let Some(StoryNode::Choice { options, .. }) = self.graph.get_node(node_id) else {
            return Err(format!("node {node_id} is not a choice"));
        };
        if option_index >= options.len() {
            return Err(format!(
                "choice node {node_id} has no option {option_index}"
            ));
        }
        let before = self
            .graph
            .connections()
            .filter(|conn| conn.from == node_id && conn.from_port == option_index)
            .cloned()
            .collect::<Vec<_>>();
        match target_node_id {
            Some(target) if self.graph.get_node(target).is_some() => {
                self.graph.connect_port(node_id, option_index, target);
            }
            Some(target) => return Err(format!("target node {target} not found")),
            None => self.graph.disconnect_port(node_id, option_index),
        }
        let after = self
            .graph
            .connections()
            .find(|conn| conn.from == node_id && conn.from_port == option_index)
            .cloned();
        let unchanged = match (before.as_slice(), after.as_ref()) {
            ([], None) => true,
            ([old], Some(new)) => old == new,
            _ => false,
        };
        if unchanged {
            return Err(format!(
                "choice node {node_id} option {option_index} target is unchanged"
            ));
        }
        Ok((
            AuthoringDelta::ChoiceOptionTargetSet {
                node_id,
                option_index,
                before,
                after: after.clone(),
            },
            OperationKind::FieldEdited,
            Some(format!(
                "graph.nodes[{node_id}].choice.options[{option_index}].target"
            )),
            Some(DiagnosticTarget::Edge {
                from: node_id,
                from_port: option_index,
                to: target_node_id,
            }),
        ))
    }

    pub(super) fn reorder_choice_option(
        &mut self,
        node_id: u32,
        from_index: usize,
        to_index: usize,
    ) -> CommandApplyResult {
        let before_node = self
            .graph
            .get_node(node_id)
            .cloned()
            .ok_or_else(|| format!("node {node_id} not found"))?;
        let option_count = match self.graph.get_node_mut(node_id) {
            Some(StoryNode::Choice { options, .. }) => {
                if from_index >= options.len() || to_index >= options.len() {
                    return Err(format!(
                        "choice node {node_id} cannot reorder option {from_index} to {to_index}"
                    ));
                }
                if from_index == to_index {
                    return Err(format!("choice node {node_id} option order is unchanged"));
                }
                let option = options.remove(from_index);
                options.insert(to_index, option);
                options.len()
            }
            _ => return Err(format!("node {node_id} is not a choice")),
        };
        let before_connections = choice_connections(&self.graph, node_id, option_count);
        remap_choice_connections(&mut self.graph, node_id, option_count, from_index, to_index);
        let after_connections = choice_connections(&self.graph, node_id, option_count);
        Ok((
            AuthoringDelta::ChoiceOptionReordered {
                node_id,
                from_index,
                to_index,
                before_node,
                before_connections,
                after_connections,
            },
            OperationKind::FieldEdited,
            Some(format!("graph.nodes[{node_id}].choice.options")),
            Some(DiagnosticTarget::Node { node_id }),
        ))
    }

    pub(super) fn remove_choice_option(
        &mut self,
        node_id: u32,
        option_index: usize,
    ) -> CommandApplyResult {
        let before_node = self
            .graph
            .get_node(node_id)
            .cloned()
            .ok_or_else(|| format!("node {node_id} not found"))?;
        let option_count = match self.graph.get_node(node_id) {
            Some(StoryNode::Choice { options, .. }) if option_index < options.len() => {
                options.len()
            }
            Some(StoryNode::Choice { .. }) => {
                return Err(format!(
                    "choice node {node_id} has no option {option_index}"
                ));
            }
            _ => return Err(format!("node {node_id} is not a choice")),
        };
        let before_connections = choice_connections(&self.graph, node_id, option_count);
        self.graph.remove_choice_option(node_id, option_index);
        let after_connections = choice_connections(&self.graph, node_id, option_count - 1);
        Ok((
            AuthoringDelta::ChoiceOptionRemoved {
                node_id,
                option_index,
                before_node,
                before_connections,
                after_connections,
            },
            OperationKind::FieldEdited,
            Some(format!(
                "graph.nodes[{node_id}].choice.options[{option_index}]"
            )),
            Some(DiagnosticTarget::Node { node_id }),
        ))
    }
}

fn choice_connections(
    graph: &NodeGraph,
    node_id: u32,
    option_count: usize,
) -> Vec<GraphConnection> {
    graph
        .connections()
        .filter(|connection| connection.from == node_id && connection.from_port < option_count)
        .cloned()
        .collect()
}

fn remap_choice_connections(
    graph: &mut NodeGraph,
    node_id: u32,
    option_count: usize,
    from_index: usize,
    to_index: usize,
) {
    let connections = choice_connections(graph, node_id, option_count);
    for connection in &connections {
        graph.disconnect_port(node_id, connection.from_port);
    }
    for connection in connections {
        graph.connect_port(
            node_id,
            remapped_port(connection.from_port, from_index, to_index),
            connection.to,
        );
    }
}

fn remapped_port(old_port: usize, from_index: usize, to_index: usize) -> usize {
    match from_index.cmp(&to_index) {
        std::cmp::Ordering::Less if old_port == from_index => to_index,
        std::cmp::Ordering::Less if old_port > from_index && old_port <= to_index => old_port - 1,
        std::cmp::Ordering::Greater if old_port == from_index => to_index,
        std::cmp::Ordering::Greater if old_port >= to_index && old_port < from_index => {
            old_port + 1
        }
        _ => old_port,
    }
}
