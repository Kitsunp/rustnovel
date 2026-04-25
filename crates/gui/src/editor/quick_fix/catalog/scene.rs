use crate::editor::{LintIssue, NodeGraph, StoryNode};

use super::support::{clearable_asset_field, require_node_id, AssetField};

pub(crate) fn apply_empty_scene_background(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_scene_background_clear(
        graph,
        require_node_id(issue, "scene_clear_empty_background")?,
    )
}

pub(crate) fn apply_empty_scene_music(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_scene_music_clear(graph, require_node_id(issue, "scene_clear_empty_music")?)
}

pub(crate) fn apply_clear_asset_reference(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    let field = clearable_asset_field(
        graph,
        issue,
        issue.code == crate::editor::LintCode::UnsafeAssetPath,
    )
    .ok_or_else(|| {
        format!(
            "unable to resolve a unique asset field for issue {}",
            issue.diagnostic_id()
        )
    })?;
    let node_id = require_node_id(issue, "clear_asset_reference")?;
    let Some(node) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} not found"));
    };

    let mut changed = false;
    match (field, node) {
        (AssetField::SceneBackground, StoryNode::Scene { background, .. }) => {
            if background.take().is_some() {
                changed = true;
            }
        }
        (AssetField::SceneMusic, StoryNode::Scene { music, .. }) => {
            if music.take().is_some() {
                changed = true;
            }
        }
        (AssetField::ScenePatchBackground, StoryNode::ScenePatch(patch)) => {
            if patch.background.take().is_some() {
                changed = true;
            }
        }
        (AssetField::ScenePatchMusic, StoryNode::ScenePatch(patch)) => {
            if patch.music.take().is_some() {
                changed = true;
            }
        }
        (AssetField::AudioAsset, StoryNode::AudioAction { action, asset, .. }) => {
            if asset.take().is_some() {
                changed = true;
            }
            if action.trim().eq_ignore_ascii_case("play") {
                *action = "stop".to_string();
                changed = true;
            }
        }
        _ => {
            return Err(format!(
                "asset field {:?} is incompatible with node {}",
                field, node_id
            ));
        }
    }

    if changed {
        graph.mark_modified();
    }
    Ok(changed)
}

fn apply_scene_background_clear(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::Scene { background, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Scene"));
    };
    if !background.as_deref().is_some_and(|v| v.trim().is_empty()) {
        return Ok(false);
    }
    *background = None;
    graph.mark_modified();
    Ok(true)
}

fn apply_scene_music_clear(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
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
