use super::*;

pub fn validate_authoring_node(ctx: &ValidationCtx<'_>, node: &StoryNode) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let id = ctx.node_id();
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
                    field_path: Some(FieldPath::new(format!("graph.nodes[{id}].speaker"))),
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
        StoryNode::Choice { options, .. } => validate_choice(ctx.graph(), id, options, &mut issues),
        StoryNode::Scene {
            profile,
            background,
            music,
            characters,
        } => {
            if let Some(profile) = profile {
                if ctx.graph().scene_profile(profile).is_none() {
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
            let asset_exists = |asset: &str| ctx.asset_exists(asset);
            validate_scene(
                id,
                background,
                music,
                characters,
                &asset_exists,
                &mut issues,
            );
        }
        StoryNode::ScenePatch(patch) => {
            let asset_exists = |asset: &str| ctx.asset_exists(asset);
            validate_scene_patch(id, patch, &asset_exists, &mut issues);
        }
        StoryNode::Jump { target } if !has_exportable_connected_target(ctx.graph(), id) => {
            validate_jump_target(id, target, ctx.script_labels(), &mut issues);
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
            if !has_exportable_connected_target(ctx.graph(), id) {
                validate_jump_target(id, target, ctx.script_labels(), &mut issues);
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
        } => {
            let asset_exists = |asset: &str| ctx.asset_exists(asset);
            validate_audio(
                AudioValidation {
                    id,
                    channel,
                    action,
                    asset,
                    volume,
                    fade_duration_ms,
                },
                &asset_exists,
                &mut issues,
            );
        }
        StoryNode::Transition {
            kind, duration_ms, ..
        } => validate_transition(id, kind, *duration_ms, &mut issues),
        StoryNode::CharacterPlacement { name, scale, .. } => {
            validate_character(id, name, scale, &mut issues)
        }
        StoryNode::SubgraphCall { .. } => {}
        StoryNode::Generic(event) => validate_generic_node(id, event, &mut issues),
        _ => {}
    }

    if !matches!(node, StoryNode::End) && !ctx.graph().connections().any(|conn| conn.from == id) {
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
    issues
}

pub fn node_quick_fixes_for_issue(
    ctx: &QuickFixCtx<'_>,
    issue: &LintIssue,
) -> Vec<BehaviorQuickFix> {
    let Some(node_id) = issue.node_id else {
        return Vec::new();
    };
    let Some(node) = ctx.graph().get_node(node_id) else {
        return Vec::new();
    };
    node_behavior_for_authoring_node(node).quick_fixes(ctx, issue)
}

fn validate_generic_node(id: u32, event: &EventRaw, issues: &mut Vec<LintIssue>) {
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
                    field_path: Some(FieldPath::new(format!("graph.nodes[{id}].generic"))),
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

fn has_exportable_connected_target(graph: &NodeGraph, id: u32) -> bool {
    graph.connections().any(|conn| {
        conn.from == id
            && conn.from_port == 0
            && graph
                .get_node(conn.to)
                .is_some_and(|node| node.is_marker() || node.export_supported())
    })
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
        .collect::<Vec<_>>();
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
