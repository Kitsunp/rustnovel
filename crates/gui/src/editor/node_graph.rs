//! Node graph data structure for the visual editor.
//!
//! This module contains the core graph data structure that represents
//! the story flow. It handles node management and connections.
//! Script synchronization is in the `script_sync` module.

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub use visual_novel_engine::authoring::{GraphConnection, SceneProfile};
use visual_novel_engine::{
    authoring::{
        AuthoringCommand, AuthoringDelta, AuthoringDocument, AuthoringDocumentCommand,
        AuthoringDocumentDelta, AuthoringDocumentSession, AuthoringPosition,
        NodeGraph as AuthoringGraph,
    },
    runtime::ScriptRaw,
};

use super::node_types::{
    node_visual_height, ContextMenu, StoryNode, NODE_HEIGHT, NODE_VERTICAL_SPACING, NODE_WIDTH,
    ZOOM_DEFAULT, ZOOM_MAX, ZOOM_MIN,
};
use super::script_sync;

mod connections;
mod fragments;
mod layout;
mod mutations;
mod navigation;
mod search;
mod view;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphOperationHint {
    pub kind: String,
    pub details: String,
    pub field_path: Option<String>,
    pub before_value: Option<String>,
    pub after_value: Option<String>,
    pub push_undo_snapshot: bool,
}

/// A node graph representing the story structure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeGraph {
    /// Headless semantic graph. GUI state below is view/interaction only.
    #[serde(flatten)]
    pub authoring: AuthoringGraph,
    /// Currently selected node
    #[serde(skip)]
    pub selected: Option<u32>,
    /// Multi-selection used for grouping fragments and batch graph operations.
    #[serde(skip)]
    pub selected_nodes: BTreeSet<u32>,
    /// Pan offset (world-space translation)
    #[serde(default)]
    pub pan: egui::Vec2,
    /// Zoom level
    #[serde(default = "default_zoom")]
    pub zoom: f32,
    /// Node being edited inline
    #[serde(skip)]
    pub editing: Option<u32>,
    /// Node being dragged (robust interaction)
    #[serde(skip)]
    pub dragging_node: Option<u32>,
    /// Node being connected (Connect To mode)
    #[serde(skip)]
    pub connecting_from: Option<(u32, usize)>,
    /// True when connection mode was started from context menu and should wait for a click.
    #[serde(skip)]
    pub connecting_sticky: bool,
    /// Graph-space marquee selection start.
    #[serde(skip)]
    pub marquee_start: Option<egui::Pos2>,
    /// Graph-space marquee selection current endpoint.
    #[serde(skip)]
    pub marquee_current: Option<egui::Pos2>,
    /// Active context menu
    #[serde(skip)]
    pub context_menu: Option<ContextMenu>,
    /// Last semantic editor operation inferred at graph level.
    #[serde(skip)]
    pub operation_hint: Option<GraphOperationHint>,
    /// Persistent core session used by graph-local editor commands.
    #[serde(skip, default = "default_authoring_session")]
    authoring_session: AuthoringDocumentSession,
}

impl Default for NodeGraph {
    fn default() -> Self {
        let authoring = AuthoringGraph::new();
        let authoring_session = authoring_session_for_graph(&authoring);
        Self {
            authoring,
            selected: None,
            selected_nodes: BTreeSet::new(),
            pan: egui::Vec2::ZERO,
            zoom: ZOOM_DEFAULT,
            editing: None,
            dragging_node: None,
            connecting_from: None,
            connecting_sticky: false,
            marquee_start: None,
            marquee_current: None,
            context_menu: None,
            operation_hint: None,
            authoring_session,
        }
    }
}

impl NodeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a node at the specified position. Returns the node ID.
    pub fn add_node(&mut self, node: StoryNode, pos: egui::Pos2) -> u32 {
        let id = self.authoring.next_node_id();
        if self
            .apply_authoring_command(AuthoringCommand::CreateNode {
                node_id: id,
                node,
                position: AuthoringPosition::new(pos.x, pos.y),
            })
            .is_none()
        {
            let details = format!("Failed to create node {id} through AuthoringDocumentSession");
            eprintln!("{details}");
            self.queue_operation_hint(
                "node_create_failed",
                details,
                Some(format!("graph.nodes[{id}]")),
                false,
            );
            return id;
        }
        self.queue_operation_hint(
            "node_created",
            format!("Created node {id}"),
            Some(format!("graph.nodes[{id}]")),
            true,
        );
        id
    }

    /// Removes a node and all its connections.
    pub fn remove_node(&mut self, id: u32) {
        let existed = self.get_node(id).is_some();
        if existed
            && self
                .apply_authoring_command(AuthoringCommand::RemoveNode { node_id: id })
                .is_none()
        {
            return;
        }

        if self.selected == Some(id) {
            self.selected = None;
        }
        self.selected_nodes.remove(&id);
        if self.editing == Some(id) {
            self.editing = None;
        }
        if self.dragging_node == Some(id) {
            self.dragging_node = None;
        }
        if let Some((from_id, _)) = self.connecting_from {
            if from_id == id {
                self.connecting_from = None;
                self.connecting_sticky = false;
            }
        }
        if self
            .context_menu
            .as_ref()
            .is_some_and(|menu| menu.node_id == Some(id))
        {
            self.context_menu = None;
        }
        if existed {
            self.queue_operation_hint(
                "node_removed",
                format!("Removed node {id}"),
                Some(format!("graph.nodes[{id}]")),
                true,
            );
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

    pub fn replace_node_with_hint(
        &mut self,
        id: u32,
        replacement: StoryNode,
        details: impl Into<String>,
        field_path: impl Into<String>,
        push_undo_snapshot: bool,
    ) -> bool {
        let Some(before) = self.get_node(id).cloned() else {
            return false;
        };
        if before == replacement {
            return false;
        }
        let before_value = serde_json::to_string(&before).ok();
        let after_value = serde_json::to_string(&replacement).ok();
        let changed = self
            .apply_authoring_command(AuthoringCommand::EditNode {
                node_id: id,
                replacement,
            })
            .is_some();
        if changed {
            self.queue_operation_hint_with_values(
                "field_edited",
                details,
                Some(field_path.into()),
                before_value,
                after_value,
                push_undo_snapshot,
            );
            self.mark_modified();
        }
        changed
    }

    pub fn get_node_pos(&self, id: u32) -> Option<egui::Pos2> {
        self.authoring
            .get_node_pos(id)
            .map(|pos| egui::pos2(pos.x, pos.y))
    }

    pub fn set_node_pos(&mut self, id: u32, pos: egui::Pos2) -> bool {
        self.set_node_pos_with_undo_hint(id, pos, true)
    }

    fn set_node_pos_with_undo_hint(
        &mut self,
        id: u32,
        pos: egui::Pos2,
        push_undo_snapshot: bool,
    ) -> bool {
        let changed = self
            .authoring
            .set_node_pos(id, AuthoringPosition::new(pos.x, pos.y));
        if changed {
            self.queue_operation_hint(
                "node_moved",
                format!("Moved node {id}"),
                Some(format!("graph.nodes[{id}].layout.position")),
                push_undo_snapshot,
            );
        }
        changed
    }

    pub fn translate_node(&mut self, id: u32, delta: egui::Vec2) -> bool {
        let Some(pos) = self.get_node_pos(id) else {
            return false;
        };
        self.set_node_pos(id, pos + delta)
    }

    pub fn translate_node_for_drag(&mut self, id: u32, delta: egui::Vec2) -> bool {
        let Some(pos) = self.get_node_pos(id) else {
            return false;
        };
        self.set_node_pos_with_undo_hint(id, pos + delta, false)
    }

    pub fn translate_selected_or_node(&mut self, anchor_id: u32, delta: egui::Vec2) -> usize {
        self.translate_selected_or_node_impl(anchor_id, delta, false)
    }

    pub fn translate_selected_or_node_for_drag(
        &mut self,
        anchor_id: u32,
        delta: egui::Vec2,
    ) -> usize {
        self.translate_selected_or_node_impl(anchor_id, delta, true)
    }

    fn translate_selected_or_node_impl(
        &mut self,
        anchor_id: u32,
        delta: egui::Vec2,
        drag_preview: bool,
    ) -> usize {
        if delta.length_sq() <= f32::EPSILON {
            return 0;
        }
        let ids = if self.selected_nodes.contains(&anchor_id) && self.selected_nodes.len() > 1 {
            self.selected_nodes.iter().copied().collect::<Vec<_>>()
        } else {
            vec![anchor_id]
        };
        ids.into_iter()
            .filter(|node_id| {
                if drag_preview {
                    self.translate_node_for_drag(*node_id, delta)
                } else {
                    self.translate_node(*node_id, delta)
                }
            })
            .count()
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

    pub fn from_authoring_graph(authoring: AuthoringGraph) -> Self {
        let authoring_session = authoring_session_for_graph(&authoring);
        Self {
            authoring,
            authoring_session,
            ..Self::default()
        }
    }

    pub fn authoring_graph(&self) -> &AuthoringGraph {
        &self.authoring
    }

    pub fn replace_authoring_graph(&mut self, authoring: AuthoringGraph) {
        self.authoring_session = authoring_session_for_graph(&authoring);
        self.authoring = authoring;
    }

    fn apply_authoring_command(&mut self, command: AuthoringCommand) -> Option<AuthoringDelta> {
        if self.authoring_session.document().graph != self.authoring {
            self.authoring_session = authoring_session_for_graph(&self.authoring);
        }
        let outcome = self
            .authoring_session
            .apply(AuthoringDocumentCommand::Graph(command))
            .ok()?;
        self.authoring = self.authoring_session.document().graph.clone();
        let AuthoringDocumentDelta::Graph(delta) = outcome.delta else {
            return None;
        };
        Some(*delta)
    }

    pub fn toggle_multi_selection(&mut self, node_id: u32) {
        if !self.selected_nodes.insert(node_id) {
            self.selected_nodes.remove(&node_id);
            if self.selected == Some(node_id) {
                self.selected = self.selected_nodes.iter().next_back().copied();
            }
            return;
        }
        self.selected = Some(node_id);
    }

    pub fn set_single_selection(&mut self, node_id: Option<u32>) {
        self.selected = node_id;
        self.selected_nodes.clear();
        if let Some(node_id) = node_id {
            self.selected_nodes.insert(node_id);
        }
    }

    pub fn select_nodes_in_rect(&mut self, rect: egui::Rect, additive: bool) -> usize {
        let selected = self
            .visible_nodes()
            .filter_map(|(id, node, pos)| {
                let node_rect = egui::Rect::from_min_size(
                    pos,
                    egui::vec2(NODE_WIDTH, node_visual_height(&node)),
                );
                rect.intersects(node_rect).then_some(id)
            })
            .collect::<Vec<_>>();
        if !additive {
            self.selected_nodes.clear();
        }
        for node_id in &selected {
            self.selected_nodes.insert(*node_id);
        }
        self.selected = selected
            .last()
            .copied()
            .or_else(|| additive.then_some(self.selected).flatten());
        selected.len()
    }

    pub fn clear_transient_interaction(&mut self) {
        self.dragging_node = None;
        self.connecting_from = None;
        self.connecting_sticky = false;
        self.marquee_start = None;
        self.marquee_current = None;
        self.context_menu = None;
    }

    pub fn has_active_interaction(&self) -> bool {
        self.dragging_node.is_some()
            || self.connecting_from.is_some()
            || self.marquee_start.is_some()
            || self.context_menu.is_some()
            || self.editing.is_some()
    }

    pub fn queue_operation_hint(
        &mut self,
        kind: impl Into<String>,
        details: impl Into<String>,
        field_path: Option<String>,
        push_undo_snapshot: bool,
    ) {
        self.queue_operation_hint_with_values(
            kind,
            details,
            field_path,
            None,
            None,
            push_undo_snapshot,
        );
    }

    pub fn queue_operation_hint_with_values(
        &mut self,
        kind: impl Into<String>,
        details: impl Into<String>,
        field_path: Option<String>,
        before_value: Option<String>,
        after_value: Option<String>,
        push_undo_snapshot: bool,
    ) {
        self.operation_hint = Some(GraphOperationHint {
            kind: kind.into(),
            details: details.into(),
            field_path,
            before_value,
            after_value,
            push_undo_snapshot,
        });
    }

    pub fn operation_hint_pushes_undo(&self) -> bool {
        self.operation_hint
            .as_ref()
            .map(|hint| hint.push_undo_snapshot)
            .unwrap_or(true)
    }

    pub fn take_operation_hint(&mut self) -> Option<GraphOperationHint> {
        self.operation_hint.take()
    }

    pub fn clear_operation_hint(&mut self) {
        self.operation_hint = None;
    }

    pub fn selected_node_ids(&self) -> Vec<u32> {
        if self.selected_nodes.is_empty() {
            self.selected.into_iter().collect()
        } else {
            self.selected_nodes.iter().copied().collect()
        }
    }
}

fn default_authoring_session() -> AuthoringDocumentSession {
    authoring_session_for_graph(&AuthoringGraph::new())
}

fn authoring_session_for_graph(authoring: &AuthoringGraph) -> AuthoringDocumentSession {
    AuthoringDocumentSession::new(AuthoringDocument::new(authoring.clone()))
}

fn default_zoom() -> f32 {
    ZOOM_DEFAULT
}
