//! Script synchronization for the visual editor graph.
//!
//! Semantic import/export is delegated to `visual_novel_engine::authoring`.
//! The GUI layer only adapts egui positions and interaction state.

use visual_novel_engine::{authoring::NodeGraph as AuthoringGraph, runtime::ScriptRaw};

use super::authoring_adapter::{from_authoring_graph, to_authoring_graph};
use super::node_graph::NodeGraph;

/// Creates a GUI `NodeGraph` from a raw script.
///
/// The headless authoring model owns the script semantics. The GUI applies its
/// layout pass afterward so imported scripts remain immediately navigable.
pub fn from_script(script: &ScriptRaw) -> NodeGraph {
    let authoring = AuthoringGraph::from_script(script);
    let mut graph = from_authoring_graph(&authoring);
    graph.auto_layout_hierarchical();
    graph.zoom_to_fit();
    graph.clear_modified();
    graph
}

/// Converts a GUI `NodeGraph` to a raw script.
pub fn to_script(graph: &NodeGraph) -> ScriptRaw {
    to_authoring_graph(graph).to_script()
}
