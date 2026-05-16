use super::*;

impl NodeGraph {
    /// Returns node ids that directly connect into `node_id`.
    pub fn incoming_nodes(&self, node_id: u32) -> Vec<u32> {
        self.authoring.incoming_nodes(node_id)
    }

    /// Returns node ids directly reachable from `node_id`.
    pub fn outgoing_nodes(&self, node_id: u32) -> Vec<u32> {
        self.authoring.outgoing_nodes(node_id)
    }

    /// Maps an event_ip from compiled/raw script flow back to the source node id.
    pub fn node_for_event_ip(&self, event_ip: u32) -> Option<u32> {
        self.authoring.node_for_event_ip(event_ip)
    }

    /// Returns the event_ip index for a node in script traversal order.
    pub fn event_ip_for_node(&self, node_id: u32) -> Option<u32> {
        self.authoring.event_ip_for_node(node_id)
    }

    /// Returns nodes that reference a concrete asset path.
    pub fn nodes_referencing_asset(&self, asset_path: &str) -> Vec<u32> {
        let needle = asset_path.trim();
        if needle.is_empty() {
            return Vec::new();
        }

        self.nodes()
            .filter_map(|(node_id, node, _)| {
                if node_references_asset(&node, needle) {
                    Some(node_id)
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

    /// Resolves the most useful graph node to focus for a diagnostic issue.
    pub fn focus_node_for_issue(
        &self,
        issue: &visual_novel_engine::authoring::LintIssue,
    ) -> Option<u32> {
        issue
            .node_id
            .or_else(|| issue.target.as_ref().and_then(target_focus_node))
            .or(issue.edge_from)
            .or_else(|| {
                issue
                    .event_ip
                    .and_then(|event_ip| self.node_for_event_ip(event_ip))
            })
            .or_else(|| {
                issue
                    .field_path
                    .as_ref()
                    .and_then(|field_path| node_id_from_field_path(field_path.value.as_str()))
            })
            .or_else(|| {
                issue
                    .asset_path
                    .as_ref()
                    .and_then(|asset| self.first_node_referencing_asset(asset))
            })
    }
}

fn target_focus_node(target: &visual_novel_engine::authoring::DiagnosticTarget) -> Option<u32> {
    use visual_novel_engine::authoring::DiagnosticTarget;
    match target {
        DiagnosticTarget::Node { node_id }
        | DiagnosticTarget::ChoiceOption { node_id, .. }
        | DiagnosticTarget::JumpTarget { node_id, .. } => Some(*node_id),
        DiagnosticTarget::Edge { from, .. } => Some(*from),
        DiagnosticTarget::AssetRef { node_id, .. }
        | DiagnosticTarget::Character { node_id, .. }
        | DiagnosticTarget::AudioChannel { node_id, .. }
        | DiagnosticTarget::Transition { node_id, .. } => *node_id,
        DiagnosticTarget::RuntimeEvent { .. }
        | DiagnosticTarget::Graph
        | DiagnosticTarget::SceneProfile { .. }
        | DiagnosticTarget::SceneLayer { .. }
        | DiagnosticTarget::Fragment { .. } => None,
        DiagnosticTarget::Generic { field_path } => field_path
            .as_ref()
            .and_then(|field_path| node_id_from_field_path(field_path.value.as_str())),
    }
}

fn node_id_from_field_path(value: &str) -> Option<u32> {
    let rest = value.strip_prefix("graph.nodes[")?;
    let (id, _) = rest.split_once(']')?;
    id.parse().ok()
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
