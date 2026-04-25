use crate::editor::{LintIssue, NodeGraph, StoryNode};

use super::support::require_node_id;

pub(crate) fn apply_empty_character_name(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_character_name_fix(
        graph,
        require_node_id(issue, "character_prune_or_fill_invalid_names")?,
    )
}

pub(crate) fn apply_invalid_character_scale(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_character_scale_fix(
        graph,
        require_node_id(issue, "character_set_default_scale")?,
    )
}

fn apply_character_name_fix(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(node) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} not found"));
    };
    let mut changed = false;
    match node {
        StoryNode::Scene { characters, .. } => {
            let before = characters.len();
            characters.retain(|character| !character.name.trim().is_empty());
            changed = characters.len() != before;
        }
        StoryNode::ScenePatch(patch) => {
            let before_add = patch.add.len();
            let before_upd = patch.update.len();
            let before_rem = patch.remove.len();
            patch
                .add
                .retain(|character| !character.name.trim().is_empty());
            patch
                .update
                .retain(|character| !character.name.trim().is_empty());
            patch.remove.retain(|name| !name.trim().is_empty());
            changed = before_add != patch.add.len()
                || before_upd != patch.update.len()
                || before_rem != patch.remove.len();
        }
        StoryNode::CharacterPlacement { name, .. } => {
            if name.trim().is_empty() {
                *name = "Character".to_string();
                changed = true;
            }
        }
        _ => return Err(format!("node_id {node_id} is not a character container")),
    }
    if changed {
        graph.mark_modified();
    }
    Ok(changed)
}

fn apply_character_scale_fix(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::CharacterPlacement { scale, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not CharacterPlacement"));
    };
    let invalid = scale.is_some_and(|value| !value.is_finite() || value <= 0.0);
    if !invalid {
        return Ok(false);
    }
    *scale = Some(1.0);
    graph.mark_modified();
    Ok(true)
}
