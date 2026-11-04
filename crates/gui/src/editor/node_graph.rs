//! Node graph data structure for the visual editor.
//!
//! This module contains the core graph data structure that represents
//! the story flow. It handles node management and connections.
//! Script synchronization is in the `script_sync` module.

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub use visual_novel_engine::authoring::{GraphConnection, SceneProfile};
use visual_novel_engine::{
    authoring::{
        AuthoringCommand, AuthoringDelta, AuthoringDocument, AuthoringDocumentCommand,
        AuthoringDocumentSession, AuthoringPosition, NodeGraph as AuthoringGraph, OperationKind,
    },
    runtime::ScriptRaw,
};

use super::node_types::{
    node_visual_height, node_visual_width, ContextMenu, StoryNode, NODE_HEIGHT,
    NODE_VERTICAL_SPACING, NODE_WIDTH, ZOOM_DEFAULT, ZOOM_MAX, ZOOM_MIN,
};
use super::script_sync;

mod connections;
mod fragments;
mod interaction;
mod layout;
mod mutations;
mod navigation;
mod search;
mod view;

mod state;
pub use state::{GraphLayoutOrientation, GraphOperationHint, GraphViewState, NodeGraph};

impl NodeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a node at the specified position. Returns the node ID.
    pub fn add_node(&mut self, node: StoryNode, pos: egui::Pos2) -> u32 {
        let id = self.authoring_graph().next_node_id();
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
        self.authoring_graph().len()
    }

    /// Returns true if the graph is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.authoring_graph().is_empty()
    }

    /// Returns true if the graph has been modified since last save.
    #[inline]
    pub fn is_modified(&self) -> bool {
        self.view.modified
    }

    /// Clears the modified flag.
    pub fn clear_modified(&mut self) {
        self.view.modified = false;
    }

    /// Marks the graph as modified.
    pub fn mark_modified(&mut self) {
        self.view.modified = true;
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
            let node_rect = egui::Rect::from_min_size(
                pos,
                egui::vec2(node_visual_width(&node), node_visual_height(&node)),
            );
            if node_rect.contains(graph_pos) {
                return Some(id);
            }
        }
        None
    }

    /// Gets a reference to a node by ID.
    pub fn get_node(&self, id: u32) -> Option<&StoryNode> {
        self.authoring_graph().get_node(id)
    }

    /// Edits a node through the document session, keeping validation and undo aligned.
    pub fn edit_node(&mut self, id: u32, edit: impl FnOnce(&mut StoryNode)) -> bool {
        let Some(mut node) = self.get_node(id).cloned() else {
            return false;
        };
        edit(&mut node);
        self.replace_node_with_hint(
            id,
            node,
            format!("Edited node {id}"),
            format!("graph.nodes[{id}]"),
            true,
        )
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
        let details = details.into();
        let field_path = field_path.into();
        let changed = self
            .authoring_session
            .apply_with_description(
                AuthoringDocumentCommand::Graph(AuthoringCommand::EditNode {
                    node_id: id,
                    replacement,
                }),
                details.clone(),
                field_path.clone(),
                before_value.clone(),
                after_value.clone(),
            )
            .is_ok();
        if changed {
            self.queue_operation_hint_with_values(
                "field_edited",
                details,
                Some(field_path),
                before_value,
                after_value,
                push_undo_snapshot,
            );
            self.mark_modified();
        }
        changed
    }

    pub fn get_node_pos(&self, id: u32) -> Option<egui::Pos2> {
        self.view
            .drag_positions
            .get(&id)
            .copied()
            .or_else(|| self.authoring_graph().get_node_pos(id))
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
        let position = AuthoringPosition::new(pos.x, pos.y);
        let changed = if push_undo_snapshot {
            self.apply_node_positions_reported(BTreeMap::from([(id, position)]))
        } else if self.get_node_pos(id).is_some_and(|before| before != pos) {
            self.view.drag_positions.insert(id, position);
            self.mark_modified();
            true
        } else {
            false
        };
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

    pub fn commit_drag_positions(&mut self) -> bool {
        match self.try_commit_drag_positions() {
            Ok(changed) => changed,
            Err(error) => {
                tracing::error!("Could not commit dragged nodes: {error}");
                false
            }
        }
    }

    pub fn try_commit_drag_positions(&mut self) -> Result<bool, String> {
        let positions = self.view.drag_positions.clone();
        let changed = self.apply_node_positions(positions)?;
        self.view.drag_positions.clear();
        Ok(changed)
    }

    fn apply_node_positions_reported(
        &mut self,
        positions: BTreeMap<u32, AuthoringPosition>,
    ) -> bool {
        match self.apply_node_positions(positions) {
            Ok(changed) => changed,
            Err(error) => {
                tracing::error!("Could not move nodes: {error}");
                false
            }
        }
    }

    fn apply_node_positions(
        &mut self,
        positions: BTreeMap<u32, AuthoringPosition>,
    ) -> Result<bool, String> {
        if positions.is_empty()
            || positions
                .iter()
                .all(|(id, pos)| self.authoring_graph().get_node_pos(*id) == Some(*pos))
        {
            return Ok(false);
        }
        self.authoring_session
            .apply(AuthoringDocumentCommand::MoveNodes { positions })?;
        self.mark_modified();
        Ok(true)
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
        if drag_preview {
            return ids
                .into_iter()
                .filter(|node_id| self.translate_node_for_drag(*node_id, delta))
                .count();
        }
        let positions: BTreeMap<_, _> = ids
            .into_iter()
            .filter_map(|id| {
                self.get_node_pos(id).map(|pos| {
                    let moved = pos + delta;
                    (id, AuthoringPosition::new(moved.x, moved.y))
                })
            })
            .filter(|(id, pos)| self.authoring_graph().get_node_pos(*id) != Some(*pos))
            .collect();
        let count = positions.len();
        if self.apply_node_positions_reported(positions) {
            self.queue_operation_hint(
                "node_moved",
                "Moved selected nodes",
                Some("graph.nodes".into()),
                true,
            );
            count
        } else {
            0
        }
    }

    /// Returns an iterator over all nodes as GUI-positioned snapshots.
    pub fn nodes(&self) -> impl Iterator<Item = (u32, StoryNode, egui::Pos2)> + '_ {
        self.authoring_graph().nodes().map(|(id, node, pos)| {
            let pos = self.view.drag_positions.get(id).unwrap_or(pos);
            (*id, node.clone(), egui::pos2(pos.x, pos.y))
        })
    }

    /// Returns an iterator over all connections as snapshots.
    pub fn connections(&self) -> impl Iterator<Item = GraphConnection> + '_ {
        self.authoring_graph().connections().cloned()
    }

    pub fn from_authoring_graph(authoring: AuthoringGraph) -> Self {
        Self::from_document(AuthoringDocument::new(authoring))
    }

    pub fn from_document(document: AuthoringDocument) -> Self {
        Self {
            authoring_session: AuthoringDocumentSession::new(document),
            view: GraphViewState::default(),
        }
    }

    pub fn authoring_graph(&self) -> &AuthoringGraph {
        &self.authoring_session.document().graph
    }

    pub(super) fn history_available(
        &self,
        command: &AuthoringDocumentCommand,
    ) -> Result<bool, String> {
        match command {
            AuthoringDocumentCommand::UndoLast | AuthoringDocumentCommand::RevertLast => {
                Ok(self.authoring_session.undo_delta_count() > 0)
            }
            AuthoringDocumentCommand::RedoLast => Ok(self.authoring_session.redo_delta_count() > 0),
            _ => Err("document history command required".to_string()),
        }
    }

    /// Applies a document history command and reconciles the graph view.
    pub(super) fn apply_document_history(
        &mut self,
        command: AuthoringDocumentCommand,
    ) -> Result<Option<visual_novel_engine::authoring::AuthoringDocumentCommandOutcome>, String>
    {
        if !self.history_available(&command)? {
            return Ok(None);
        }
        let outcome = self.authoring_session.apply(command)?;
        self.reconcile_view_after_document_change();
        Ok(Some(outcome))
    }

    pub fn replace_authoring_graph(&mut self, graph: AuthoringGraph) {
        self.edit_graph("Replace graph", |working| *working = graph);
    }

    pub fn edit_graph<R>(
        &mut self,
        details: &str,
        edit: impl FnOnce(&mut AuthoringGraph) -> R,
    ) -> R {
        let (result, changed) = self
            .authoring_session
            .edit_graph(
                OperationKind::FieldEdited,
                details,
                Some("graph".into()),
                edit,
            )
            .expect("a changed graph is a valid replacement");
        if changed {
            self.mark_modified();
        }
        result
    }

    fn apply_authoring_command(&mut self, command: AuthoringCommand) -> Option<AuthoringDelta> {
        let result = self
            .authoring_session
            .apply(AuthoringDocumentCommand::Graph(command))
            .ok();
        if result.is_some() {
            self.mark_modified();
        }
        result.and_then(|outcome| match outcome.delta {
            visual_novel_engine::authoring::AuthoringDocumentDelta::Graph(delta) => Some(*delta),
            _ => None,
        })
    }
}

fn default_zoom() -> f32 {
    ZOOM_DEFAULT
}

#[cfg(test)]
mod history_tests {
    use super::*;

    #[test]
    fn history_rejects_other_commands_and_keeps_empty_history_a_noop() {
        let mut graph = NodeGraph::new();
        assert!(graph
            .apply_document_history(AuthoringDocumentCommand::UndoLast)
            .unwrap()
            .is_none());
        assert!(graph
            .apply_document_history(AuthoringDocumentCommand::RedoLast)
            .unwrap()
            .is_none());
        assert_eq!(
            graph
                .apply_document_history(AuthoringDocumentCommand::ResetComposerOverrides)
                .unwrap_err(),
            "document history command required"
        );
        assert!(graph.is_empty());
    }
}
