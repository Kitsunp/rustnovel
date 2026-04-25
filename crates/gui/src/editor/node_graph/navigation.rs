use super::*;
use crate::editor::authoring_adapter::to_authoring_graph;

impl NodeGraph {
    /// Returns node ids that directly connect into `node_id`.
    pub fn incoming_nodes(&self, node_id: u32) -> Vec<u32> {
        self.connections
            .iter()
            .filter(|connection| connection.to == node_id)
            .map(|connection| connection.from)
            .collect()
    }

    /// Returns node ids directly reachable from `node_id`.
    pub fn outgoing_nodes(&self, node_id: u32) -> Vec<u32> {
        self.connections
            .iter()
            .filter(|connection| connection.from == node_id)
            .map(|connection| connection.to)
            .collect()
    }

    /// Maps an event_ip from compiled/raw script flow back to the source node id.
    pub fn node_for_event_ip(&self, event_ip: u32) -> Option<u32> {
        to_authoring_graph(self).node_for_event_ip(event_ip)
    }

    /// Returns the event_ip index for a node in script traversal order.
    pub fn event_ip_for_node(&self, node_id: u32) -> Option<u32> {
        to_authoring_graph(self).event_ip_for_node(node_id)
    }

    /// Returns nodes that reference a concrete asset path.
    pub fn nodes_referencing_asset(&self, asset_path: &str) -> Vec<u32> {
        let needle = asset_path.trim();
        if needle.is_empty() {
            return Vec::new();
        }

        self.nodes
            .iter()
            .filter_map(|(node_id, node, _)| {
                if node_references_asset(node, needle) {
                    Some(*node_id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns the first node that references the provided asset path.
    pub fn first_node_referencing_asset(&self, asset_path: &str) -> Option<u32> {
        self.nodes_referencing_asset(asset_path).into_iter().next()
    }
}

fn node_references_asset(node: &StoryNode, asset_path: &str) -> bool {
    match node {
        StoryNode::Scene {
            background,
            music,
            characters,
            ..
        } => {
            background.as_deref() == Some(asset_path)
                || music.as_deref() == Some(asset_path)
                || characters
                    .iter()
                    .any(|character| character.expression.as_deref() == Some(asset_path))
        }
        StoryNode::ScenePatch(patch) => {
            patch.background.as_deref() == Some(asset_path)
                || patch.music.as_deref() == Some(asset_path)
                || patch
                    .add
                    .iter()
                    .any(|character| character.expression.as_deref() == Some(asset_path))
                || patch
                    .update
                    .iter()
                    .any(|character| character.expression.as_deref() == Some(asset_path))
        }
        StoryNode::AudioAction { asset, .. } => asset.as_deref() == Some(asset_path),
        _ => false,
    }
}
