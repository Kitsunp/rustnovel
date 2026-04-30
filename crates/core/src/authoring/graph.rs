use std::collections::BTreeMap;
use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{analyze_flow_graph, FlowGraphAnalysis};
use crate::{CharacterPlacementRaw, ScriptRaw, VnResult};

use super::script_sync;
use super::{AuthoringPosition, StoryNode};

mod fragments;
mod search;
pub use fragments::{DecisionHub, FragmentPort, GraphFragment, GraphStack, PortalNode};
use search::searchable_text;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphConnection {
    pub from: u32,
    pub from_port: usize,
    pub to: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneLayer {
    pub name: String,
    pub visible: bool,
    pub background: Option<String>,
    pub characters: Vec<CharacterPlacementRaw>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacterPoseBinding {
    pub character: String,
    pub pose: String,
    pub image: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneProfile {
    pub background: Option<String>,
    pub music: Option<String>,
    pub characters: Vec<CharacterPlacementRaw>,
    #[serde(default)]
    pub layers: Vec<SceneLayer>,
    #[serde(default)]
    pub poses: Vec<CharacterPoseBinding>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NodeGraph {
    nodes: Vec<(u32, StoryNode, AuthoringPosition)>,
    connections: Vec<GraphConnection>,
    pub(super) scene_profiles: BTreeMap<String, SceneProfile>,
    #[serde(default)]
    bookmarks: BTreeMap<String, u32>,
    #[serde(default)]
    fragments: BTreeMap<String, GraphFragment>,
    #[serde(default)]
    graph_stack: GraphStack,
    next_id: u32,
    #[serde(skip)]
    pub(super) modified: bool,
}

impl NodeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_script(script: &ScriptRaw) -> Self {
        script_sync::from_script(script)
    }

    pub fn to_script(&self) -> ScriptRaw {
        script_sync::to_script(self)
    }

    pub fn to_script_strict(&self) -> VnResult<ScriptRaw> {
        script_sync::to_script_strict(self)
    }

    pub fn to_script_lossy_for_diagnostics(&self) -> ScriptRaw {
        script_sync::to_script_lossy_for_diagnostics(self)
    }

    pub fn add_node(&mut self, node: StoryNode, pos: AuthoringPosition) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push((id, node, pos));
        self.modified = true;
        id
    }

    pub fn add_node_with_id(&mut self, id: u32, node: StoryNode, pos: AuthoringPosition) -> bool {
        if self.nodes.iter().any(|(node_id, _, _)| *node_id == id) {
            return false;
        }
        self.next_id = self.next_id.max(id.saturating_add(1));
        self.nodes.push((id, node, pos));
        self.modified = true;
        true
    }

    pub fn remove_node(&mut self, id: u32) {
        self.nodes.retain(|(nid, _, _)| *nid != id);
        self.connections
            .retain(|conn| conn.from != id && conn.to != id);
        self.bookmarks.retain(|_, target| *target != id);
        self.modified = true;
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn clear_modified(&mut self) {
        self.modified = false;
    }

    pub fn mark_modified(&mut self) {
        self.modified = true;
    }

    pub fn get_node(&self, id: u32) -> Option<&StoryNode> {
        self.nodes
            .iter()
            .find(|(node_id, _, _)| *node_id == id)
            .map(|(_, node, _)| node)
    }

    pub fn get_node_mut(&mut self, id: u32) -> Option<&mut StoryNode> {
        self.nodes
            .iter_mut()
            .find(|(node_id, _, _)| *node_id == id)
            .map(|(_, node, _)| node)
    }

    pub fn get_node_pos(&self, id: u32) -> Option<AuthoringPosition> {
        self.nodes
            .iter()
            .find(|(node_id, _, _)| *node_id == id)
            .map(|(_, _, pos)| *pos)
    }

    pub fn set_node_pos(&mut self, id: u32, pos: AuthoringPosition) -> bool {
        let Some((_, _, current)) = self.nodes.iter_mut().find(|(node_id, _, _)| *node_id == id)
        else {
            return false;
        };
        if *current == pos {
            return false;
        }
        *current = pos;
        self.modified = true;
        true
    }

    pub fn nodes(&self) -> impl Iterator<Item = &(u32, StoryNode, AuthoringPosition)> {
        self.nodes.iter()
    }

    pub fn connections(&self) -> impl Iterator<Item = &GraphConnection> {
        self.connections.iter()
    }

    pub fn flow_analysis(&self, start_nodes: &[u32]) -> FlowGraphAnalysis {
        let nodes = self.nodes.iter().map(|(id, _, _)| *id).collect::<Vec<_>>();
        let edges = self
            .connections
            .iter()
            .map(|connection| (connection.from, connection.to))
            .collect::<Vec<_>>();
        analyze_flow_graph(&nodes, &edges, start_nodes)
    }

    pub fn incoming_nodes(&self, node_id: u32) -> Vec<u32> {
        self.connections
            .iter()
            .filter(|connection| connection.to == node_id)
            .map(|connection| connection.from)
            .collect()
    }

    pub fn outgoing_nodes(&self, node_id: u32) -> Vec<u32> {
        self.connections
            .iter()
            .filter(|connection| connection.from == node_id)
            .map(|connection| connection.to)
            .collect()
    }

    pub fn node_for_event_ip(&self, event_ip: u32) -> Option<u32> {
        let idx = usize::try_from(event_ip).ok()?;
        self.script_order_node_ids().get(idx).copied()
    }

    pub fn event_ip_for_node(&self, node_id: u32) -> Option<u32> {
        let idx = self
            .script_order_node_ids()
            .iter()
            .position(|candidate| *candidate == node_id)?;
        u32::try_from(idx).ok()
    }

    pub fn connect(&mut self, from: u32, to: u32) {
        self.connect_port(from, 0, to)
    }

    pub fn connect_port(&mut self, from: u32, from_port: usize, to: u32) {
        let Some(from_node) = self.get_node(from).cloned() else {
            return;
        };
        let Some(to_node) = self.get_node(to) else {
            return;
        };
        if !from_node.can_connect_from() || !to_node.can_connect_to() {
            return;
        }
        if matches!(from_node, StoryNode::Choice { .. }) {
            self.ensure_choice_option(from, from_port);
        } else if matches!(from_node, StoryNode::JumpIf { .. }) {
            if from_port > 1 {
                return;
            }
        } else if from_port != 0 {
            return;
        }
        if self
            .connections
            .iter()
            .any(|conn| conn.from == from && conn.from_port == from_port && conn.to == to)
        {
            return;
        }
        self.connections
            .retain(|conn| !(conn.from == from && conn.from_port == from_port));
        self.connections.push(GraphConnection {
            from,
            from_port,
            to,
        });
        self.modified = true;
    }

    pub fn disconnect_port(&mut self, from: u32, from_port: usize) {
        let before = self.connections.len();
        self.connections
            .retain(|conn| !(conn.from == from && conn.from_port == from_port));
        self.modified |= self.connections.len() != before;
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn remove_choice_option(&mut self, node_id: u32, option_idx: usize) {
        if let Some(StoryNode::Choice { options, .. }) = self.get_node_mut(node_id) {
            if option_idx < options.len() {
                options.remove(option_idx);
            }
        }
        self.connections
            .retain(|conn| !(conn.from == node_id && conn.from_port == option_idx));
        for conn in &mut self.connections {
            if conn.from == node_id && conn.from_port > option_idx {
                conn.from_port -= 1;
            }
        }
        self.modified = true;
    }

    pub fn search_nodes(&self, query: &str) -> Vec<u32> {
        let needle = query.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }
        self.nodes
            .iter()
            .filter_map(|(id, node, _)| searchable_text(node).contains(&needle).then_some(*id))
            .collect()
    }

    pub fn script_order_node_ids(&self) -> Vec<u32> {
        let start_id = self
            .nodes
            .iter()
            .find(|(_, node, _)| matches!(node, StoryNode::Start))
            .map(|(id, _, _)| *id);

        let mut visited = Vec::new();
        let mut visited_set = HashSet::new();
        let mut queue = VecDeque::new();
        let mut queued = HashSet::new();
        if let Some(start) = start_id {
            queue.push_back(start);
            queued.insert(start);
        }

        while let Some(id) = queue.pop_front() {
            if !visited_set.insert(id) {
                continue;
            }
            visited.push(id);

            let mut outgoing: Vec<_> = self
                .connections
                .iter()
                .filter(|connection| connection.from == id)
                .collect();
            let from_node = self.get_node(id);
            outgoing.sort_by_key(|connection| {
                (
                    script_order_port_key(from_node, connection.from_port),
                    connection.to,
                )
            });
            for connection in outgoing {
                if !visited_set.contains(&connection.to) && queued.insert(connection.to) {
                    queue.push_back(connection.to);
                }
            }
        }

        let mut ordered = visited
            .into_iter()
            .filter(|node_id| {
                self.get_node(*node_id)
                    .is_some_and(|node| !node.is_marker())
            })
            .collect::<Vec<_>>();

        for (id, node, _) in &self.nodes {
            if !node.is_marker() && !ordered.contains(id) {
                ordered.push(*id);
            }
        }
        ordered
    }

    pub fn set_bookmark(&mut self, name: impl Into<String>, node_id: u32) -> bool {
        if self.get_node(node_id).is_none() {
            return false;
        }
        let name = name.into().trim().to_string();
        if name.is_empty() {
            return false;
        }
        self.bookmarks.insert(name, node_id);
        self.modified = true;
        true
    }

    pub fn remove_bookmark(&mut self, name: &str) -> bool {
        if self.bookmarks.remove(name).is_some() {
            self.modified = true;
            true
        } else {
            false
        }
    }

    pub fn bookmarked_node(&self, name: &str) -> Option<u32> {
        self.bookmarks.get(name).copied()
    }

    pub fn bookmarks(&self) -> impl Iterator<Item = (&String, &u32)> {
        self.bookmarks.iter()
    }

    pub fn create_fragment(
        &mut self,
        fragment_id: impl Into<String>,
        title: impl Into<String>,
        mut node_ids: Vec<u32>,
    ) -> bool {
        let fragment_id = fragment_id.into().trim().to_string();
        if fragment_id.is_empty() || self.fragments.contains_key(&fragment_id) {
            return false;
        }
        node_ids.retain(|node_id| self.get_node(*node_id).is_some());
        node_ids.sort_unstable();
        node_ids.dedup();
        let node_set = node_ids.iter().copied().collect::<HashSet<_>>();
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for connection in &self.connections {
            let from_inside = node_set.contains(&connection.from);
            let to_inside = node_set.contains(&connection.to);
            match (from_inside, to_inside) {
                (false, true) => inputs.push(FragmentPort {
                    port_id: format!("in_{}_{}", connection.to, connection.from_port),
                    label: format!("from {}:{}", connection.from, connection.from_port),
                    node_id: Some(connection.to),
                }),
                (true, false) => outputs.push(FragmentPort {
                    port_id: format!("out_{}_{}", connection.from, connection.from_port),
                    label: format!("to {}", connection.to),
                    node_id: Some(connection.from),
                }),
                _ => {}
            }
        }
        inputs.sort_by(|a, b| a.port_id.cmp(&b.port_id));
        inputs.dedup_by(|a, b| a.port_id == b.port_id);
        outputs.sort_by(|a, b| a.port_id.cmp(&b.port_id));
        outputs.dedup_by(|a, b| a.port_id == b.port_id);
        self.fragments.insert(
            fragment_id.clone(),
            GraphFragment {
                fragment_id,
                title: title.into(),
                node_ids,
                inputs,
                outputs,
            },
        );
        self.modified = true;
        true
    }

    pub fn remove_fragment(&mut self, fragment_id: &str) -> Option<GraphFragment> {
        let removed = self.fragments.remove(fragment_id);
        if removed.is_some() {
            if self.graph_stack.active_fragment.as_deref() == Some(fragment_id) {
                self.graph_stack.active_fragment = None;
            }
            self.graph_stack
                .breadcrumb
                .retain(|candidate| candidate != fragment_id);
            self.modified = true;
        }
        removed
    }

    pub fn fragment(&self, fragment_id: &str) -> Option<&GraphFragment> {
        self.fragments.get(fragment_id)
    }

    pub fn fragments(&self) -> impl Iterator<Item = (&String, &GraphFragment)> {
        self.fragments.iter()
    }

    pub fn graph_stack(&self) -> &GraphStack {
        &self.graph_stack
    }

    pub fn enter_fragment(&mut self, fragment_id: &str) -> bool {
        if !self.fragments.contains_key(fragment_id) {
            return false;
        }
        if let Some(active) = self.graph_stack.active_fragment.take() {
            self.graph_stack.breadcrumb.push(active);
        }
        self.graph_stack.active_fragment = Some(fragment_id.to_string());
        self.modified = true;
        true
    }

    pub fn leave_fragment(&mut self) -> bool {
        let Some(previous) = self.graph_stack.breadcrumb.pop() else {
            if self.graph_stack.active_fragment.take().is_some() {
                self.modified = true;
                return true;
            }
            return false;
        };
        self.graph_stack.active_fragment = Some(previous);
        self.modified = true;
        true
    }

    fn ensure_choice_option(&mut self, node_id: u32, option_idx: usize) {
        let Some(StoryNode::Choice { options, .. }) = self.get_node_mut(node_id) else {
            return;
        };
        let mut changed = false;
        while options.len() <= option_idx {
            let next = options.len() + 1;
            options.push(format!("Option {next}"));
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }
}

fn script_order_port_key(node: Option<&StoryNode>, port: usize) -> usize {
    if matches!(node, Some(StoryNode::JumpIf { .. })) {
        match port {
            1 => 0,
            0 => 1,
            other => other.saturating_add(1),
        }
    } else {
        port
    }
}
