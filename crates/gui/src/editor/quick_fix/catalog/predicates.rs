use crate::editor::{LintCode, LintIssue, NodeGraph, StoryNode};

use super::support::clearable_asset_field;

fn matches_issue_on_node(
    issue: &LintIssue,
    graph: &NodeGraph,
    code: LintCode,
    predicate: fn(&NodeGraph, u32) -> bool,
) -> bool {
    issue.code == code
        && issue
            .node_id
            .is_some_and(|node_id| predicate(graph, node_id))
}

pub(crate) fn node_is_choice(graph: &NodeGraph, node_id: u32) -> bool {
    matches!(graph.get_node(node_id), Some(StoryNode::Choice { .. }))
}

pub(crate) fn node_is_dialogue(graph: &NodeGraph, node_id: u32) -> bool {
    matches!(graph.get_node(node_id), Some(StoryNode::Dialogue { .. }))
}

pub(crate) fn node_is_jump_like(graph: &NodeGraph, node_id: u32) -> bool {
    matches!(
        graph.get_node(node_id),
        Some(StoryNode::Jump { .. } | StoryNode::JumpIf { .. })
    )
}

pub(crate) fn node_is_transition(graph: &NodeGraph, node_id: u32) -> bool {
    matches!(graph.get_node(node_id), Some(StoryNode::Transition { .. }))
}

pub(crate) fn node_is_audio_action(graph: &NodeGraph, node_id: u32) -> bool {
    matches!(graph.get_node(node_id), Some(StoryNode::AudioAction { .. }))
}

pub(crate) fn node_is_scene(graph: &NodeGraph, node_id: u32) -> bool {
    matches!(graph.get_node(node_id), Some(StoryNode::Scene { .. }))
}

pub(crate) fn node_is_character_container(graph: &NodeGraph, node_id: u32) -> bool {
    matches!(
        graph.get_node(node_id),
        Some(
            StoryNode::Scene { .. }
                | StoryNode::ScenePatch(_)
                | StoryNode::CharacterPlacement { .. }
        )
    )
}

pub(crate) fn node_is_character_placement(graph: &NodeGraph, node_id: u32) -> bool {
    matches!(
        graph.get_node(node_id),
        Some(StoryNode::CharacterPlacement { .. })
    )
}

pub(crate) fn matches_missing_start(issue: &LintIssue, _graph: &NodeGraph) -> bool {
    issue.code == LintCode::MissingStart
}

pub(crate) fn matches_dead_end(issue: &LintIssue, graph: &NodeGraph) -> bool {
    if issue.code != LintCode::DeadEnd {
        return false;
    }
    let Some(node_id) = issue.node_id else {
        return false;
    };
    !matches!(graph.get_node(node_id), Some(StoryNode::End))
}

pub(crate) fn matches_choice_no_options(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(issue, graph, LintCode::ChoiceNoOptions, node_is_choice)
}

pub(crate) fn matches_choice_option_unlinked(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(issue, graph, LintCode::ChoiceOptionUnlinked, node_is_choice)
}

pub(crate) fn matches_choice_port_out_of_range(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(issue, graph, LintCode::ChoicePortOutOfRange, node_is_choice)
}

pub(crate) fn matches_empty_speaker(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(issue, graph, LintCode::EmptySpeakerName, node_is_dialogue)
}

pub(crate) fn matches_empty_jump_target(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(issue, graph, LintCode::EmptyJumpTarget, node_is_jump_like)
}

pub(crate) fn matches_invalid_transition_kind(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(
        issue,
        graph,
        LintCode::InvalidTransitionKind,
        node_is_transition,
    )
}

pub(crate) fn matches_invalid_transition_duration(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(
        issue,
        graph,
        LintCode::InvalidTransitionDuration,
        node_is_transition,
    )
}

pub(crate) fn matches_invalid_audio_channel(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(
        issue,
        graph,
        LintCode::InvalidAudioChannel,
        node_is_audio_action,
    )
}

pub(crate) fn matches_invalid_audio_action(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(
        issue,
        graph,
        LintCode::InvalidAudioAction,
        node_is_audio_action,
    )
}

pub(crate) fn matches_invalid_audio_volume(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(
        issue,
        graph,
        LintCode::InvalidAudioVolume,
        node_is_audio_action,
    )
}

pub(crate) fn matches_invalid_audio_fade(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(
        issue,
        graph,
        LintCode::InvalidAudioFade,
        node_is_audio_action,
    )
}

pub(crate) fn matches_empty_scene_background(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(issue, graph, LintCode::SceneBackgroundEmpty, node_is_scene)
}

pub(crate) fn matches_empty_scene_music(issue: &LintIssue, graph: &NodeGraph) -> bool {
    if issue.code != LintCode::AudioAssetEmpty {
        return false;
    }
    let Some(node_id) = issue.node_id else {
        return false;
    };
    let Some(StoryNode::Scene { music, .. }) = graph.get_node(node_id) else {
        return false;
    };
    music
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
}

pub(crate) fn matches_empty_audio_asset(issue: &LintIssue, graph: &NodeGraph) -> bool {
    if issue.code != LintCode::AudioAssetEmpty {
        return false;
    }
    let Some(node_id) = issue.node_id else {
        return false;
    };
    let Some(StoryNode::AudioAction { asset, .. }) = graph.get_node(node_id) else {
        return false;
    };
    asset
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
}

pub(crate) fn matches_audio_missing_asset(issue: &LintIssue, graph: &NodeGraph) -> bool {
    if issue.code != LintCode::AudioAssetMissing {
        return false;
    }
    let Some(node_id) = issue.node_id else {
        return false;
    };
    let Some(StoryNode::AudioAction { action, asset, .. }) = graph.get_node(node_id) else {
        return false;
    };
    action.trim().eq_ignore_ascii_case("play")
        && asset.as_deref().is_none_or(|value| value.trim().is_empty())
}

pub(crate) fn matches_missing_asset_reference(issue: &LintIssue, graph: &NodeGraph) -> bool {
    issue.code == LintCode::AssetReferenceMissing
        && clearable_asset_field(graph, issue, false).is_some()
}

pub(crate) fn matches_unsafe_asset_reference(issue: &LintIssue, graph: &NodeGraph) -> bool {
    issue.code == LintCode::UnsafeAssetPath && clearable_asset_field(graph, issue, true).is_some()
}

pub(crate) fn matches_empty_character_name(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(
        issue,
        graph,
        LintCode::EmptyCharacterName,
        node_is_character_container,
    )
}

pub(crate) fn matches_invalid_character_scale(issue: &LintIssue, graph: &NodeGraph) -> bool {
    matches_issue_on_node(
        issue,
        graph,
        LintCode::InvalidCharacterScale,
        node_is_character_placement,
    )
}
