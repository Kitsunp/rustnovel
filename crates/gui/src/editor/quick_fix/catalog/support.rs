use crate::editor::{LintIssue, NodeGraph, StoryNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssetField {
    SceneBackground,
    SceneMusic,
    ScenePatchBackground,
    ScenePatchMusic,
    AudioAsset,
}

pub(crate) fn require_node_id(issue: &LintIssue, fix_id: &str) -> Result<u32, String> {
    issue
        .node_id
        .ok_or_else(|| format!("fix '{fix_id}' requires node_id"))
}

pub(crate) fn is_unsafe_asset_path(value: &str) -> bool {
    let path = value.trim();
    if path.is_empty() {
        return false;
    }
    let bytes = path.as_bytes();
    path.contains("..")
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.starts_with("http://")
        || path.starts_with("https://")
        || (bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic())
}

pub(crate) fn clearable_asset_field(
    graph: &NodeGraph,
    issue: &LintIssue,
    unsafe_only: bool,
) -> Option<AssetField> {
    let node_id = issue.node_id?;
    let node = graph.get_node(node_id)?;
    let mut candidates: Vec<(AssetField, &str)> = Vec::new();

    match node {
        StoryNode::Scene {
            background, music, ..
        } => {
            if let Some(path) = background.as_deref() {
                candidates.push((AssetField::SceneBackground, path));
            }
            if let Some(path) = music.as_deref() {
                candidates.push((AssetField::SceneMusic, path));
            }
        }
        StoryNode::ScenePatch(patch) => {
            if let Some(path) = patch.background.as_deref() {
                candidates.push((AssetField::ScenePatchBackground, path));
            }
            if let Some(path) = patch.music.as_deref() {
                candidates.push((AssetField::ScenePatchMusic, path));
            }
        }
        StoryNode::AudioAction { asset, .. } => {
            if let Some(path) = asset.as_deref() {
                candidates.push((AssetField::AudioAsset, path));
            }
        }
        _ => {}
    }

    if candidates.is_empty() {
        return None;
    }

    let filtered = if let Some(explicit_path) = issue.asset_path.as_deref() {
        let target = explicit_path.trim();
        candidates
            .into_iter()
            .filter(|(_, path)| path.trim() == target)
            .collect::<Vec<_>>()
    } else if unsafe_only {
        candidates
            .into_iter()
            .filter(|(_, path)| is_unsafe_asset_path(path))
            .collect::<Vec<_>>()
    } else {
        candidates
    };

    if filtered.len() == 1 {
        return Some(filtered[0].0);
    }
    None
}
