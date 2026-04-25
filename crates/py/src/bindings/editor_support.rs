use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use visual_novel_engine::CmpOp;
use visual_novel_gui::editor::quick_fix::{
    apply_fix, suggest_fixes, QuickFixCandidate, QuickFixRisk,
};
use visual_novel_gui::editor::{validate_graph, LintIssue, NodeGraph};

pub(super) fn parse_cmp_op(op: &str) -> PyResult<CmpOp> {
    match op {
        "eq" => Ok(CmpOp::Eq),
        "ne" => Ok(CmpOp::Ne),
        "lt" => Ok(CmpOp::Lt),
        "le" => Ok(CmpOp::Le),
        "gt" => Ok(CmpOp::Gt),
        "ge" => Ok(CmpOp::Ge),
        _ => Err(PyValueError::new_err(format!(
            "Unknown comparison op '{op}'"
        ))),
    }
}

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

pub(super) fn apply_autofix_pass(
    graph: &mut NodeGraph,
    include_review: bool,
) -> Result<usize, String> {
    let mut applied = 0usize;
    let mut guard = 0usize;

    while guard < 128 {
        guard += 1;
        let issues = validate_graph(graph);
        let mut applied_this_round = false;

        for issue in issues {
            let Some(candidate) = select_fix_candidate(&issue, graph, include_review) else {
                continue;
            };
            if apply_fix(graph, &issue, candidate.fix_id)? {
                applied += 1;
                applied_this_round = true;
                break;
            }
        }

        if !applied_this_round {
            break;
        }
    }

    Ok(applied)
}
