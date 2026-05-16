use std::collections::BTreeSet;
use std::path::Path;

mod assets;
mod event_details;
mod flow;
mod scene;
mod trace;

pub use assets::{
    asset_exists_from_project_root, default_asset_exists, is_unsafe_asset_ref,
    should_probe_asset_exists,
};
use event_details::{validate_audio, validate_character, validate_transition, AudioValidation};
use flow::unreachable_blocker_context;
use scene::{validate_scene, validate_scene_patch, validate_scene_profiles};
use trace::parse_import_trace_context;

use crate::{CondRaw, EventRaw};

use super::{
    DiagnosticTarget, GraphConnection, LintCode, LintIssue, NodeGraph, SemanticValue,
    SemanticValueKind, StoryNode, ValidationPhase,
};

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
    let script = graph.to_script_lossy_for_diagnostics();
    let script_labels = script.labels.keys().cloned().collect::<BTreeSet<_>>();
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
        validate_node(graph, *id, node, &script_labels, &asset_exists, &mut issues);
    }
    validate_scene_profiles(graph, &asset_exists, &mut issues);
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
    validate_with_asset_resolver(graph, |asset| {
        asset_exists_from_project_root(project_root, asset)
    })
}

fn validate_node<F>(
    graph: &NodeGraph,
    id: u32,
    node: &StoryNode,
    script_labels: &BTreeSet<String>,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    if !node.is_marker() && !node.export_supported() {
        issues.push(
            LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::ContractUnsupportedExport,
                "Node is not export-compatible",
            )
            .with_target(DiagnosticTarget::Node { node_id: id })
            .with_field_path(format!("graph.nodes[{id}]"))
            .with_evidence_trace(),
        );
    }
    match node {
        StoryNode::Dialogue { speaker, .. } if speaker.trim().is_empty() => {
            issues.push(
                LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::EmptySpeakerName,
                    "Dialogue speaker is empty",
                )
                .with_target(DiagnosticTarget::Character {
                    node_id: Some(id),
                    name: speaker.clone(),
                    field_path: Some(super::FieldPath::new(format!("graph.nodes[{id}].speaker"))),
                })
                .with_field_path(format!("graph.nodes[{id}].speaker"))
                .with_semantic_value(SemanticValue::new(
                    SemanticValueKind::CharacterRef,
                    speaker.clone(),
                    format!("graph.nodes[{id}].speaker"),
                ))
                .with_evidence_trace(),
            );
        }
        StoryNode::Choice { options, .. } => validate_choice(graph, id, options, issues),
        StoryNode::Scene {
            profile,
            background,
            music,
            characters,
        } => {
            if let Some(profile) = profile {
                if graph.scene_profile(profile).is_none() {
                    issues.push(
                        LintIssue::error(
                            Some(id),
                            ValidationPhase::Graph,
                            LintCode::AssetReferenceMissing,
                            "Scene profile does not exist",
                        )
                        .with_asset_path(Some(profile.clone()))
                        .with_target(DiagnosticTarget::SceneProfile {
                            profile_id: profile.clone(),
                        })
                        .with_field_path(format!("graph.nodes[{id}].profile"))
                        .with_semantic_value(SemanticValue::new(
                            SemanticValueKind::AssetRef,
                            profile.clone(),
                            format!("graph.nodes[{id}].profile"),
                        ))
                        .with_evidence_trace(),
                    );
                }
            }
            validate_scene(id, background, music, characters, asset_exists, issues)
        }
        StoryNode::ScenePatch(patch) => {
            validate_scene_patch(id, patch, asset_exists, issues);
        }
        StoryNode::Jump { target } => {
            validate_jump_target(id, target, script_labels, issues);
        }
        StoryNode::JumpIf { target, cond } => {
            if cond_key_empty(cond) {
                issues.push(
                    LintIssue::error(
                        Some(id),
                        ValidationPhase::Graph,
                        LintCode::EmptyStateKey,
                        "JumpIf condition key is empty",
                    )
                    .with_target(DiagnosticTarget::JumpTarget {
                        node_id: id,
                        target: target.clone(),
                    })
                    .with_field_path(format!("graph.nodes[{id}].cond.key"))
                    .with_evidence_trace(),
                );
            }
            let has_connected_target = graph
                .connections()
                .any(|conn| conn.from == id && conn.from_port == 0);
            if !has_connected_target {
                validate_jump_target(id, target, script_labels, issues);
            }
        }
        StoryNode::SetVariable { key, .. } | StoryNode::SetFlag { key, .. }
            if key.trim().is_empty() =>
        {
            issues.push(
                LintIssue::error(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::EmptyStateKey,
                    "State key is empty",
                )
                .with_field_path(format!("graph.nodes[{id}].key"))
                .with_semantic_value(SemanticValue::new(
                    SemanticValueKind::VariableRef,
                    key.clone(),
                    format!("graph.nodes[{id}].key"),
                ))
                .with_evidence_trace(),
            );
        }
        StoryNode::AudioAction {
            channel,
            action,
            asset,
            volume,
            fade_duration_ms,
            ..
        } => validate_audio(
            AudioValidation {
                id,
                channel,
                action,
                asset,
                volume,
                fade_duration_ms,
            },
            asset_exists,
            issues,
        ),
        StoryNode::Transition {
            kind, duration_ms, ..
        } => validate_transition(id, kind, *duration_ms, issues),
        StoryNode::CharacterPlacement { name, scale, .. } => {
            validate_character(id, name, scale, issues)
        }
        StoryNode::SubgraphCall { .. } => {}
        StoryNode::Generic(event) => {
            let mut issue = LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::GenericEventUnchecked,
                "Generic event has limited semantic validation",
            );
            if let EventRaw::ExtCall { command, args } = event {
                if let Some(trace) = parse_import_trace_context(args) {
                    let ip_segment = trace
                        .event_ip
                        .map(|ip| format!(" ip={ip}"))
                        .unwrap_or_default();
                    let snippet_segment = trace
                        .snippet
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                        .map(|value| format!(" snippet='{}'", value.trim()))
                        .unwrap_or_default();
                    issue.message = format!(
                        "Import fallback extcall '{}' requires review (trace_id={}, code={}, source={}, area={}, phase={}{}{})",
                        command,
                        trace.trace_id,
                        trace.issue_code,
                        trace.source_command,
                        trace.area,
                        trace.phase,
                        ip_segment,
                        snippet_segment
                    );
                    issue = issue
                        .with_blocked_by(trace.blocked_by)
                        .with_target(DiagnosticTarget::Generic {
                            field_path: Some(super::FieldPath::new(format!(
                                "graph.nodes[{id}].generic"
                            ))),
                        })
                        .with_field_path(format!("graph.nodes[{id}].generic"))
                        .with_semantic_value(SemanticValue::new(
                            SemanticValueKind::PluginRef,
                            command.clone(),
                            format!("graph.nodes[{id}].generic.command"),
                        ))
                        .with_evidence_trace();
                }
            }
            issues.push(issue);
        }
        _ => {}
    }
    if !matches!(node, StoryNode::End) && !graph.connections().any(|conn| conn.from == id) {
        issues.push(
            LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::DeadEnd,
                "Node has no outgoing transition",
            )
            .with_target(DiagnosticTarget::Node { node_id: id })
            .with_evidence_trace(),
        );
    }
}

fn validate_layout_position(id: u32, x: f32, y: f32, issues: &mut Vec<LintIssue>) {
    const MAX_AUTHORING_COORD: f32 = 1_000_000.0;
    if !x.is_finite()
        || !y.is_finite()
        || x.abs() > MAX_AUTHORING_COORD
        || y.abs() > MAX_AUTHORING_COORD
    {
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

fn validate_jump_target(
    id: u32,
    target: &str,
    script_labels: &BTreeSet<String>,
    issues: &mut Vec<LintIssue>,
) {
    let target = target.trim();
    if target.is_empty() {
        issues.push(
            LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::EmptyJumpTarget,
                "Jump target is empty",
            )
            .with_target(DiagnosticTarget::JumpTarget {
                node_id: id,
                target: target.to_string(),
            })
            .with_field_path(format!("graph.nodes[{id}].target"))
            .with_semantic_value(SemanticValue::new(
                SemanticValueKind::LabelRef,
                target,
                format!("graph.nodes[{id}].target"),
            ))
            .with_evidence_trace(),
        );
    } else if !script_labels.contains(target) {
        issues.push(
            LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::MissingJumpTarget,
                format!("Jump target '{target}' does not exist"),
            )
            .with_target(DiagnosticTarget::JumpTarget {
                node_id: id,
                target: target.to_string(),
            })
            .with_field_path(format!("graph.nodes[{id}].target"))
            .with_semantic_value(SemanticValue::new(
                SemanticValueKind::LabelRef,
                target,
                format!("graph.nodes[{id}].target"),
            ))
            .with_evidence_trace(),
        );
    }
}

fn cond_key_empty(cond: &CondRaw) -> bool {
    match cond {
        CondRaw::Flag { key, .. } | CondRaw::VarCmp { key, .. } => key.trim().is_empty(),
    }
}

fn validate_choice(graph: &NodeGraph, id: u32, options: &[String], issues: &mut Vec<LintIssue>) {
    if options.is_empty() {
        issues.push(
            LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::ChoiceNoOptions,
                "Choice has no options",
            )
            .with_target(DiagnosticTarget::Node { node_id: id })
            .with_field_path(format!("graph.nodes[{id}].options"))
            .with_evidence_trace(),
        );
    }
    for (idx, option) in options.iter().enumerate() {
        if is_placeholder_option(option, idx) {
            issues.push(
                LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::PlaceholderChoiceOption,
                    format!("Choice option {idx} still uses placeholder text"),
                )
                .with_target(DiagnosticTarget::ChoiceOption {
                    node_id: id,
                    option_index: idx,
                })
                .with_field_path(format!("graph.nodes[{id}].options[{idx}].text"))
                .with_semantic_value(SemanticValue::new(
                    SemanticValueKind::Text,
                    option.clone(),
                    format!("graph.nodes[{id}].options[{idx}].text"),
                ))
                .with_evidence_trace(),
            );
        }
    }
    let outgoing = graph
        .connections()
        .filter(|conn| conn.from == id)
        .collect::<Vec<&GraphConnection>>();
    for idx in 0..options.len() {
        if !outgoing.iter().any(|conn| conn.from_port == idx) {
            issues.push(
                LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::ChoiceOptionUnlinked,
                    format!("Choice option {idx} is unlinked"),
                )
                .with_edge(Some(id), None)
                .with_target(DiagnosticTarget::ChoiceOption {
                    node_id: id,
                    option_index: idx,
                })
                .with_field_path(format!("graph.nodes[{id}].options[{idx}].target"))
                .with_evidence_trace(),
            );
        }
    }
    for conn in outgoing {
        if conn.from_port >= options.len() {
            issues.push(
                LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::ChoicePortOutOfRange,
                    "Choice connection port is out of range",
                )
                .with_edge(Some(conn.from), Some(conn.to))
                .with_target(DiagnosticTarget::Edge {
                    from: conn.from,
                    from_port: conn.from_port,
                    to: Some(conn.to),
                })
                .with_evidence_trace(),
            );
        }
    }
}

fn is_placeholder_option(option: &str, index: usize) -> bool {
    option.trim() == format!("Option {}", index + 1)
}
