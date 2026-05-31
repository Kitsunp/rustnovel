use super::{AuthoringPosition, LintCode, LintIssue, NodeGraph, StoryNode};
use crate::event_behavior::{
    node_quick_fixes_for_issue, BehaviorQuickFix, BehaviorQuickFixRisk, QuickFixCtx,
};

mod assets;
mod audio;
mod character;
mod display;
mod preconditions;
mod transition;

use std::collections::HashSet;

use preconditions::issue_still_matches;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickFixRisk {
    Safe,
    Review,
}

impl QuickFixRisk {
    pub fn label(self) -> &'static str {
        match self {
            QuickFixRisk::Safe => "SAFE",
            QuickFixRisk::Review => "REVIEW",
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuickFixCandidate {
    pub fix_id: &'static str,
    pub title_es: &'static str,
    pub title_en: &'static str,
    pub preconditions_es: &'static str,
    pub preconditions_en: &'static str,
    pub postconditions_es: &'static str,
    pub postconditions_en: &'static str,
    pub risk: QuickFixRisk,
    pub structural: bool,
}

pub fn suggest_fixes(issue: &LintIssue, graph: &NodeGraph) -> Vec<QuickFixCandidate> {
    let mut candidates = Vec::new();
    let behavior_ctx = QuickFixCtx::new(graph);
    candidates.extend(
        node_quick_fixes_for_issue(&behavior_ctx, issue)
            .into_iter()
            .map(candidate_from_behavior),
    );
    match issue.code {
        LintCode::MissingStart => candidates.push(candidate(
            "graph_add_start",
            "Agregar nodo Start",
            "Add Start node",
            QuickFixRisk::Review,
            true,
        )),
        LintCode::DeadEnd => candidates.push(candidate(
            "node_connect_dead_end_to_end",
            "Conectar a End",
            "Connect to End",
            QuickFixRisk::Review,
            true,
        )),
        _ => {}
    }
    candidates
}

pub fn apply_fix(graph: &mut NodeGraph, issue: &LintIssue, fix_id: &str) -> Result<bool, String> {
    if issue.node_id.is_none() && quick_fix_requires_node_id(fix_id) {
        return Err(format!("quick-fix {fix_id} requires node_id"));
    }
    if !suggest_fixes(issue, graph)
        .iter()
        .any(|candidate| candidate.fix_id == fix_id)
    {
        return Err(format!(
            "stale quick-fix {fix_id}: issue {} no longer matches graph",
            issue.diagnostic_id()
        ));
    }
    if (issue.node_id.is_some() || issue.code == LintCode::MissingStart)
        && !issue_still_matches(issue, graph)
    {
        return Err(format!(
            "stale quick-fix {fix_id}: issue {} no longer matches graph",
            issue.diagnostic_id()
        ));
    }

    match fix_id {
        "graph_add_start" => Ok(add_start(graph)),
        "node_connect_dead_end_to_end" => connect_dead_end(graph, require_node(issue)?),
        "choice_add_default_option" => add_choice_option(graph, require_node(issue)?),
        "choice_add_default_option_to_end" => add_choice_option_to_end(graph, require_node(issue)?),
        "choice_link_unlinked_to_end" => link_unlinked_choice_options(graph, require_node(issue)?),
        "choice_expand_options_to_ports" => {
            expand_choice_options_to_ports(graph, require_node(issue)?)
        }
        "dialogue_fill_speaker" => fill_speaker(graph, require_node(issue)?),
        "jump_set_existing_target" => set_jump_target_existing(graph, require_node(issue)?),
        "transition_set_fade" => transition::set_kind(graph, require_node(issue)?),
        "transition_set_default_duration" => transition::set_duration(graph, require_node(issue)?),
        "audio_normalize_channel" => audio::normalize_channel(graph, require_node(issue)?),
        "audio_normalize_action" => audio::normalize_action(graph, require_node(issue)?),
        "audio_clamp_volume" => audio::clamp_volume(graph, require_node(issue)?),
        "audio_set_default_fade" => audio::set_default_fade(graph, require_node(issue)?),
        "scene_clear_empty_background" => assets::clear_empty_scene_background(graph, issue),
        "scene_clear_empty_music" => assets::clear_empty_scene_music(graph, issue),
        "audio_clear_empty_asset" => assets::clear_empty_audio_asset(graph, issue),
        "audio_missing_asset_to_stop" => assets::audio_missing_asset_to_stop(graph, issue),
        "clear_missing_asset_reference" | "clear_unsafe_asset_reference" => {
            assets::clear_asset_reference(graph, issue)
        }
        "character_prune_or_fill_invalid_names" => {
            character::fix_names(graph, require_node(issue)?)
        }
        "character_set_default_scale" => character::set_scale(graph, require_node(issue)?),
        other => Err(format!("unknown quick-fix id {other}")),
    }
}

fn quick_fix_requires_node_id(fix_id: &str) -> bool {
    matches!(
        fix_id,
        "node_connect_dead_end_to_end"
            | "choice_add_default_option"
            | "choice_add_default_option_to_end"
            | "choice_link_unlinked_to_end"
            | "choice_expand_options_to_ports"
            | "dialogue_fill_speaker"
            | "jump_set_existing_target"
            | "transition_set_fade"
            | "transition_set_default_duration"
            | "audio_normalize_channel"
            | "audio_normalize_action"
            | "audio_clamp_volume"
            | "audio_set_default_fade"
            | "scene_clear_empty_background"
            | "scene_clear_empty_music"
            | "audio_clear_empty_asset"
            | "audio_missing_asset_to_stop"
            | "clear_missing_asset_reference"
            | "clear_unsafe_asset_reference"
            | "character_prune_or_fill_invalid_names"
            | "character_set_default_scale"
    )
}

fn candidate(
    fix_id: &'static str,
    title_es: &'static str,
    title_en: &'static str,
    risk: QuickFixRisk,
    structural: bool,
) -> QuickFixCandidate {
    QuickFixCandidate {
        fix_id,
        title_es,
        title_en,
        preconditions_es: "El issue sigue presente en el grafo.",
        preconditions_en: "The issue is still present in the graph.",
        postconditions_es: "El grafo se actualiza de forma deterministica.",
        postconditions_en: "The graph is updated deterministically.",
        risk,
        structural,
    }
}

fn candidate_from_behavior(fix: BehaviorQuickFix) -> QuickFixCandidate {
    candidate(
        fix.fix_id,
        fix.title_es,
        fix.title_en,
        match fix.risk {
            BehaviorQuickFixRisk::Safe => QuickFixRisk::Safe,
            BehaviorQuickFixRisk::Review => QuickFixRisk::Review,
        },
        fix.structural,
    )
}

fn require_node(issue: &LintIssue) -> Result<u32, String> {
    issue
        .node_id
        .ok_or_else(|| format!("quick-fix {} requires node_id", issue.code.label()))
}

fn add_start(graph: &mut NodeGraph) -> bool {
    if graph
        .nodes()
        .any(|(_, node, _)| matches!(node, StoryNode::Start))
    {
        return false;
    }
    graph.add_node(StoryNode::Start, AuthoringPosition::new(50.0, 30.0));
    true
}

fn ensure_end(graph: &mut NodeGraph, source: u32) -> Result<u32, String> {
    if let Some((id, _, _)) = graph
        .nodes()
        .find(|(_, node, _)| matches!(node, StoryNode::End))
        .cloned()
    {
        return Ok(id);
    }
    let pos = graph
        .get_node_pos(source)
        .ok_or_else(|| format!("source node {source} not found"))?;
    Ok(graph.add_node(
        StoryNode::End,
        AuthoringPosition::new(pos.x + 140.0, pos.y + 120.0),
    ))
}

fn connect_dead_end(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    if graph.connections().any(|conn| conn.from == node_id) {
        return Ok(false);
    }
    let end = ensure_end(graph, node_id)?;
    graph.connect(node_id, end);
    Ok(true)
}

fn add_choice_option(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::Choice { options, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Choice"));
    };
    if !options.is_empty() {
        return Ok(false);
    }
    options.push("Option 1".to_string());
    graph.mark_modified();
    Ok(true)
}

fn add_choice_option_to_end(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let added = add_choice_option(graph, node_id)?;
    let end = ensure_end(graph, node_id)?;
    graph.connect_port(node_id, 0, end);
    Ok(added
        || graph
            .connections()
            .any(|conn| conn.from == node_id && conn.from_port == 0 && conn.to == end))
}

fn link_unlinked_choice_options(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let options_len = match graph.get_node(node_id) {
        Some(StoryNode::Choice { options, .. }) => options.len(),
        _ => return Err(format!("node_id {node_id} is not Choice")),
    };
    let linked = graph
        .connections()
        .filter(|conn| conn.from == node_id)
        .map(|conn| conn.from_port)
        .collect::<HashSet<_>>();
    let unlinked = (0..options_len)
        .filter(|idx| !linked.contains(idx))
        .collect::<Vec<_>>();
    if unlinked.is_empty() {
        return Ok(false);
    }
    let end = ensure_end(graph, node_id)?;
    for port in unlinked {
        graph.connect_port(node_id, port, end);
    }
    Ok(true)
}

fn expand_choice_options_to_ports(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::Choice { options, .. }) = graph.get_node(node_id) else {
        return Err(format!("node_id {node_id} is not Choice"));
    };
    let before = options.len();
    let max_port = graph
        .connections()
        .filter(|conn| conn.from == node_id)
        .map(|conn| conn.from_port)
        .max()
        .unwrap_or(0);
    if max_port < before {
        return Ok(false);
    }

    let Some(StoryNode::Choice { options, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Choice"));
    };
    while options.len() <= max_port {
        let next = options.len() + 1;
        options.push(format!("Option {next}"));
    }
    graph.mark_modified();
    Ok(true)
}

fn fill_speaker(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::Dialogue { speaker, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Dialogue"));
    };
    if !speaker.trim().is_empty() {
        return Ok(false);
    }
    *speaker = "Narrator".to_string();
    graph.mark_modified();
    Ok(true)
}

fn set_jump_target_existing(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let target_label = existing_jump_target(graph)
        .ok_or_else(|| "no existing target label available for empty jump".to_string())?;
    let Some(node) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} not found"));
    };
    match node {
        StoryNode::Jump { target } | StoryNode::JumpIf { target, .. }
            if target.trim().is_empty() =>
        {
            *target = target_label;
            graph.mark_modified();
            Ok(true)
        }
        StoryNode::Jump { .. } | StoryNode::JumpIf { .. } => Ok(false),
        _ => Err(format!("node_id {node_id} is not Jump/JumpIf")),
    }
}

fn existing_jump_target(graph: &NodeGraph) -> Option<String> {
    if graph
        .nodes()
        .any(|(_, node, _)| matches!(node, StoryNode::Start))
    {
        return Some("start".to_string());
    }
    graph
        .nodes()
        .find(|(_, node, _)| !node.is_marker())
        .map(|(id, _, _)| format!("node_{id}"))
}
