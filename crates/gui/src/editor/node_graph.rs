//! Node graph data structure for the visual editor.
//!
//! This module contains the core graph data structure that represents
//! the story flow. It handles node management and connections.
//! Script synchronization is in the `script_sync` module.

use std::collections::BTreeMap;

use eframe::egui;
use serde::{Deserialize, Serialize};
use visual_novel_engine::{CharacterPlacementRaw, ScriptRaw};

use super::node_types::{
    node_visual_height, ContextMenu, StoryNode, NODE_HEIGHT, NODE_VERTICAL_SPACING, NODE_WIDTH,
    ZOOM_DEFAULT, ZOOM_MAX, ZOOM_MIN,
};
use super::script_sync;

mod connections;
mod layout;
mod mutations;
mod navigation;
mod search;
mod view;

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

/// A node graph representing the story structure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeGraph {
    /// Nodes: (id, node, position in graph space)
    pub(crate) nodes: Vec<(u32, StoryNode, egui::Pos2)>,
    /// Connections: structured connections with ports
    pub(crate) connections: Vec<GraphConnection>,
    /// Reusable scene presets/environments.
    pub(crate) scene_profiles: BTreeMap<String, SceneProfile>,
    /// Named anchors for fast navigation in large graphs.
    #[serde(default)]
    pub(crate) bookmarks: BTreeMap<String, u32>,
    /// Next available node ID
    next_id: u32,
    /// Currently selected node
    #[serde(skip)]
    pub selected: Option<u32>,
    /// Pan offset (world-space translation)
    pub(crate) pan: egui::Vec2,
    /// Zoom level
    pub(crate) zoom: f32,
    /// Node being edited inline
    #[serde(skip)]
    pub editing: Option<u32>,
    /// Node being dragged (robust interaction)
    #[serde(skip)]
    pub dragging_node: Option<u32>,
    /// Node being connected (Connect To mode)
    #[serde(skip)]
    pub connecting_from: Option<(u32, usize)>,
    /// Active context menu
    #[serde(skip)]
    pub context_menu: Option<ContextMenu>,
    /// Dirty flag (script modified since last save)
    #[serde(skip)]
    pub(crate) modified: bool,
}

impl Default for NodeGraph {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            connections: Vec::new(),
            scene_profiles: BTreeMap::new(),
            bookmarks: BTreeMap::new(),
            next_id: 0,
            selected: None,
            pan: egui::Vec2::ZERO,
            zoom: ZOOM_DEFAULT,
            editing: None,
            dragging_node: None,
            connecting_from: None,
            context_menu: None,
            modified: false,
        }
    }
}

impl NodeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a node at the specified position. Returns the node ID.
    pub fn add_node(&mut self, node: StoryNode, pos: egui::Pos2) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push((id, node, pos));
        self.modified = true;
        id
    }

    pub(crate) fn add_node_with_id(&mut self, id: u32, node: StoryNode, pos: egui::Pos2) -> bool {
        if self.nodes.iter().any(|(node_id, _, _)| *node_id == id) {
            return false;
        }
        self.next_id = self.next_id.max(id.saturating_add(1));
        self.nodes.push((id, node, pos));
        self.modified = true;
        true
    }

    /// Removes a node and all its connections.
    pub fn remove_node(&mut self, id: u32) {
        self.nodes.retain(|(nid, _, _)| *nid != id);
        self.connections.retain(|c| c.from != id && c.to != id);
        self.bookmarks.retain(|_, target| *target != id);

        if self.selected == Some(id) {
            self.selected = None;
        }
        if self.editing == Some(id) {
            self.editing = None;
        }
        if let Some((from_id, _)) = self.connecting_from {
            if from_id == id {
                self.connecting_from = None;
            }
        }

        self.modified = true;
    }

    /// Returns the number of nodes.
    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns true if the graph is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns true if the graph has been modified since last save.
    #[inline]
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Clears the modified flag.
    pub fn clear_modified(&mut self) {
        self.modified = false;
    }

    /// Marks the graph as modified.
    pub fn mark_modified(&mut self) {
        self.modified = true;
    }

    /// Creates a node graph from a raw script.
    pub fn from_script(script: &ScriptRaw) -> Self {
        script_sync::from_script(script)
    }

    /// Converts the node graph to a raw script.
    pub fn to_script(&self) -> ScriptRaw {
        script_sync::to_script(self)
    }

    /// Returns the node at the given graph position, if any.
    pub fn node_at_position(&self, graph_pos: egui::Pos2) -> Option<u32> {
        for (id, node, pos) in &self.nodes {
            let node_rect =
                egui::Rect::from_min_size(*pos, egui::vec2(NODE_WIDTH, node_visual_height(node)));
            if node_rect.contains(graph_pos) {
                return Some(*id);
            }
        }
        None
    }

    /// Gets a reference to a node by ID.
    pub fn get_node(&self, id: u32) -> Option<&StoryNode> {
        self.nodes
            .iter()
            .find(|(nid, _, _)| *nid == id)
            .map(|(_, node, _)| node)
    }

    /// Gets a mutable reference to a node by ID.
    pub fn get_node_mut(&mut self, id: u32) -> Option<&mut StoryNode> {
        self.nodes
            .iter_mut()
            .find(|(nid, _, _)| *nid == id)
            .map(|(_, node, _)| node)
    }

    /// Gets a mutable reference to a node position by ID.
    pub fn get_node_pos_mut(&mut self, id: u32) -> Option<&mut egui::Pos2> {
        self.nodes
            .iter_mut()
            .find(|(nid, _, _)| *nid == id)
            .map(|(_, _, pos)| pos)
    }

    /// Returns an iterator over all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &(u32, StoryNode, egui::Pos2)> {
        self.nodes.iter()
    }

    /// Returns an iterator over all connections.
    pub fn connections(&self) -> impl Iterator<Item = &GraphConnection> {
        self.connections.iter()
    }

    /// Returns a slice of all nodes (internal use).
    #[allow(dead_code)]
    pub(crate) fn nodes_slice(&self) -> &[(u32, StoryNode, egui::Pos2)] {
        &self.nodes
    }

    /// Returns a slice of all connections (internal use).
    #[allow(dead_code)]
    pub(crate) fn connections_slice(&self) -> &[GraphConnection] {
        &self.connections
    }
}

#[cfg(test)]
#[path = "tests/node_graph_scene_profile_tests.rs"]
mod scene_profile_tests;

#[cfg(test)]
#[path = "tests/node_graph_tests.rs"]
mod tests;
