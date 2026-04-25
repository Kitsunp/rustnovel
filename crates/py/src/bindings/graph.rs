//! Python bindings for the story graph system.
//!
//! These bindings expose the story graph for visualization and analysis
//! in Python-based editors and tools.

use pyo3::prelude::*;
use visual_novel_engine::{GraphStats, ScriptRaw, StoryGraph};

/// Python wrapper for GraphStats.
#[pyclass(name = "GraphStats")]
#[derive(Clone)]
pub struct PyGraphStats {
    #[pyo3(get)]
    pub total_nodes: usize,
    #[pyo3(get)]
    pub reachable_nodes: usize,
    #[pyo3(get)]
    pub unreachable_nodes: usize,
    #[pyo3(get)]
    pub dialogue_count: usize,
    #[pyo3(get)]
    pub choice_count: usize,
    #[pyo3(get)]
    pub branch_count: usize,
    #[pyo3(get)]
    pub edge_count: usize,
}

impl From<GraphStats> for PyGraphStats {
    fn from(stats: GraphStats) -> Self {
        Self {
            total_nodes: stats.total_nodes,
            reachable_nodes: stats.reachable_nodes,
            unreachable_nodes: stats.unreachable_nodes,
            dialogue_count: stats.dialogue_count,
            choice_count: stats.choice_count,
            branch_count: stats.branch_count,
            edge_count: stats.edge_count,
        }
    }
}

#[pymethods]
impl PyGraphStats {
    fn __repr__(&self) -> String {
        format!(
            "GraphStats(nodes={}, reachable={}, unreachable={}, dialogues={}, choices={}, branches={}, edges={})",
            self.total_nodes,
            self.reachable_nodes,
            self.unreachable_nodes,
            self.dialogue_count,
            self.choice_count,
            self.branch_count,
            self.edge_count
        )
    }
}

/// Python wrapper for a graph node.
#[pyclass(name = "GraphNode")]
#[derive(Clone)]
pub struct PyGraphNode {
    #[pyo3(get)]
    pub id: u32,
    #[pyo3(get)]
    pub node_type: String,
    #[pyo3(get)]
    pub labels: Vec<String>,
    #[pyo3(get)]
    pub reachable: bool,
    #[pyo3(get)]
    pub details: String,
}

#[pymethods]
impl PyGraphNode {
    fn __repr__(&self) -> String {
        format!(
            "GraphNode(id={}, type='{}', reachable={}, labels={:?})",
            self.id, self.node_type, self.reachable, self.labels
        )
    }
}

/// Python wrapper for a graph edge.
#[pyclass(name = "GraphEdge")]
#[derive(Clone)]
pub struct PyGraphEdge {
    #[pyo3(get)]
    pub from_id: u32,
    #[pyo3(get)]
    pub to_id: u32,
    #[pyo3(get)]
    pub edge_type: String,
    #[pyo3(get)]
    pub label: Option<String>,
}

#[pymethods]
impl PyGraphEdge {
    fn __repr__(&self) -> String {
        format!(
            "GraphEdge({} -> {}, type='{}')",
            self.from_id, self.to_id, self.edge_type
        )
    }
}

/// Python wrapper for the StoryGraph.
#[pyclass(name = "StoryGraph")]
pub struct PyStoryGraph {
    inner: StoryGraph,
}

#[pymethods]
impl PyStoryGraph {
    /// Creates a story graph from a script JSON string.
    #[staticmethod]
    fn from_json(script_json: &str) -> PyResult<Self> {
        let raw = ScriptRaw::from_json(script_json)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let compiled = raw
            .compile()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let graph = StoryGraph::from_script(&compiled);
        Ok(Self { inner: graph })
    }

    /// Returns graph statistics.
    fn stats(&self) -> PyGraphStats {
        self.inner.stats().into()
    }

    /// Returns all nodes.
    fn nodes(&self) -> Vec<PyGraphNode> {
        self.inner
            .nodes
            .iter()
            .map(|n| {
                let (node_type, details) = match &n.node_type {
                    visual_novel_engine::NodeType::Dialogue {
                        speaker,
                        text_preview,
                    } => (
                        "dialogue".to_string(),
                        format!("{}: {}", speaker, text_preview),
                    ),
                    visual_novel_engine::NodeType::Choice {
                        prompt,
                        option_count,
                    } => (
                        "choice".to_string(),
                        format!("{} ({} options)", prompt, option_count),
                    ),
                    visual_novel_engine::NodeType::Scene { background } => {
                        ("scene".to_string(), format!("{:?}", background))
                    }
                    visual_novel_engine::NodeType::Jump => ("jump".to_string(), String::new()),
                    visual_novel_engine::NodeType::ConditionalJump { condition } => {
                        ("conditional".to_string(), condition.clone())
                    }
                    visual_novel_engine::NodeType::StateChange { description } => {
                        ("state_change".to_string(), description.clone())
                    }
                    visual_novel_engine::NodeType::Patch => ("patch".to_string(), String::new()),
                    visual_novel_engine::NodeType::ExtCall { command } => {
                        ("ext_call".to_string(), command.clone())
                    }
                    visual_novel_engine::NodeType::AudioAction {
                        channel,
                        action,
                        asset,
                    } => (
                        "audio_action".to_string(),
                        format!(
                            "Channel: {}, Action: {}, Asset: {:?}",
                            channel, action, asset
                        ),
                    ),
                    visual_novel_engine::NodeType::Transition { kind, duration } => (
                        "transition".to_string(),
                        format!("Kind: {}, Duration: {}", kind, duration),
                    ),
                    visual_novel_engine::NodeType::CharacterPlacement { name, x, y, scale } => (
                        "character_placement".to_string(),
                        format!("Name: {}, x: {}, y: {}, scale: {:?}", name, x, y, scale),
                    ),
                };
                PyGraphNode {
                    id: n.id,
                    node_type,
                    labels: n.labels.clone(),
                    reachable: n.reachable,
                    details,
                }
            })
            .collect()
    }

    /// Returns all edges.
    fn edges(&self) -> Vec<PyGraphEdge> {
        self.inner
            .edges
            .iter()
            .map(|e| {
                let edge_type = match &e.edge_type {
                    visual_novel_engine::EdgeType::Sequential => "sequential",
                    visual_novel_engine::EdgeType::Jump => "jump",
                    visual_novel_engine::EdgeType::ConditionalTrue => "conditional_true",
                    visual_novel_engine::EdgeType::ConditionalFalse => "conditional_false",
                    visual_novel_engine::EdgeType::Choice { .. } => "choice",
                };
                PyGraphEdge {
                    from_id: e.from,
                    to_id: e.to,
                    edge_type: edge_type.to_string(),
                    label: e.label.clone(),
                }
            })
            .collect()
    }

    /// Returns IDs of unreachable nodes.
    fn unreachable_nodes(&self) -> Vec<u32> {
        self.inner.unreachable_nodes()
    }

    /// Finds a node by label.
    fn find_by_label(&self, label: &str) -> Option<u32> {
        self.inner.find_by_label(label)
    }

    /// Exports the graph to DOT format for Graphviz visualization.
    fn to_dot(&self) -> String {
        self.inner.to_dot()
    }

    /// Returns the starting node ID.
    #[getter]
    fn start_id(&self) -> u32 {
        self.inner.start_id
    }

    fn __repr__(&self) -> String {
        let stats = self.inner.stats();
        format!(
            "StoryGraph(nodes={}, edges={}, unreachable={})",
            stats.total_nodes, stats.edge_count, stats.unreachable_nodes
        )
    }

    fn __len__(&self) -> usize {
        self.inner.nodes.len()
    }
}
