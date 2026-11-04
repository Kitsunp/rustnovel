use visual_novel_engine::authoring::quick_fix::{suggest_fixes, QuickFixCandidate, QuickFixRisk};
use visual_novel_engine::authoring::{LintIssue, NodeGraph};

pub(super) fn select_fix_candidate(
    issue: &LintIssue,
    graph: &NodeGraph,
    include_review: bool,
) -> Option<QuickFixCandidate> {
    let candidates = suggest_fixes(issue, graph);
    if include_review {
        candidates
            .iter()
            .find(|candidate| candidate.risk == QuickFixRisk::Safe)
            .cloned()
            .or_else(|| candidates.into_iter().next())
    } else {
        candidates
            .into_iter()
            .find(|candidate| candidate.risk == QuickFixRisk::Safe)
    }
}
