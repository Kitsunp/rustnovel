use crate::editor::authoring_adapter::{replace_gui_semantics_from_authoring, to_authoring_graph};
use crate::editor::{LintIssue, NodeGraph};

pub use visual_novel_engine::authoring::quick_fix::{QuickFixCandidate, QuickFixRisk};

pub fn suggest_fixes(issue: &LintIssue, graph: &NodeGraph) -> Vec<QuickFixCandidate> {
    let authoring = to_authoring_graph(graph);
    visual_novel_engine::authoring::quick_fix::suggest_fixes(issue, &authoring)
}

pub fn apply_fix(graph: &mut NodeGraph, issue: &LintIssue, fix_id: &str) -> Result<bool, String> {
    let mut authoring = to_authoring_graph(graph);
    let changed =
        visual_novel_engine::authoring::quick_fix::apply_fix(&mut authoring, issue, fix_id)?;
    if changed {
        replace_gui_semantics_from_authoring(graph, &authoring);
    }
    Ok(changed)
}

#[cfg(test)]
#[path = "tests/quick_fix_tests.rs"]
mod tests;
