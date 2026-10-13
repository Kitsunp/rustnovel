use std::path::Path;

use crate::editor::node_graph::NodeGraph;

pub use visual_novel_engine::authoring::{LintCode, LintIssue, LintSeverity, ValidationPhase};

pub fn validate(graph: &NodeGraph) -> Vec<LintIssue> {
    visual_novel_engine::authoring::validate_authoring_graph_no_io(graph.authoring_graph())
}

#[allow(dead_code)]
pub fn validate_with_asset_probe<F>(graph: &NodeGraph, asset_exists: F) -> Vec<LintIssue>
where
    F: Fn(&str) -> bool,
{
    visual_novel_engine::authoring::validate_authoring_graph_with_resolver(
        graph.authoring_graph(),
        asset_exists,
    )
}

pub fn validate_with_project_root(graph: &NodeGraph, project_root: &Path) -> Vec<LintIssue> {
    visual_novel_engine::authoring::validate_authoring_graph_with_project_root(
        graph.authoring_graph(),
        project_root,
    )
}
