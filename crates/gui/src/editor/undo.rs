//! Undo/Redo system for the node editor.
//!
//! Provides an undo stack that stores graph snapshots before each action.
//! Maximum 50 states to limit memory usage.

use std::collections::VecDeque;

use super::node_graph::NodeGraph;

/// Maximum number of undo states to keep in memory.
const MAX_UNDO_STATES: usize = 50;

/// Manages undo/redo history for a NodeGraph.
///
/// # Design
/// - Stores full clones of NodeGraph (simple but memory-heavy)
/// - Clears redo stack on new actions (standard behavior)
/// - Limited to MAX_UNDO_STATES to prevent unbounded growth
#[derive(Clone, Debug, Default)]
pub struct UndoStack {
    /// History of past states (most recent at back)
    history: VecDeque<NodeGraph>,
    /// States available for redo (most recent at back)
    redo_stack: VecDeque<NodeGraph>,
}

impl UndoStack {
    /// Creates a new empty undo stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes the current state before an action.
    ///
    /// # Contract
    /// - Clears redo stack (new action invalidates redo)
    /// - Limits history to MAX_UNDO_STATES
    pub fn push(&mut self, state: NodeGraph) {
        // Clear redo stack - new action invalidates future
        self.redo_stack.clear();

        // Add to history
        self.history.push_back(state);

        // Limit size
        while self.history.len() > MAX_UNDO_STATES {
            self.history.pop_front();
        }

        debug_assert!(
            self.history.len() <= MAX_UNDO_STATES,
            "History should not exceed max size"
        );
    }

    /// Undoes the last action, returning the previous state.
    ///
    /// # Returns
    /// - Some(state) if there was a state to restore
    /// - None if history is empty
    pub fn undo(&mut self, current: NodeGraph) -> Option<NodeGraph> {
        if let Some(previous) = self.history.pop_back() {
            // Save current for redo
            self.redo_stack.push_back(current);
            Some(previous)
        } else {
            None
        }
    }

    /// Redoes the last undone action.
    ///
    /// # Returns
    /// - Some(state) if there was a state to redo
    /// - None if redo stack is empty
    pub fn redo(&mut self, current: NodeGraph) -> Option<NodeGraph> {
        if let Some(next) = self.redo_stack.pop_back() {
            // Save current for undo
            self.history.push_back(current);
            Some(next)
        } else {
            None
        }
    }

    /// Returns true if undo is available.
    #[inline]
    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    /// Returns true if redo is available.
    #[inline]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clears all history.
    pub fn clear(&mut self) {
        self.history.clear();
        self.redo_stack.clear();
    }

    /// Returns the number of undo states available.
    #[inline]
    pub fn undo_count(&self) -> usize {
        self.history.len()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::node_types::StoryNode;

    fn create_graph_with_nodes(count: usize) -> NodeGraph {
        let mut graph = NodeGraph::new();
        for i in 0..count {
            graph.add_node(StoryNode::Start, eframe::egui::pos2(i as f32 * 50.0, 0.0));
        }
        graph
    }

    #[test]
    fn test_undo_stack_push_and_undo() {
        let mut stack = UndoStack::new();

        let state1 = create_graph_with_nodes(1);
        let state2 = create_graph_with_nodes(2);
        let state3 = create_graph_with_nodes(3);

        stack.push(state1.clone());
        stack.push(state2.clone());

        assert!(stack.can_undo());
        assert!(!stack.can_redo());

        // Undo returns previous state
        let restored = stack.undo(state3.clone()).unwrap();
        assert_eq!(restored.len(), 2); // state2

        assert!(stack.can_redo());
    }

    #[test]
    fn test_undo_redo_cycle() {
        let mut stack = UndoStack::new();

        let state1 = create_graph_with_nodes(1);
        let state2 = create_graph_with_nodes(2);

        stack.push(state1.clone());

        // Undo
        let restored = stack.undo(state2.clone()).unwrap();
        assert_eq!(restored.len(), 1);

        // Redo
        let redone = stack.redo(restored).unwrap();
        assert_eq!(redone.len(), 2);
    }

    #[test]
    fn test_new_action_clears_redo() {
        let mut stack = UndoStack::new();

        let state1 = create_graph_with_nodes(1);
        let state2 = create_graph_with_nodes(2);
        let state3 = create_graph_with_nodes(3);

        stack.push(state1.clone());
        stack.undo(state2.clone());

        assert!(stack.can_redo());

        // New action should clear redo
        stack.push(state3);
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_max_history_limit() {
        let mut stack = UndoStack::new();

        // Push more than max states
        for i in 0..60 {
            stack.push(create_graph_with_nodes(i));
        }

        assert_eq!(stack.undo_count(), MAX_UNDO_STATES);
    }
}
