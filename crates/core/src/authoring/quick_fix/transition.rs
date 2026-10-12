use crate::authoring::{NodeGraph, StoryNode};
use crate::event::transition_kind_code;

pub(super) fn set_kind(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::Transition { kind, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Transition"));
    };
    if transition_kind_code(kind).is_some() {
        return Ok(false);
    }
    *kind = "fade".to_string();
    graph.mark_modified();
    Ok(true)
}

pub(super) fn set_duration(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::Transition { duration_ms, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Transition"));
    };
    if *duration_ms > 0 {
        return Ok(false);
    }
    *duration_ms = 300;
    graph.mark_modified();
    Ok(true)
}
