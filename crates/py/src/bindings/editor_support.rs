use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use visual_novel_engine::authoring::quick_fix::{suggest_fixes, QuickFixCandidate, QuickFixRisk};
use visual_novel_engine::authoring::{LintIssue, NodeGraph};
use visual_novel_engine::runtime::CmpOp;

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
