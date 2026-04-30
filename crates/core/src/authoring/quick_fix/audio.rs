use super::super::{NodeGraph, StoryNode};

pub(super) fn normalize_channel(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
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

pub(super) fn normalize_action(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
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

pub(super) fn clamp_volume(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
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

pub(super) fn set_default_fade(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
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
