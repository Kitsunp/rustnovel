use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::{AuthoringPosition, GraphConnection, NodeGraph, StoryNode};

type NodeEntry = std::sync::Arc<(u32, StoryNode, AuthoringPosition)>;

/// Sparse inverse of a mutation limited to nodes and connections.
/// Indices preserve ordering; unchanged graph data is not retained in history.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GraphMutationUndo {
    nodes: Vec<(usize, NodeEntry)>,
    added_nodes: Vec<u32>,
    connections: Vec<(usize, GraphConnection)>,
    added_connections: Vec<GraphConnection>,
    next_id: u32,
}

impl NodeGraph {
    pub(crate) fn prune_node_connections(
        &mut self,
        node_id: u32,
        replacement: &StoryNode,
    ) -> Vec<(usize, GraphConnection)> {
        let mut removed = Vec::new();
        let mut index = 0;
        self.connections.retain(|edge| {
            let compatible = (edge.from != node_id || replacement.has_output_port(edge.from_port))
                && (edge.to != node_id || replacement.can_connect_to());
            if !compatible {
                removed.push((index, edge.clone()));
            }
            index += 1;
            compatible
        });
        removed
    }

    pub(crate) fn restore_connections(&mut self, connections: &[(usize, GraphConnection)]) {
        for (index, edge) in connections {
            self.connections
                .insert((*index).min(self.connections.len()), edge.clone());
        }
    }

    /// The closure must only mutate nodes, connections and their allocation counter,
    /// preserving the relative order of unchanged entries.
    /// Errors and no-ops restore those fields without producing a history entry.
    pub(crate) fn record_node_edge_mutation(
        &mut self,
        mutate: impl FnOnce(&mut Self) -> Result<bool, String>,
    ) -> Result<Option<GraphMutationUndo>, String> {
        let nodes = self.nodes.clone();
        let connections = self.connections.clone();
        let next_id = self.next_id;
        let modified = self.modified;
        let result = mutate(self);
        if !matches!(result, Ok(true)) {
            self.nodes = nodes;
            self.connections = connections;
            self.next_id = next_id;
            self.modified = modified;
            return result.map(|_| None);
        }

        let current: HashMap<_, _> = self.nodes.iter().map(|node| (node.0, node)).collect();
        let before_ids: HashSet<_> = nodes.iter().map(|node| node.0).collect();
        let added_nodes = self
            .nodes
            .iter()
            .filter(|node| !before_ids.contains(&node.0))
            .map(|node| node.0)
            .collect::<Vec<_>>();
        let nodes = nodes
            .into_iter()
            .enumerate()
            .filter(|(_, node)| current.get(&node.0).is_none_or(|after| **after != *node))
            .collect::<Vec<_>>();
        let before_edges: HashSet<_> = connections.iter().map(edge_key).collect();
        let after_edges: HashSet<_> = self.connections.iter().map(edge_key).collect();
        let added_connections = self
            .connections
            .iter()
            .filter(|edge| !before_edges.contains(&edge_key(edge)))
            .cloned()
            .collect::<Vec<_>>();
        let connections = connections
            .into_iter()
            .enumerate()
            .filter(|(_, edge)| !after_edges.contains(&edge_key(edge)))
            .collect::<Vec<_>>();
        if nodes.is_empty()
            && added_nodes.is_empty()
            && connections.is_empty()
            && added_connections.is_empty()
            && self.next_id == next_id
        {
            self.modified = modified;
            return Ok(None);
        }
        Ok(Some(GraphMutationUndo {
            nodes,
            added_nodes,
            connections,
            added_connections,
            next_id,
        }))
    }

    pub(crate) fn restore_node_edge_mutation(&mut self, undo: &GraphMutationUndo) {
        let replaced: HashSet<_> = undo
            .nodes
            .iter()
            .map(|(_, node)| node.0)
            .chain(undo.added_nodes.iter().copied())
            .collect();
        self.nodes.retain(|node| !replaced.contains(&node.0));
        for (index, node) in &undo.nodes {
            self.nodes
                .insert((*index).min(self.nodes.len()), node.clone());
        }
        let added: HashSet<_> = undo.added_connections.iter().map(edge_key).collect();
        self.connections
            .retain(|edge| !added.contains(&edge_key(edge)));
        self.restore_connections(&undo.connections);
        self.next_id = undo.next_id;
        self.modified = true;
    }
}

fn edge_key(edge: &GraphConnection) -> (u32, usize, u32) {
    (edge.from, edge.from_port, edge.to)
}
