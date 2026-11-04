use std::collections::BTreeSet;
use std::path::Path;

pub(crate) mod assets;
pub(crate) mod event_details;
mod flow;
pub(crate) mod scene;
pub(crate) mod trace;

pub use assets::{asset_exists_from_project_root, is_unsafe_asset_ref, should_probe_asset_exists};
use flow::unreachable_blocker_context;
use scene::validate_scene_profiles;

use crate::event_behavior::{node_behavior_for_authoring_node, NodeBehavior, ValidationCtx};

use super::{DiagnosticTarget, LintCode, LintIssue, NodeGraph, StoryNode, ValidationPhase};

pub fn validate(graph: &NodeGraph) -> Vec<LintIssue> {
    validate_no_io(graph)
}

pub fn validate_no_io(graph: &NodeGraph) -> Vec<LintIssue> {
    validate_with_asset_resolver(graph, |_| true)
}

pub fn validate_with_asset_probe<F>(graph: &NodeGraph, asset_exists: F) -> Vec<LintIssue>
where
    F: Fn(&str) -> bool,
{
    validate_with_asset_resolver(graph, asset_exists)
}

pub fn validate_with_asset_resolver<F>(graph: &NodeGraph, asset_exists: F) -> Vec<LintIssue>
where
    F: Fn(&str) -> bool,
{
    validate_with_asset_probe_result(graph, |asset| {
        if asset_exists(asset) {
            Ok(())
        } else {
            Err(assets::AssetProbeError::Missing)
        }
    })
}

fn validate_with_asset_probe_result<F>(graph: &NodeGraph, asset_probe: F) -> Vec<LintIssue>
where
    F: Fn(&str) -> Result<(), assets::AssetProbeError>,
{
    validate_with_asset_probe_and_script(graph, asset_probe, None)
}

fn validate_with_asset_probe_and_script<F>(
    graph: &NodeGraph,
    asset_probe: F,
    script: Option<&crate::script::ScriptRaw>,
) -> Vec<LintIssue>
where
    F: Fn(&str) -> Result<(), assets::AssetProbeError>,
{
    let mut issues = Vec::new();
    let start_nodes = graph
        .nodes()
        .filter_map(|(id, node, _)| matches!(node, StoryNode::Start).then_some(*id))
        .collect::<Vec<_>>();
    match start_nodes.len() {
        0 => issues.push(
            LintIssue::error(
                None,
                ValidationPhase::Graph,
                LintCode::MissingStart,
                "Missing Start node",
            )
            .with_target(DiagnosticTarget::Graph)
            .with_evidence_trace(),
        ),
        1 => {}
        count => issues.push(
            LintIssue::error(
                None,
                ValidationPhase::Graph,
                LintCode::MultipleStart,
                format!("Multiple Start nodes found ({count})"),
            )
            .with_target(DiagnosticTarget::Graph)
            .with_evidence_trace(),
        ),
    }
    let flow = graph.flow_analysis(&start_nodes);
    let script_labels = if graph
        .nodes()
        .any(|(_, node, _)| matches!(node, StoryNode::Jump { .. } | StoryNode::JumpIf { .. }))
    {
        match script {
            Some(script) => script.labels.keys().cloned().collect::<BTreeSet<_>>(),
            None => graph
                .to_script_lossy_for_diagnostics()
                .labels
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
        }
    } else {
        BTreeSet::new()
    };
    for (id, node, position) in graph.nodes() {
        validate_layout_position(*id, position.x, position.y, &mut issues);
        if !flow.reachable.contains(id) {
            let (edge_from, blocked_by) = unreachable_blocker_context(graph, *id, &flow.reachable);
            let mut issue = LintIssue::warning(
                Some(*id),
                ValidationPhase::Graph,
                LintCode::UnreachableNode,
                "Unreachable node",
            )
            .with_blocked_by(blocked_by);
            if let Some(from_id) = edge_from {
                issue = issue.with_edge(Some(from_id), Some(*id));
            }
            issues.push(
                issue
                    .with_target(DiagnosticTarget::Node { node_id: *id })
                    .with_evidence_trace(),
            );
        }
        validate_node(graph, *id, node, &script_labels, &asset_probe, &mut issues);
    }
    validate_scene_profiles(graph, &asset_probe, &mut issues);
    issues.extend(graph.validate_fragments());
    for node_id in flow.reachable_cycle_nodes {
        issues.push(
            LintIssue::warning(
                Some(node_id),
                ValidationPhase::Graph,
                LintCode::PotentialLoop,
                "Potential execution loop detected on reachable route",
            )
            .with_target(DiagnosticTarget::Node { node_id })
            .with_evidence_trace(),
        );
    }
    issues
}

pub fn validate_with_project_root(graph: &NodeGraph, project_root: &Path) -> Vec<LintIssue> {
    validate_with_project_root_and_script(graph, project_root, None)
}

pub(super) fn validate_with_script(
    graph: &NodeGraph,
    project_root: Option<&Path>,
    script: &crate::script::ScriptRaw,
) -> Vec<LintIssue> {
    match project_root {
        Some(root) => validate_with_project_root_and_script(graph, root, Some(script)),
        None => validate_with_asset_probe_and_script(graph, |_| Ok(()), Some(script)),
    }
}

fn validate_with_project_root_and_script(
    graph: &NodeGraph,
    project_root: &Path,
    script: Option<&crate::script::ScriptRaw>,
) -> Vec<LintIssue> {
    let canonical_root = match project_root.canonicalize() {
        Ok(root) => root,
        Err(err) => {
            let mut issues = validate_with_asset_probe_and_script(graph, |_| Ok(()), script);
            issues.push(
                LintIssue::error(
                    None,
                    ValidationPhase::Graph,
                    LintCode::AssetReferenceMissing,
                    format!(
                        "Project root unavailable for asset validation: {} ({err})",
                        project_root.display()
                    ),
                )
                .with_target(DiagnosticTarget::Graph)
                .with_evidence_trace(),
            );
            return issues;
        }
    };
    validate_with_asset_probe_and_script(
        graph,
        |asset| {
            assets::resolve_asset_from_canonical_project_root(&canonical_root, asset).map(|_| ())
        },
        script,
    )
}

fn validate_node<F>(
    graph: &NodeGraph,
    id: u32,
    node: &StoryNode,
    script_labels: &BTreeSet<String>,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> Result<(), assets::AssetProbeError>,
{
    let ctx = ValidationCtx::with_asset_probe(graph, id, script_labels, asset_exists);
    issues.extend(node_behavior_for_authoring_node(node).validate(&ctx, node));
}

fn validate_layout_position(id: u32, x: f32, y: f32, issues: &mut Vec<LintIssue>) {
    if layout_position_is_invalid(x, y) {
        issues.push(
            LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::InvalidLayoutPosition,
                "Node layout position is invalid",
            )
            .with_target(DiagnosticTarget::Node { node_id: id })
            .with_field_path(format!("graph.nodes[{id}].position"))
            .with_evidence_trace(),
        );
    }
}

pub(super) fn layout_position_is_invalid(x: f32, y: f32) -> bool {
    const MAX_AUTHORING_COORD: f32 = 1_000_000.0;
    !x.is_finite()
        || !y.is_finite()
        || x.abs() > MAX_AUTHORING_COORD
        || y.abs() > MAX_AUTHORING_COORD
}

#[cfg(test)]
mod asset_probe_tests {
    use super::*;
    use crate::authoring::AuthoringPosition;
    use std::cell::Cell;

    #[test]
    fn repeated_path_preserves_each_probe_result() {
        let mut graph = NodeGraph::new();
        graph.add_node(StoryNode::Start, AuthoringPosition { x: 0.0, y: 0.0 });
        graph.add_node(
            StoryNode::Scene {
                profile: None,
                background: Some("same.png".into()),
                music: Some("same.png".into()),
                characters: Vec::new(),
            },
            AuthoringPosition { x: 0.0, y: 1.0 },
        );
        let count = Cell::new(0);
        let issues = validate_with_asset_probe_result(&graph, |_| {
            count.set(count.get() + 1);
            if count.get() == 1 {
                Err(assets::AssetProbeError::Missing)
            } else {
                Err(assets::AssetProbeError::NotFile)
            }
        });
        let codes: Vec<_> = issues
            .iter()
            .filter(|issue| issue.asset_path.as_deref() == Some("same.png"))
            .map(|issue| issue.code)
            .collect();
        assert_eq!(
            codes,
            [LintCode::AssetReferenceMissing, LintCode::AssetAccessError]
        );
        assert_eq!(count.get(), 2);
    }
}
