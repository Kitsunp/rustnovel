use std::collections::{BTreeMap, VecDeque};

use crate::{
    error::{VnError, VnResult},
    event::EventCompiled,
    runtime::Engine,
};

use super::super::{NodeGraph, StoryNode};

const MAX_PREVIEW_EVENTS: usize = 4096;

fn record_preview_event(visited: &mut Vec<u32>, ip: u32) -> VnResult<()> {
    if visited.len() == MAX_PREVIEW_EVENTS {
        return Err(VnError::resource_limit(format!(
            "Composer route exceeded {MAX_PREVIEW_EVENTS} events at ip {ip}"
        )));
    }
    visited.push(ip);
    Ok(())
}

/// Reconstructs an authoring route without executing external calls or requiring
/// the author to play through choices. Returns only the events actually visited.
pub fn preview_scene_route(
    engine: &Engine,
    graph: &NodeGraph,
    selected_node_id: Option<u32>,
) -> VnResult<(Engine, Vec<u32>)> {
    let Some(target) = selected_node_id else {
        let mut preview = engine.clone();
        let mut visited = Vec::new();
        loop {
            let ip = preview.state().position;
            let event = match preview.current_event_ref() {
                Ok(event) => event,
                Err(VnError::EndOfScript) => break,
                Err(error) => return Err(error),
            };
            if matches!(
                event,
                EventCompiled::Choice(_) | EventCompiled::ExtCall { .. }
            ) {
                break;
            }
            let visual_event = matches!(
                event,
                EventCompiled::Scene(_)
                    | EventCompiled::Patch(_)
                    | EventCompiled::SetCharacterPosition(_)
                    | EventCompiled::Dialogue(_)
            );
            record_preview_event(&mut visited, ip)?;
            preview.step()?;
            if preview.state().position == ip || visual_event {
                break;
            }
        }
        return Ok((preview, visited));
    };
    let start = graph
        .nodes()
        .find_map(|(id, node, _)| matches!(node, StoryNode::Start).then_some(*id));
    let mut parents = BTreeMap::new();
    let mut queue = VecDeque::new();
    if let Some(start) = start {
        parents.insert(start, None);
        queue.push_back(start);
    }
    while let Some(node_id) = queue.pop_front() {
        if node_id == target {
            break;
        }
        let mut edges = graph
            .connections()
            .filter(|edge| edge.from == node_id)
            .collect::<Vec<_>>();
        edges.sort_by_key(|edge| (edge.from_port, edge.to));
        for edge in edges {
            if let std::collections::btree_map::Entry::Vacant(entry) = parents.entry(edge.to) {
                entry.insert(Some(node_id));
                queue.push_back(edge.to);
            }
        }
    }
    let mut route = vec![target];
    let mut cursor = target;
    while let Some(Some(parent)) = parents.get(&cursor) {
        route.push(*parent);
        cursor = *parent;
    }
    route.reverse();
    let mut preview = engine.fresh_session();
    let mut visited = Vec::new();
    let script_order = graph.script_order_node_ids();
    for node_id in route {
        if graph.get_node(node_id).is_none_or(StoryNode::is_marker) {
            continue;
        }
        preview.jump_to_label(&format!("node_{node_id}"))?;
        while script_order.get(preview.state().position as usize) == Some(&node_id) {
            let ip = preview.state().position;
            let event = preview.current_event_ref()?;
            record_preview_event(&mut visited, ip)?;
            // These events need user or host input; inspection follows graph edges instead.
            if matches!(
                event,
                EventCompiled::Choice(_)
                    | EventCompiled::Dialogue(_)
                    | EventCompiled::ExtCall { .. }
            ) {
                break;
            }
            preview.step()?;
            if preview.state().position == ip {
                break;
            }
        }
    }
    Ok((preview, visited))
}
