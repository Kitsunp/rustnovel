#[path = "rules_node.rs"]
mod node_rules;

use super::context::unreachable_blocker_context;
use super::helpers::analyze_editor_flow;
use super::*;
use std::collections::HashSet;

pub(super) fn validate_with_asset_probe_impl<F>(
    graph: &NodeGraph,
    asset_exists: F,
) -> Vec<LintIssue>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    let start_nodes: Vec<u32> = graph
        .nodes
        .iter()
        .filter_map(|(id, node, _)| {
            if matches!(node, StoryNode::Start) {
                Some(*id)
            } else {
                None
            }
        })
        .collect();

    if start_nodes.is_empty() {
        issues.push(LintIssue::error(
            None,
            ValidationPhase::Graph,
            LintCode::MissingStart,
            "Missing Start node",
        ));
    } else if start_nodes.len() > 1 {
        issues.push(LintIssue::warning(
            None,
            ValidationPhase::Graph,
            LintCode::MultipleStart,
            format!("Multiple Start nodes found ({})", start_nodes.len()),
        ));
    }

    let flow = analyze_editor_flow(graph, &start_nodes);
    let visited = flow.reachable.iter().copied().collect::<HashSet<_>>();

    for (id, _, _) in &graph.nodes {
        if !visited.contains(id) {
            let (edge_from, blocked_by) = unreachable_blocker_context(graph, *id, &visited);
            let mut issue = LintIssue::warning(
                Some(*id),
                ValidationPhase::Graph,
                LintCode::UnreachableNode,
                "Unreachable node (dead code) - blocked flow",
            )
            .with_blocked_by(blocked_by);
            if let Some(from_id) = edge_from {
                issue = issue.with_edge(Some(from_id), Some(*id));
            }
            issues.push(issue);
        }
    }

    for node_id in flow.reachable_cycle_nodes {
        issues.push(LintIssue::warning(
            Some(node_id),
            ValidationPhase::Graph,
            LintCode::PotentialLoop,
            "Potential execution loop detected on reachable route",
        ));
    }

    for (id, node, _) in &graph.nodes {
        node_rules::validate_node(graph, *id, node, &asset_exists, &mut issues);
    }

    issues
}
