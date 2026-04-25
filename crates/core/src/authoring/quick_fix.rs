use super::{AuthoringPosition, LintCode, LintIssue, NodeGraph, StoryNode};

mod assets;
mod display;

use std::collections::HashSet;

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
    let Some(candidate) = (match issue.code {
        LintCode::MissingStart => Some(candidate(
            "graph_add_start",
            "Agregar nodo Start",
            "Add Start node",
            QuickFixRisk::Review,
            true,
        )),
        LintCode::DeadEnd => Some(candidate(
            "node_connect_dead_end_to_end",
            "Conectar a End",
            "Connect to End",
            QuickFixRisk::Review,
            true,
        )),
        LintCode::ChoiceNoOptions => Some(candidate(
            "choice_add_default_option",
            "Agregar opcion",
            "Add option",
            QuickFixRisk::Safe,
            false,
        )),
        LintCode::ChoiceOptionUnlinked => Some(candidate(
            "choice_link_unlinked_to_end",
            "Conectar opciones sin salida",
            "Connect unlinked options",
            QuickFixRisk::Review,
            true,
        )),
        LintCode::ChoicePortOutOfRange => Some(candidate(
            "choice_expand_options_to_ports",
            "Sincronizar opciones con puertos",
            "Sync options with ports",
            QuickFixRisk::Safe,
            false,
        )),
        LintCode::EmptySpeakerName => Some(candidate(
            "dialogue_fill_speaker",
            "Rellenar speaker",
            "Fill speaker",
            QuickFixRisk::Safe,
            false,
        )),
        LintCode::EmptyJumpTarget => Some(candidate(
            "jump_set_start_target",
            "Usar start como destino",
            "Use start as target",
            QuickFixRisk::Review,
            false,
        )),
        LintCode::InvalidTransitionKind => Some(candidate(
            "transition_set_fade",
            "Usar fade",
            "Use fade",
            QuickFixRisk::Safe,
            false,
        )),
        LintCode::InvalidTransitionDuration => Some(candidate(
            "transition_set_default_duration",
            "Usar duracion por defecto",
            "Use default duration",
            QuickFixRisk::Safe,
            false,
        )),
        LintCode::InvalidAudioChannel => Some(candidate(
            "audio_normalize_channel",
            "Normalizar canal",
            "Normalize channel",
            QuickFixRisk::Safe,
            false,
        )),
        LintCode::InvalidAudioAction => Some(candidate(
            "audio_normalize_action",
            "Normalizar accion",
            "Normalize action",
            QuickFixRisk::Safe,
            false,
        )),
        LintCode::InvalidAudioVolume => Some(candidate(
            "audio_clamp_volume",
            "Ajustar volumen",
            "Clamp volume",
            QuickFixRisk::Safe,
            false,
        )),
        LintCode::InvalidAudioFade => Some(candidate(
            "audio_set_default_fade",
            "Usar fade por defecto",
            "Use default fade",
            QuickFixRisk::Safe,
            false,
        )),
        LintCode::SceneBackgroundEmpty => Some(candidate(
            "scene_clear_empty_background",
            "Limpiar background vacio",
            "Clear empty background",
            QuickFixRisk::Safe,
            false,
        )),
        LintCode::AudioAssetEmpty => assets::empty_asset_candidate(issue, graph),
        LintCode::AudioAssetMissing => assets::missing_audio_candidate(issue, graph),
        LintCode::AssetReferenceMissing => assets::clear_asset_candidate(
            issue,
            graph,
            "clear_missing_asset_reference",
            "Limpiar asset inexistente",
            "Clear missing asset",
        ),
        LintCode::UnsafeAssetPath => assets::clear_asset_candidate(
            issue,
            graph,
            "clear_unsafe_asset_reference",
            "Limpiar asset inseguro",
            "Clear unsafe asset",
        ),
        LintCode::EmptyCharacterName => Some(candidate(
            "character_prune_or_fill_invalid_names",
            "Corregir nombres vacios",
            "Fix empty names",
            QuickFixRisk::Safe,
            false,
        )),
        LintCode::InvalidCharacterScale => Some(candidate(
            "character_set_default_scale",
            "Usar escala por defecto",
            "Use default scale",
            QuickFixRisk::Safe,
            false,
        )),
        _ => None,
    }) else {
        return Vec::new();
    };
    vec![candidate]
}

pub fn apply_fix(graph: &mut NodeGraph, issue: &LintIssue, fix_id: &str) -> Result<bool, String> {
    match fix_id {
        "graph_add_start" => Ok(add_start(graph)),
        "node_connect_dead_end_to_end" => connect_dead_end(graph, require_node(issue)?),
        "choice_add_default_option" => add_choice_option(graph, require_node(issue)?),
        "choice_link_unlinked_to_end" => link_unlinked_choice_options(graph, require_node(issue)?),
        "choice_expand_options_to_ports" => {
            expand_choice_options_to_ports(graph, require_node(issue)?)
        }
        "dialogue_fill_speaker" => fill_speaker(graph, require_node(issue)?),
        "jump_set_start_target" => set_jump_target_start(graph, require_node(issue)?),
        "transition_set_fade" => set_transition_kind(graph, require_node(issue)?),
        "transition_set_default_duration" => set_transition_duration(graph, require_node(issue)?),
        "audio_normalize_channel" => normalize_audio_channel(graph, require_node(issue)?),
        "audio_normalize_action" => normalize_audio_action(graph, require_node(issue)?),
        "audio_clamp_volume" => clamp_audio_volume(graph, require_node(issue)?),
        "audio_set_default_fade" => set_audio_fade(graph, require_node(issue)?),
        "scene_clear_empty_background" => assets::clear_empty_scene_background(graph, issue),
        "scene_clear_empty_music" => assets::clear_empty_scene_music(graph, issue),
        "audio_clear_empty_asset" => assets::clear_empty_audio_asset(graph, issue),
        "audio_missing_asset_to_stop" => assets::audio_missing_asset_to_stop(graph, issue),
        "clear_missing_asset_reference" | "clear_unsafe_asset_reference" => {
            assets::clear_asset_reference(graph, issue)
        }
        "character_prune_or_fill_invalid_names" => fix_character_names(graph, require_node(issue)?),
        "character_set_default_scale" => set_character_scale(graph, require_node(issue)?),
        other => Err(format!("unknown quick-fix id {other}")),
    }
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

fn set_jump_target_start(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(node) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} not found"));
    };
    match node {
        StoryNode::Jump { target } | StoryNode::JumpIf { target, .. }
            if target.trim().is_empty() =>
        {
            *target = "start".to_string();
            graph.mark_modified();
            Ok(true)
        }
        StoryNode::Jump { .. } | StoryNode::JumpIf { .. } => Ok(false),
        _ => Err(format!("node_id {node_id} is not Jump/JumpIf")),
    }
}

fn set_transition_kind(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::Transition { kind, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Transition"));
    };
    if matches!(kind.as_str(), "fade" | "fade_black" | "dissolve" | "cut") {
        return Ok(false);
    }
    *kind = "fade".to_string();
    graph.mark_modified();
    Ok(true)
}

fn set_transition_duration(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::Transition { duration_ms, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Transition"));
    };
    if *duration_ms > 0 {
        return Ok(false);
    }
    *duration_ms = 300;
    graph.mark_modified();
    Ok(true)
}

fn normalize_audio_channel(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::AudioAction { channel, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not AudioAction"));
    };
    let normalized = match channel.to_ascii_lowercase().as_str() {
        "sfx" | "fx" => "sfx",
        "voice" | "vo" => "voice",
        _ => "bgm",
    };
    if channel == normalized {
        return Ok(false);
    }
    *channel = normalized.to_string();
    graph.mark_modified();
    Ok(true)
}

fn normalize_audio_action(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::AudioAction { action, asset, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not AudioAction"));
    };
    let normalized = match action.to_ascii_lowercase().as_str() {
        "play" | "start" => "play",
        "fade" | "fadeout" | "fade_out" => "fade_out",
        "stop" => "stop",
        _ if asset
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()) =>
        {
            "play"
        }
        _ => "stop",
    };
    if action == normalized {
        return Ok(false);
    }
    *action = normalized.to_string();
    graph.mark_modified();
    Ok(true)
}

fn clamp_audio_volume(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::AudioAction { volume, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not AudioAction"));
    };
    let Some(current) = *volume else {
        return Ok(false);
    };
    let normalized = if current.is_finite() {
        current.clamp(0.0, 1.0)
    } else {
        1.0
    };
    if (normalized - current).abs() <= f32::EPSILON {
        return Ok(false);
    }
    *volume = Some(normalized);
    graph.mark_modified();
    Ok(true)
}

fn set_audio_fade(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::AudioAction {
        fade_duration_ms, ..
    }) = graph.get_node_mut(node_id)
    else {
        return Err(format!("node_id {node_id} is not AudioAction"));
    };
    if fade_duration_ms.unwrap_or(0) > 0 {
        return Ok(false);
    }
    *fade_duration_ms = Some(250);
    graph.mark_modified();
    Ok(true)
}

fn fix_character_names(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(node) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} not found"));
    };
    match node {
        StoryNode::CharacterPlacement { name, .. } if name.trim().is_empty() => {
            *name = "Character".to_string();
            graph.mark_modified();
            Ok(true)
        }
        StoryNode::Scene { characters, .. } => {
            let before = characters.len();
            characters.retain(|character| !character.name.trim().is_empty());
            let changed = before != characters.len();
            if changed {
                graph.mark_modified();
            }
            Ok(changed)
        }
        StoryNode::ScenePatch(patch) => {
            let before_add = patch.add.len();
            let before_update = patch.update.len();
            let before_remove = patch.remove.len();
            patch
                .add
                .retain(|character| !character.name.trim().is_empty());
            patch
                .update
                .retain(|character| !character.name.trim().is_empty());
            patch.remove.retain(|name| !name.trim().is_empty());
            let changed = before_add != patch.add.len()
                || before_update != patch.update.len()
                || before_remove != patch.remove.len();
            if changed {
                graph.mark_modified();
            }
            Ok(changed)
        }
        _ => Ok(false),
    }
}

fn set_character_scale(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::CharacterPlacement { scale, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not CharacterPlacement"));
    };
    if !scale.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        return Ok(false);
    }
    *scale = Some(1.0);
    graph.mark_modified();
    Ok(true)
}
