use super::super::{LintCode, LintIssue, NodeGraph, StoryNode};

pub(super) fn issue_still_matches(issue: &LintIssue, graph: &NodeGraph) -> bool {
    match issue.code {
        LintCode::MissingStart => graph
            .nodes()
            .all(|(_, node, _)| !matches!(node, StoryNode::Start)),
        LintCode::DeadEnd => issue.node_id.is_some_and(|node_id| {
            graph.get_node(node_id).is_some_and(|node| {
                !matches!(node, StoryNode::End)
                    && !graph.connections().any(|conn| conn.from == node_id)
            })
        }),
        LintCode::ChoiceNoOptions => issue.node_id.is_some_and(|node_id| {
            matches!(
                graph.get_node(node_id),
                Some(StoryNode::Choice { options, .. }) if options.is_empty()
            )
        }),
        LintCode::ChoiceOptionUnlinked => issue.node_id.is_some_and(|node_id| {
            if let Some(StoryNode::Choice { options, .. }) = graph.get_node(node_id) {
                (0..options.len()).any(|idx| {
                    !graph
                        .connections()
                        .any(|conn| conn.from == node_id && conn.from_port == idx)
                })
            } else {
                false
            }
        }),
        LintCode::ChoicePortOutOfRange => issue.node_id.is_some_and(|node_id| {
            if let Some(StoryNode::Choice { options, .. }) = graph.get_node(node_id) {
                graph
                    .connections()
                    .any(|conn| conn.from == node_id && conn.from_port >= options.len())
            } else {
                false
            }
        }),
        LintCode::EmptySpeakerName => issue.node_id.is_some_and(|node_id| {
            matches!(
                graph.get_node(node_id),
                Some(StoryNode::Dialogue { speaker, .. }) if speaker.trim().is_empty()
            )
        }),
        LintCode::EmptyJumpTarget => issue.node_id.is_some_and(|node_id| {
            matches!(
                graph.get_node(node_id),
                Some(StoryNode::Jump { target } | StoryNode::JumpIf { target, .. })
                    if target.trim().is_empty()
            )
        }),
        LintCode::InvalidTransitionKind => issue.node_id.is_some_and(|node_id| {
            matches!(
                graph.get_node(node_id),
                Some(StoryNode::Transition { kind, .. })
                    if !matches!(kind.as_str(), "fade" | "fade_black" | "dissolve" | "cut")
            )
        }),
        LintCode::InvalidTransitionDuration => issue.node_id.is_some_and(|node_id| {
            matches!(
                graph.get_node(node_id),
                Some(StoryNode::Transition { duration_ms, .. }) if *duration_ms == 0
            )
        }),
        LintCode::InvalidAudioChannel => issue.node_id.is_some_and(|node_id| {
            matches!(
                graph.get_node(node_id),
                Some(StoryNode::AudioAction { channel, .. })
                    if !matches!(channel.as_str(), "bgm" | "sfx" | "voice")
            )
        }),
        LintCode::InvalidAudioAction => issue.node_id.is_some_and(|node_id| {
            matches!(
                graph.get_node(node_id),
                Some(StoryNode::AudioAction { action, .. })
                    if !matches!(action.as_str(), "play" | "stop" | "fade_out")
            )
        }),
        LintCode::InvalidAudioVolume => issue.node_id.is_some_and(|node_id| {
            matches!(
                graph.get_node(node_id),
                Some(StoryNode::AudioAction { volume: Some(volume), .. })
                    if !volume.is_finite() || !(0.0..=1.0).contains(volume)
            )
        }),
        LintCode::InvalidAudioFade => issue.node_id.is_some_and(|node_id| {
            matches!(
                graph.get_node(node_id),
                Some(StoryNode::AudioAction { action, fade_duration_ms, .. })
                    if matches!(action.as_str(), "stop" | "fade_out")
                        && fade_duration_ms.unwrap_or(0) == 0
            )
        }),
        LintCode::SceneBackgroundEmpty => issue.node_id.is_some_and(|node_id| {
            matches!(
                graph.get_node(node_id),
                Some(StoryNode::Scene { background: Some(background), .. })
                    if background.trim().is_empty()
            )
        }),
        LintCode::AudioAssetEmpty => {
            node_has_asset_ref(graph, issue, |asset| asset.trim().is_empty())
        }
        LintCode::AudioAssetMissing
        | LintCode::AssetReferenceMissing
        | LintCode::UnsafeAssetPath => {
            let Some(expected) = issue.asset_path.as_deref() else {
                return issue
                    .node_id
                    .and_then(|node_id| graph.get_node(node_id))
                    .is_some();
            };
            node_has_asset_ref(graph, issue, |asset| asset == expected)
        }
        LintCode::EmptyCharacterName => issue.node_id.is_some_and(|node_id| {
            graph.get_node(node_id).is_some_and(|node| match node {
                StoryNode::CharacterPlacement { name, .. } => name.trim().is_empty(),
                StoryNode::Scene { characters, .. } => characters
                    .iter()
                    .any(|character| character.name.trim().is_empty()),
                StoryNode::ScenePatch(patch) => {
                    patch
                        .add
                        .iter()
                        .any(|character| character.name.trim().is_empty())
                        || patch
                            .update
                            .iter()
                            .any(|character| character.name.trim().is_empty())
                        || patch.remove.iter().any(|name| name.trim().is_empty())
                }
                _ => false,
            })
        }),
        LintCode::InvalidCharacterScale => issue.node_id.is_some_and(|node_id| {
            matches!(
                graph.get_node(node_id),
                Some(StoryNode::CharacterPlacement { scale: Some(scale), .. })
                    if !scale.is_finite() || *scale <= 0.0
            )
        }),
        _ => true,
    }
}

fn node_has_asset_ref<F>(graph: &NodeGraph, issue: &LintIssue, predicate: F) -> bool
where
    F: Fn(&str) -> bool,
{
    let Some(node_id) = issue.node_id else {
        return false;
    };
    let Some(node) = graph.get_node(node_id) else {
        return false;
    };
    match node {
        StoryNode::Scene {
            background,
            music,
            characters,
            ..
        } => {
            background.as_deref().is_some_and(&predicate)
                || music.as_deref().is_some_and(&predicate)
                || characters
                    .iter()
                    .filter_map(|character| character.expression.as_deref())
                    .any(&predicate)
        }
        StoryNode::ScenePatch(patch) => {
            patch.background.as_deref().is_some_and(&predicate)
                || patch.music.as_deref().is_some_and(&predicate)
                || patch
                    .add
                    .iter()
                    .filter_map(|character| character.expression.as_deref())
                    .any(&predicate)
                || patch
                    .update
                    .iter()
                    .filter_map(|character| character.expression.as_deref())
                    .any(&predicate)
        }
        StoryNode::AudioAction { asset, .. } => asset.as_deref().is_some_and(predicate),
        _ => false,
    }
}
