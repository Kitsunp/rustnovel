use super::super::context::{non_exportable_event_name, parse_import_trace_context};
use super::super::helpers::{
    has_outgoing, is_unsafe_asset_ref, is_valid_audio_action, is_valid_audio_channel,
    is_valid_transition_kind, should_probe_asset_exists,
};
use super::super::{LintCode, LintIssue, ValidationPhase};
use crate::editor::{execution_contract, NodeGraph, StoryNode};

pub(super) fn validate_node<F>(
    graph: &NodeGraph,
    id: u32,
    node: &StoryNode,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    let contract = execution_contract::contract_for_node(node);
    if !node.is_marker() && !contract.export_supported {
        let event_name = non_exportable_event_name(node, contract.event_name);
        issues.push(LintIssue::error(
            Some(id),
            ValidationPhase::Graph,
            LintCode::ContractUnsupportedExport,
            format!(
                "Event '{}' is not export-compatible (contract mismatch)",
                event_name
            ),
        ));
    }

    match node {
        StoryNode::Dialogue { speaker, .. } => {
            if speaker.trim().is_empty() {
                issues.push(LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::EmptySpeakerName,
                    "Dialogue speaker is empty",
                ));
            }
        }
        StoryNode::Scene {
            background,
            music,
            characters,
            ..
        } => {
            validate_asset_path(id, background, "background", asset_exists, issues);
            validate_asset_path(id, music, "music", asset_exists, issues);
            if characters.iter().any(|c| c.name.trim().is_empty()) {
                issues.push(LintIssue::error(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::EmptyCharacterName,
                    "Scene has character entry with empty name",
                ));
            }
        }
        StoryNode::SetVariable { .. } => {}
        StoryNode::ScenePatch(patch) => {
            validate_patch_asset_path(
                id,
                patch.background.as_ref(),
                "scene patch background",
                asset_exists,
                issues,
            );
            validate_patch_asset_path(
                id,
                patch.music.as_ref(),
                "scene patch music",
                asset_exists,
                issues,
            );

            if patch.add.iter().any(|c| c.name.trim().is_empty()) {
                issues.push(LintIssue::error(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::EmptyCharacterName,
                    "Scene patch has add-entry with empty character name",
                ));
            }
            if patch.update.iter().any(|c| c.name.trim().is_empty()) {
                issues.push(LintIssue::error(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::EmptyCharacterName,
                    "Scene patch has update-entry with empty character name",
                ));
            }
            if patch.remove.iter().any(|name| name.trim().is_empty()) {
                issues.push(LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::EmptyCharacterName,
                    "Scene patch has empty character name in remove-list",
                ));
            }
        }
        StoryNode::Generic(_) => validate_generic_node(id, node, issues),
        StoryNode::Transition {
            kind, duration_ms, ..
        } => {
            if *duration_ms == 0 {
                issues.push(LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::InvalidTransitionDuration,
                    "Transition duration should be > 0 ms",
                ));
            }
            if !is_valid_transition_kind(kind) {
                issues.push(LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::InvalidTransitionKind,
                    format!("Transition kind '{}' is not recognized", kind),
                ));
            }
        }
        StoryNode::Jump { target } | StoryNode::JumpIf { target, .. } => {
            if target.trim().is_empty() {
                issues.push(LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::EmptyJumpTarget,
                    "Jump target is empty",
                ));
            }
        }
        StoryNode::Choice { options, .. } => validate_choice_node(graph, id, options, issues),
        StoryNode::AudioAction {
            channel,
            action,
            asset,
            volume,
            fade_duration_ms,
            ..
        } => validate_audio_node(
            id,
            channel,
            action,
            asset,
            volume,
            fade_duration_ms,
            asset_exists,
            issues,
        ),
        StoryNode::CharacterPlacement { name, scale, .. } => {
            if name.trim().is_empty() {
                issues.push(LintIssue::error(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::EmptyCharacterName,
                    "Character name cannot be empty",
                ));
            }
            if let Some(scale) = scale {
                if !scale.is_finite() || *scale <= 0.0 {
                    issues.push(LintIssue::error(
                        Some(id),
                        ValidationPhase::Graph,
                        LintCode::InvalidCharacterScale,
                        "Character scale must be finite and > 0",
                    ));
                }
            }
        }
        StoryNode::Start | StoryNode::End => {}
    }

    if !matches!(node, StoryNode::End) && !has_outgoing(graph, id) {
        issues.push(LintIssue::warning(
            Some(id),
            ValidationPhase::Graph,
            LintCode::DeadEnd,
            "Node has no outgoing transition",
        ));
    }
}

fn validate_asset_path<F>(
    id: u32,
    value: &Option<String>,
    label: &str,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    let Some(path) = value else {
        return;
    };
    if path.trim().is_empty() {
        issues.push(LintIssue::warning(
            Some(id),
            ValidationPhase::Graph,
            if label == "background" {
                LintCode::SceneBackgroundEmpty
            } else {
                LintCode::AudioAssetEmpty
            },
            if label == "background" {
                "Scene background path is empty"
            } else {
                "Scene music path is empty"
            },
        ));
    } else {
        validate_patch_asset_path(id, Some(path), label, asset_exists, issues);
    }
}

fn validate_patch_asset_path<F>(
    id: u32,
    value: Option<&String>,
    label: &str,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    let Some(path) = value else {
        return;
    };
    if is_unsafe_asset_ref(path) {
        issues.push(
            LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::UnsafeAssetPath,
                format!("Unsafe {} path: '{}'", label, path),
            )
            .with_asset_path(Some(path.clone())),
        );
    } else if should_probe_asset_exists(path) && !asset_exists(path) {
        issues.push(
            LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::AssetReferenceMissing,
                format!("{} asset does not exist: '{}'", capitalize(label), path),
            )
            .with_asset_path(Some(path.clone())),
        );
    }
}

fn validate_generic_node(id: u32, node: &StoryNode, issues: &mut Vec<LintIssue>) {
    let mut issue = LintIssue::warning(
        Some(id),
        ValidationPhase::Graph,
        LintCode::GenericEventUnchecked,
        "Generic event has limited semantic validation",
    );
    if let StoryNode::Generic(visual_novel_engine::EventRaw::ExtCall { command, args }) = node {
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
            issue = issue.with_blocked_by(trace.blocked_by);
        } else {
            issue.message = format!(
                "ExtCall '{}' has no structured trace envelope for semantic validation",
                command
            );
        }
    }
    issues.push(issue);
}

fn validate_choice_node(
    graph: &NodeGraph,
    id: u32,
    options: &[String],
    issues: &mut Vec<LintIssue>,
) {
    if options.is_empty() {
        issues.push(LintIssue::error(
            Some(id),
            ValidationPhase::Graph,
            LintCode::ChoiceNoOptions,
            "Choice node has no options",
        ));
    }

    for (idx, _) in options.iter().enumerate() {
        if !graph
            .connections
            .iter()
            .any(|c| c.from == id && c.from_port == idx)
        {
            issues.push(
                LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::ChoiceOptionUnlinked,
                    format!("Choice option {} has no outgoing connection", idx + 1),
                )
                .with_edge(Some(id), None),
            );
        }
    }

    for conn in graph.connections.iter().filter(|c| c.from == id) {
        if conn.from_port >= options.len() {
            issues.push(
                LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::ChoicePortOutOfRange,
                    format!(
                        "Connection from invalid option port {} (options: {})",
                        conn.from_port,
                        options.len()
                    ),
                )
                .with_edge(Some(conn.from), Some(conn.to)),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_audio_node<F>(
    id: u32,
    channel: &str,
    action: &str,
    asset: &Option<String>,
    volume: &Option<f32>,
    fade_duration_ms: &Option<u64>,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    let normalized_channel = channel.trim().to_ascii_lowercase();
    let normalized_action = action.trim().to_ascii_lowercase();

    if !is_valid_audio_channel(&normalized_channel) {
        issues.push(LintIssue::error(
            Some(id),
            ValidationPhase::Graph,
            LintCode::InvalidAudioChannel,
            format!("Invalid audio channel '{}'", channel),
        ));
    }
    if !is_valid_audio_action(&normalized_action) {
        issues.push(LintIssue::error(
            Some(id),
            ValidationPhase::Graph,
            LintCode::InvalidAudioAction,
            format!("Invalid audio action '{}'", action),
        ));
    }
    if let Some(value) = volume {
        if !value.is_finite() || !(0.0..=1.0).contains(value) {
            issues.push(LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::InvalidAudioVolume,
                "Audio volume must be finite and in range [0.0, 1.0]",
            ));
        }
    }
    if let Some(duration) = fade_duration_ms {
        if *duration == 0 && matches!(normalized_action.as_str(), "stop" | "fade_out") {
            issues.push(LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::InvalidAudioFade,
                "Fade duration should be > 0 ms for stop/fade_out",
            ));
        }
    }

    if normalized_action == "play" {
        match asset {
            None => issues.push(LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::AudioAssetMissing,
                "Audio asset path is missing",
            )),
            Some(path) if path.trim().is_empty() => issues.push(LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::AudioAssetEmpty,
                "Audio asset path is empty",
            )),
            Some(path) if is_unsafe_asset_ref(path) => issues.push(
                LintIssue::error(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::UnsafeAssetPath,
                    format!("Unsafe audio asset path: '{}'", path),
                )
                .with_asset_path(Some(path.clone())),
            ),
            Some(path) if should_probe_asset_exists(path) && !asset_exists(path) => issues.push(
                LintIssue::error(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::AssetReferenceMissing,
                    format!("Audio asset does not exist: '{}'", path),
                )
                .with_asset_path(Some(path.clone())),
            ),
            Some(_) => {}
        }
    }
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
