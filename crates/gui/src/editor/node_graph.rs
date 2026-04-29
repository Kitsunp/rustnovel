//! Node graph data structure for the visual editor.
//!
//! This module contains the core graph data structure that represents
//! the story flow. It handles node management and connections.
//! Script synchronization is in the `script_sync` module.

use eframe::egui;
use serde::{Deserialize, Serialize};
pub use visual_novel_engine::authoring::{GraphConnection, SceneProfile};
use visual_novel_engine::{
    authoring::{AuthoringPosition, NodeGraph as AuthoringGraph},
    ScriptRaw,
};

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

/// A node graph representing the story structure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeGraph {
    /// Headless semantic graph. GUI state below is view/interaction only.
    #[serde(flatten)]
    pub(crate) authoring: AuthoringGraph,
    /// Currently selected node
    #[serde(skip)]
    pub selected: Option<u32>,
    /// Pan offset (world-space translation)
    #[serde(default)]
    pub(crate) pan: egui::Vec2,
    /// Zoom level
    #[serde(default = "default_zoom")]
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
}

impl Default for NodeGraph {
    fn default() -> Self {
        Self {
            authoring: AuthoringGraph::new(),
            selected: None,
            pan: egui::Vec2::ZERO,
            zoom: ZOOM_DEFAULT,
            editing: None,
            dragging_node: None,
            connecting_from: None,
            context_menu: None,
        }
    }
}

impl NodeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a node at the specified position. Returns the node ID.
    pub fn add_node(&mut self, node: StoryNode, pos: egui::Pos2) -> u32 {
        self.authoring
            .add_node(node, AuthoringPosition::new(pos.x, pos.y))
    }

    /// Removes a node and all its connections.
    pub fn remove_node(&mut self, id: u32) {
        self.authoring.remove_node(id);

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
    }

    /// Returns the number of nodes.
    #[inline]
    pub fn len(&self) -> usize {
        self.authoring.len()
    }

    /// Returns true if the graph is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.authoring.is_empty()
    }

    /// Returns true if the graph has been modified since last save.
    #[inline]
    pub fn is_modified(&self) -> bool {
        self.authoring.is_modified()
    }

    /// Clears the modified flag.
    pub fn clear_modified(&mut self) {
        self.authoring.clear_modified();
    }

    /// Marks the graph as modified.
    pub fn mark_modified(&mut self) {
        self.authoring.mark_modified();
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
        for (id, node, pos) in self.nodes() {
            let node_rect =
                egui::Rect::from_min_size(pos, egui::vec2(NODE_WIDTH, node_visual_height(&node)));
            if node_rect.contains(graph_pos) {
                return Some(id);
            }
        }
        None
    }

    /// Gets a reference to a node by ID.
    pub fn get_node(&self, id: u32) -> Option<&StoryNode> {
        self.authoring.get_node(id)
    }

    /// Gets a mutable reference to a node by ID.
    pub fn get_node_mut(&mut self, id: u32) -> Option<&mut StoryNode> {
        self.authoring.get_node_mut(id)
    }

    pub fn get_node_pos(&self, id: u32) -> Option<egui::Pos2> {
        self.authoring
            .get_node_pos(id)
            .map(|pos| egui::pos2(pos.x, pos.y))
    }

    pub fn set_node_pos(&mut self, id: u32, pos: egui::Pos2) -> bool {
        self.authoring
            .set_node_pos(id, AuthoringPosition::new(pos.x, pos.y))
    }

    pub fn translate_node(&mut self, id: u32, delta: egui::Vec2) -> bool {
        let Some(pos) = self.get_node_pos(id) else {
            return false;
        };
        self.set_node_pos(id, pos + delta)
    }

    /// Returns an iterator over all nodes as GUI-positioned snapshots.
    pub fn nodes(&self) -> impl Iterator<Item = (u32, StoryNode, egui::Pos2)> + '_ {
        self.authoring
            .nodes()
            .map(|(id, node, pos)| (*id, node.clone(), egui::pos2(pos.x, pos.y)))
    }

    /// Returns an iterator over all connections as snapshots.
    pub fn connections(&self) -> impl Iterator<Item = GraphConnection> + '_ {
        self.authoring.connections().cloned()
    }

    pub(crate) fn from_authoring_graph(authoring: AuthoringGraph) -> Self {
        Self {
            authoring,
            ..Self::default()
        }
    }

    pub(crate) fn authoring_graph(&self) -> &AuthoringGraph {
        &self.authoring
    }

    pub(crate) fn replace_authoring_graph(&mut self, authoring: AuthoringGraph) {
        self.authoring = authoring;
    }
}

fn default_zoom() -> f32 {
    ZOOM_DEFAULT
}

#[cfg(test)]
#[path = "tests/node_graph_scene_profile_tests.rs"]
mod scene_profile_tests;

#[cfg(test)]
#[path = "tests/node_graph_tests.rs"]
mod tests;
