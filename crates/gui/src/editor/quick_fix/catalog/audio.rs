use crate::editor::{LintIssue, NodeGraph, StoryNode};

use super::support::require_node_id;

fn canonical_token(value: &str) -> String {
    value
        .chars()
        .filter(|char| char.is_ascii_alphanumeric())
        .map(|char| char.to_ascii_lowercase())
        .collect()
}

fn normalize_audio_channel(value: &str) -> &'static str {
    match canonical_token(value).as_str() {
        "bgm" | "music" | "backgroundmusic" | "bgmusic" => "bgm",
        "sfx" | "fx" | "soundeffect" | "soundeffects" => "sfx",
        "voice" | "vo" | "voiceover" => "voice",
        _ => "bgm",
    }
}

fn normalize_audio_action(value: &str, has_asset: bool) -> &'static str {
    match canonical_token(value).as_str() {
        "play" | "start" | "resume" => "play",
        "stop" | "halt" => "stop",
        "fadeout" | "fade" => "fade_out",
        _ => {
            if has_asset {
                "play"
            } else {
                "stop"
            }
        }
    }
}

pub(crate) fn apply_invalid_audio_channel(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_audio_channel_fix(graph, require_node_id(issue, "audio_normalize_channel")?)
}

pub(crate) fn apply_invalid_audio_action(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_audio_action_fix(graph, require_node_id(issue, "audio_normalize_action")?)
}

pub(crate) fn apply_invalid_audio_volume(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_audio_volume_fix(graph, require_node_id(issue, "audio_clamp_volume")?)
}

pub(crate) fn apply_invalid_audio_fade(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_audio_fade_fix(graph, require_node_id(issue, "audio_set_default_fade")?)
}

pub(crate) fn apply_empty_audio_asset(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_audio_asset_clear(graph, require_node_id(issue, "audio_clear_empty_asset")?)
}

pub(crate) fn apply_audio_missing_asset(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_audio_missing_asset_fix(
        graph,
        require_node_id(issue, "audio_missing_asset_to_stop")?,
    )
}

fn apply_audio_channel_fix(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::AudioAction { channel, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not AudioAction"));
    };
    let normalized = normalize_audio_channel(channel).to_string();
    if channel.trim().eq_ignore_ascii_case(&normalized) {
        return Ok(false);
    }
    *channel = normalized;
    graph.mark_modified();
    Ok(true)
}

fn apply_audio_action_fix(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::AudioAction { action, asset, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not AudioAction"));
    };
    let has_asset = asset
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let normalized = normalize_audio_action(action, has_asset).to_string();
    if action.trim().eq_ignore_ascii_case(&normalized) {
        return Ok(false);
    }
    *action = normalized;
    graph.mark_modified();
    Ok(true)
}

fn apply_audio_volume_fix(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
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
    if (normalized - current).abs() < f32::EPSILON {
        return Ok(false);
    }
    *volume = Some(normalized);
    graph.mark_modified();
    Ok(true)
}

fn apply_audio_fade_fix(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::AudioAction {
        action,
        fade_duration_ms,
        ..
    }) = graph.get_node_mut(node_id)
    else {
        return Err(format!("node_id {node_id} is not AudioAction"));
    };
    let normalized_action = action.trim().to_ascii_lowercase();
    if !matches!(normalized_action.as_str(), "stop" | "fade_out") {
        return Ok(false);
    }
    if fade_duration_ms.unwrap_or(0) > 0 {
        return Ok(false);
    }
    *fade_duration_ms = Some(250);
    graph.mark_modified();
    Ok(true)
}

fn apply_audio_asset_clear(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::AudioAction { asset, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not AudioAction"));
    };
    if !asset
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Ok(false);
    }
    *asset = None;
    graph.mark_modified();
    Ok(true)
}

fn apply_audio_missing_asset_fix(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::AudioAction { action, asset, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not AudioAction"));
    };
    let is_play = action.trim().eq_ignore_ascii_case("play");
    let missing_asset = asset.as_deref().is_none_or(|value| value.trim().is_empty());
    if !is_play || !missing_asset {
        return Ok(false);
    }
    *action = "stop".to_string();
    *asset = None;
    graph.mark_modified();
    Ok(true)
}
