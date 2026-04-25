#[cfg(test)]
use std::path::Path;

use crate::editor::node_graph::NodeGraph;
use crate::editor::node_types::StoryNode;

pub use visual_novel_engine::authoring::{LintCode, LintIssue, LintSeverity, ValidationPhase};

pub fn validate(graph: &NodeGraph) -> Vec<LintIssue> {
    validate_with_asset_probe(graph, helpers::default_asset_exists)
}

pub fn validate_with_asset_probe<F>(graph: &NodeGraph, asset_exists: F) -> Vec<LintIssue>
where
    F: Fn(&str) -> bool,
{
    rules::validate_with_asset_probe_impl(graph, asset_exists)
}

#[cfg(test)]
pub fn validate_with_project_root(graph: &NodeGraph, project_root: &Path) -> Vec<LintIssue> {
    rules::validate_with_asset_probe_impl(graph, |asset| {
        helpers::asset_exists_from_project_root(project_root, asset)
    })
}

mod context;
mod helpers;
mod rules;

#[cfg(test)]
#[path = "tests/validator_tests.rs"]
mod tests;
