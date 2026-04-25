use super::super::{LintIssue, NodeGraph, StoryNode};
use super::{candidate, QuickFixCandidate, QuickFixRisk};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AssetField {
    SceneBackground,
    SceneMusic,
    ScenePatchBackground,
    ScenePatchMusic,
    AudioAsset,
}

pub(super) fn empty_asset_candidate(
    issue: &LintIssue,
    graph: &NodeGraph,
) -> Option<QuickFixCandidate> {
    let node_id = issue.node_id?;
    match graph.get_node(node_id) {
        Some(StoryNode::Scene { music, .. })
            if music
                .as_deref()
                .is_some_and(|value| value.trim().is_empty()) =>
        {
            Some(candidate(
                "scene_clear_empty_music",
                "Limpiar musica vacia",
                "Clear empty music",
                QuickFixRisk::Safe,
                false,
            ))
        }
        Some(StoryNode::AudioAction { asset, .. })
            if asset
                .as_deref()
                .is_some_and(|value| value.trim().is_empty()) =>
        {
            Some(candidate(
                "audio_clear_empty_asset",
                "Limpiar asset de audio vacio",
                "Clear empty audio asset",
                QuickFixRisk::Safe,
                false,
            ))
        }
        _ => None,
    }
}

pub(super) fn missing_audio_candidate(
    issue: &LintIssue,
    graph: &NodeGraph,
) -> Option<QuickFixCandidate> {
    let node_id = issue.node_id?;
    let Some(StoryNode::AudioAction { action, asset, .. }) = graph.get_node(node_id) else {
        return None;
    };
    if !action.trim().eq_ignore_ascii_case("play")
        || asset
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        return None;
    }
    Some(candidate(
        "audio_missing_asset_to_stop",
        "Normalizar play sin asset a stop",
        "Normalize play without asset to stop",
        QuickFixRisk::Review,
        false,
    ))
}

pub(super) fn clear_asset_candidate(
    issue: &LintIssue,
    graph: &NodeGraph,
    fix_id: &'static str,
    title_es: &'static str,
    title_en: &'static str,
) -> Option<QuickFixCandidate> {
    clearable_asset_field(graph, issue)
        .map(|_| candidate(fix_id, title_es, title_en, QuickFixRisk::Review, false))
}

pub(super) fn clear_empty_scene_background(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    let node_id = require_node(issue, "scene_clear_empty_background")?;
    let Some(StoryNode::Scene { background, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Scene"));
    };
    if !background
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Ok(false);
    }
    *background = None;
    graph.mark_modified();
    Ok(true)
}

pub(super) fn clear_empty_scene_music(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    let node_id = require_node(issue, "scene_clear_empty_music")?;
    let Some(StoryNode::Scene { music, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Scene"));
    };
    if !music
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Ok(false);
    }
    *music = None;
    graph.mark_modified();
    Ok(true)
}

pub(super) fn clear_empty_audio_asset(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    let node_id = require_node(issue, "audio_clear_empty_asset")?;
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

pub(super) fn audio_missing_asset_to_stop(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    let node_id = require_node(issue, "audio_missing_asset_to_stop")?;
    let Some(StoryNode::AudioAction { action, asset, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not AudioAction"));
    };
    let missing_asset = asset.as_deref().is_none_or(|value| value.trim().is_empty());
    if !action.trim().eq_ignore_ascii_case("play") || !missing_asset {
        return Ok(false);
    }
    *action = "stop".to_string();
    *asset = None;
    graph.mark_modified();
    Ok(true)
}

pub(super) fn clear_asset_reference(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    let node_id = require_node(issue, "clear_asset_reference")?;
    let field = clearable_asset_field(graph, issue).ok_or_else(|| {
        format!(
            "unable to resolve a unique asset field for issue {}",
            issue.diagnostic_id()
        )
    })?;
    let Some(node) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} not found"));
    };

    let mut changed = false;
    match (field, node) {
        (AssetField::SceneBackground, StoryNode::Scene { background, .. }) => {
            changed |= background.take().is_some();
        }
        (AssetField::SceneMusic, StoryNode::Scene { music, .. }) => {
            changed |= music.take().is_some();
        }
        (AssetField::ScenePatchBackground, StoryNode::ScenePatch(patch)) => {
            changed |= patch.background.take().is_some();
        }
        (AssetField::ScenePatchMusic, StoryNode::ScenePatch(patch)) => {
            changed |= patch.music.take().is_some();
        }
        (AssetField::AudioAsset, StoryNode::AudioAction { action, asset, .. }) => {
            changed |= asset.take().is_some();
            if action.trim().eq_ignore_ascii_case("play") {
                *action = "stop".to_string();
                changed = true;
            }
        }
        _ => return Err(format!("asset field is incompatible with node {node_id}")),
    }
    if changed {
        graph.mark_modified();
    }
    Ok(changed)
}

fn clearable_asset_field(graph: &NodeGraph, issue: &LintIssue) -> Option<AssetField> {
    let node_id = issue.node_id?;
    let node = graph.get_node(node_id)?;
    let target = issue.asset_path.as_deref();
    let mut fields = Vec::new();
    match node {
        StoryNode::Scene {
            background, music, ..
        } => {
            push_if_matches(&mut fields, AssetField::SceneBackground, background, target);
            push_if_matches(&mut fields, AssetField::SceneMusic, music, target);
        }
        StoryNode::ScenePatch(patch) => {
            push_if_matches(
                &mut fields,
                AssetField::ScenePatchBackground,
                &patch.background,
                target,
            );
            push_if_matches(
                &mut fields,
                AssetField::ScenePatchMusic,
                &patch.music,
                target,
            );
        }
        StoryNode::AudioAction { asset, .. } => {
            push_if_matches(&mut fields, AssetField::AudioAsset, asset, target);
        }
        _ => {}
    }
    if fields.len() == 1 {
        fields.pop()
    } else {
        None
    }
}

fn push_if_matches(
    fields: &mut Vec<AssetField>,
    field: AssetField,
    value: &Option<String>,
    target: Option<&str>,
) {
    let Some(value) = value.as_deref() else {
        return;
    };
    let matched = match target {
        Some(target) => value == target,
        None => is_unsafe_asset_ref(value),
    };
    if matched {
        fields.push(field);
    }
}

fn require_node(issue: &LintIssue, fix_id: &str) -> Result<u32, String> {
    issue
        .node_id
        .ok_or_else(|| format!("quick-fix {fix_id} requires node_id"))
}

fn is_unsafe_asset_ref(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    path.starts_with('/')
        || path.starts_with('\\')
        || lower.contains("://")
        || path.split(['/', '\\']).any(|part| part == "..")
}
