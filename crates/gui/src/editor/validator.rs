#[cfg(test)]
use std::path::Path;

use crate::editor::authoring_adapter::to_authoring_graph;
use crate::editor::node_graph::NodeGraph;

pub use visual_novel_engine::authoring::{LintCode, LintIssue, LintSeverity, ValidationPhase};

pub fn validate(graph: &NodeGraph) -> Vec<LintIssue> {
    let authoring = to_authoring_graph(graph);
    visual_novel_engine::authoring::validate_authoring_graph_with_resolver(
        &authoring,
        visual_novel_engine::authoring::default_asset_exists,
    )
}

#[allow(dead_code)]
pub fn validate_with_asset_probe<F>(graph: &NodeGraph, asset_exists: F) -> Vec<LintIssue>
where
    F: Fn(&str) -> bool,
{
    let authoring = to_authoring_graph(graph);
    visual_novel_engine::authoring::validate_authoring_graph_with_resolver(&authoring, asset_exists)
}

#[cfg(test)]
pub fn validate_with_project_root(graph: &NodeGraph, project_root: &Path) -> Vec<LintIssue> {
    let authoring = to_authoring_graph(graph);
    visual_novel_engine::authoring::validate_authoring_graph_with_project_root(
        &authoring,
        project_root,
    )
}

#[cfg(test)]
#[path = "tests/validator_tests.rs"]
mod tests;
