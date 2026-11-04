use serde::Serialize;

use super::{AuthoringPosition, FragmentPort, GraphConnection, NodeGraph, StoryNode};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NodeRemovalContext {
    node_index: usize,
    connection_indices: Vec<usize>,
    bookmarks: Vec<String>,
    fragments: Vec<FragmentRemovalContext>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct FragmentRemovalContext {
    fragment_id: String,
    node_indices: Vec<usize>,
    inputs: Vec<(usize, FragmentPort)>,
    outputs: Vec<(usize, FragmentPort)>,
}

impl NodeGraph {
    pub(crate) fn node_removal_context(&self, node_id: u32) -> Option<NodeRemovalContext> {
        let node_index = self.nodes.iter().position(|node| node.0 == node_id)?;
        let connection_indices = self
            .connections
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.from == node_id || edge.to == node_id)
            .map(|(index, _)| index)
            .collect();
        let bookmarks = self
            .bookmarks
            .iter()
            .filter(|(_, target)| **target == node_id)
            .map(|(name, _)| name.clone())
            .collect();
        let fragments = self
            .fragments
            .values()
            .filter_map(|fragment| {
                let node_indices = fragment
                    .node_ids
                    .iter()
                    .enumerate()
                    .filter(|(_, id)| **id == node_id)
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>();
                let inputs = removed_ports(&fragment.inputs, node_id);
                let outputs = removed_ports(&fragment.outputs, node_id);
                if node_indices.is_empty() && inputs.is_empty() && outputs.is_empty() {
                    return None;
                }
                Some(FragmentRemovalContext {
                    fragment_id: fragment.fragment_id.clone(),
                    node_indices,
                    inputs,
                    outputs,
                })
            })
            .collect();
        Some(NodeRemovalContext {
            node_index,
            connection_indices,
            bookmarks,
            fragments,
        })
    }

    pub(crate) fn restore_removed_node(
        &mut self,
        node_id: u32,
        node: &StoryNode,
        position: AuthoringPosition,
        connections: &[GraphConnection],
        context: &NodeRemovalContext,
    ) -> Result<(), String> {
        // Check every fallible precondition before changing graph data.
        if self.get_node(node_id).is_some() {
            return Err(format!("node {node_id} already exists"));
        }
        if context
            .fragments
            .iter()
            .any(|fragment| !self.fragments.contains_key(&fragment.fragment_id))
        {
            return Err(format!("node {node_id} fragment could not be restored"));
        }
        if context
            .bookmarks
            .iter()
            .any(|name| self.bookmarks.contains_key(name))
        {
            return Err(format!("node {node_id} bookmark already exists"));
        }
        self.nodes.insert(
            context.node_index.min(self.nodes.len()),
            std::sync::Arc::new((node_id, node.clone(), position)),
        );
        for (index, edge) in context.connection_indices.iter().zip(connections) {
            self.connections
                .insert((*index).min(self.connections.len()), edge.clone());
        }
        for name in &context.bookmarks {
            self.bookmarks.insert(name.clone(), node_id);
        }
        for saved in &context.fragments {
            if let Some(fragment) = self.fragments.get_mut(&saved.fragment_id) {
                for index in &saved.node_indices {
                    fragment
                        .node_ids
                        .insert((*index).min(fragment.node_ids.len()), node_id);
                }
                restore_ports(&mut fragment.inputs, &saved.inputs);
                restore_ports(&mut fragment.outputs, &saved.outputs);
            }
        }
        self.modified = true;
        Ok(())
    }
}

fn removed_ports(ports: &[FragmentPort], node_id: u32) -> Vec<(usize, FragmentPort)> {
    ports
        .iter()
        .enumerate()
        .filter(|(_, port)| port.node_id == Some(node_id))
        .map(|(index, port)| (index, port.clone()))
        .collect()
}

fn restore_ports(ports: &mut Vec<FragmentPort>, saved: &[(usize, FragmentPort)]) {
    for (index, port) in saved {
        ports.insert((*index).min(ports.len()), port.clone());
    }
}
