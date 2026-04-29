use super::super::{NodeGraph, StoryNode};

pub(super) fn fix_names(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
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

pub(super) fn set_scale(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
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
