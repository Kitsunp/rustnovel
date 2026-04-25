use std::path::Path;

use crate::editor::{authoring_adapter::to_authoring_graph, node_graph::NodeGraph};

pub use visual_novel_engine::authoring::compiler::{
    compile_authoring_graph, enumerate_choice_routes, simulate_raw_sequence, ChoicePolicy,
    ChoiceStrategy, CompilationPhase, CompilationResult, DryRunReport, DryRunStepTrace,
    DryRunStopReason, PhaseTrace,
};
pub use visual_novel_engine::authoring::{LintCode, LintIssue, LintSeverity, ValidationPhase};

pub fn compile_project(graph: &NodeGraph) -> CompilationResult {
    compile_project_with_project_root(graph, None)
}

pub fn compile_project_with_project_root(
    graph: &NodeGraph,
    project_root: Option<&Path>,
) -> CompilationResult {
    let authoring = to_authoring_graph(graph);
    compile_authoring_graph(&authoring, project_root)
}

#[cfg(test)]
#[path = "tests/compiler_tests.rs"]
mod tests;
