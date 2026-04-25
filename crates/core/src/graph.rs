//! Story graph generation and analysis for the Visual Novel Engine.
//!
//! This module generates a directed graph representation of the narrative flow
//! from compiled scripts. It enables:
//! - Visualization of story structure
//! - Detection of unreachable nodes (dead code)
//! - Navigation in the editor
//!
//! # Contracts
//! - **Precondition**: Graph is generated from a valid `ScriptCompiled`.
//! - **Postcondition**: All reachable nodes are marked, unreachable nodes are flagged.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::script::ScriptCompiled;

pub use analysis::{analyze_flow_graph, FlowGraphAnalysis};

// =============================================================================
// Node Types
// =============================================================================

/// Unique identifier for a graph node (corresponds to event index/IP).
pub type NodeId = u32;

/// Type of node in the story graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// A dialogue event.
    Dialogue {
        speaker: String,
        text_preview: String,
    },
    /// A choice point with multiple options.
    Choice { prompt: String, option_count: usize },
    /// A scene change.
    Scene { background: Option<String> },
    /// An unconditional jump.
    Jump,
    /// A conditional jump.
    ConditionalJump { condition: String },
    /// A flag or variable modification.
    StateChange { description: String },
    /// A scene patch (partial update).
    Patch,
    /// An external command call.
    ExtCall { command: String },
    /// An audio action.
    AudioAction {
        channel: u8,
        action: u8,
        asset: Option<String>,
    },
    /// A scene transition.
    Transition { kind: String, duration: u64 },
    /// Explicit character placement with coordinates.
    CharacterPlacement {
        name: String,
        x: i32,
        y: i32,
        scale: Option<f32>,
    },
}

/// A node in the story graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphNode {
    /// The instruction pointer / event index.
    pub id: NodeId,
    /// The type of node.
    pub node_type: NodeType,
    /// Label(s) pointing to this node (if any).
    pub labels: Vec<String>,
    /// Whether this node is reachable from the start.
    pub reachable: bool,
}

// =============================================================================
// Edge Types
// =============================================================================

/// Type of transition between nodes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// Normal sequential flow (next instruction).
    Sequential,
    /// Unconditional jump.
    Jump,
    /// Conditional jump (when condition is true).
    ConditionalTrue,
    /// Conditional jump fallthrough (when condition is false).
    ConditionalFalse,
    /// Choice option selected.
    Choice { option_index: usize },
}

/// A directed edge in the story graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Source node ID.
    pub from: NodeId,
    /// Target node ID.
    pub to: NodeId,
    /// Type of edge.
    pub edge_type: EdgeType,
    /// Optional label (e.g., choice text).
    pub label: Option<String>,
}

// =============================================================================
// Story Graph
// =============================================================================

/// The complete story graph generated from a compiled script.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StoryGraph {
    /// All nodes in the graph.
    pub nodes: Vec<GraphNode>,
    /// All edges in the graph.
    pub edges: Vec<GraphEdge>,
    /// The starting node ID.
    pub start_id: NodeId,
    /// Labels mapped to node IDs.
    pub label_map: BTreeMap<String, NodeId>,
}

/// Statistics about the story graph.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GraphStats {
    /// Total number of nodes.
    pub total_nodes: usize,
    /// Number of reachable nodes.
    pub reachable_nodes: usize,
    /// Number of unreachable nodes.
    pub unreachable_nodes: usize,
    /// Number of dialogue nodes.
    pub dialogue_count: usize,
    /// Number of choice nodes.
    pub choice_count: usize,
    /// Number of branch points (choices + conditionals).
    pub branch_count: usize,
    /// Total number of edges.
    pub edge_count: usize,
}

impl StoryGraph {
    /// Generates a story graph from a compiled script.
    pub fn from_script(script: &ScriptCompiled) -> Self {
        let mut nodes = Vec::with_capacity(script.events.len());
        let mut edges = Vec::new();

        // Create reverse label map
        let mut label_map: BTreeMap<String, NodeId> = BTreeMap::new();
        for (label, &ip) in &script.labels {
            label_map.insert(label.clone(), ip);
        }

        // Create IP to labels mapping
        let mut ip_labels: BTreeMap<NodeId, Vec<String>> = BTreeMap::new();
        for (label, &ip) in &script.labels {
            ip_labels.entry(ip).or_default().push(label.clone());
        }

        // Generate nodes and edges
        for (ip, event) in script.events.iter().enumerate() {
            let ip = ip as NodeId;
            let labels = ip_labels.get(&ip).cloned().unwrap_or_default();

            let (node_type, event_edges) = Self::process_event(ip, event, script.events.len());

            nodes.push(GraphNode {
                id: ip,
                node_type,
                labels,
                reachable: false, // Will be computed later
            });

            edges.extend(event_edges);
        }

        let mut graph = Self {
            nodes,
            edges,
            start_id: script.start_ip,
            label_map,
        };

        // Compute reachability
        graph.compute_reachability();

        graph
    }

    fn compute_reachability(&mut self) {
        let analysis = self.flow_analysis();

        // Mark nodes as reachable
        for node in &mut self.nodes {
            node.reachable = analysis.reachable.contains(&node.id);
        }
    }

    /// Returns canonical reachability and reachable-cycle analysis for this graph.
    pub fn flow_analysis(&self) -> FlowGraphAnalysis {
        let nodes = self.nodes.iter().map(|node| node.id).collect::<Vec<_>>();
        let edges = self
            .edges
            .iter()
            .map(|edge| (edge.from, edge.to))
            .collect::<Vec<_>>();
        analyze_flow_graph(&nodes, &edges, &[self.start_id])
    }

    /// Returns statistics about the graph.
    pub fn stats(&self) -> GraphStats {
        let reachable_nodes = self.nodes.iter().filter(|n| n.reachable).count();
        let dialogue_count = self
            .nodes
            .iter()
            .filter(|n| matches!(n.node_type, NodeType::Dialogue { .. }))
            .count();
        let choice_count = self
            .nodes
            .iter()
            .filter(|n| matches!(n.node_type, NodeType::Choice { .. }))
            .count();
        let conditional_count = self
            .nodes
            .iter()
            .filter(|n| matches!(n.node_type, NodeType::ConditionalJump { .. }))
            .count();

        GraphStats {
            total_nodes: self.nodes.len(),
            reachable_nodes,
            unreachable_nodes: self.nodes.len() - reachable_nodes,
            dialogue_count,
            choice_count,
            branch_count: choice_count + conditional_count,
            edge_count: self.edges.len(),
        }
    }

    /// Returns all unreachable node IDs.
    pub fn unreachable_nodes(&self) -> Vec<NodeId> {
        self.nodes
            .iter()
            .filter(|n| !n.reachable)
            .map(|n| n.id)
            .collect()
    }

    /// Gets a node by ID.
    pub fn get_node(&self, id: NodeId) -> Option<&GraphNode> {
        self.nodes.get(id as usize)
    }

    /// Gets all outgoing edges from a node.
    pub fn outgoing_edges(&self, id: NodeId) -> Vec<&GraphEdge> {
        self.edges.iter().filter(|e| e.from == id).collect()
    }

    /// Gets all incoming edges to a node.
    pub fn incoming_edges(&self, id: NodeId) -> Vec<&GraphEdge> {
        self.edges.iter().filter(|e| e.to == id).collect()
    }

    /// Finds a node by label.
    pub fn find_by_label(&self, label: &str) -> Option<NodeId> {
        self.label_map.get(label).copied()
    }
}

// =============================================================================
// Tests
// =============================================================================

mod analysis;
mod build;
mod export;
#[cfg(test)]
#[path = "tests/graph_tests.rs"]
mod tests;
