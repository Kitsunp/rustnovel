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
            let mut changed = false;
            for character in characters {
                if character.name.trim().is_empty() {
                    character.name = "Character".to_string();
                    changed = true;
                }
            }
            if changed {
                graph.mark_modified();
            }
            Ok(changed)
        }
        StoryNode::ScenePatch(patch) => {
            let before_remove = patch.remove.len();
            let mut changed = false;
            for character in &mut patch.add {
                if character.name.trim().is_empty() {
                    character.name = "Character".to_string();
                    changed = true;
                }
            }
            for character in &mut patch.update {
                if character.name.trim().is_empty() {
                    character.name = "Character".to_string();
                    changed = true;
                }
            }
            patch.remove.retain(|name| !name.trim().is_empty());
            let changed = changed || before_remove != patch.remove.len();
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
