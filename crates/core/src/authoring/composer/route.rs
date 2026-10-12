use std::collections::{BTreeMap, VecDeque};

use crate::{event::EventCompiled, runtime::Engine};

use super::super::{NodeGraph, StoryNode};

/// Reconstructs an authoring route without executing external calls or requiring
/// the author to play through choices. Returns only the events actually visited.
pub fn preview_scene_route(
    engine: &Engine,
    graph: &NodeGraph,
    selected_node_id: Option<u32>,
) -> (Engine, Vec<u32>) {
    let Some(target) = selected_node_id else {
        let mut preview = engine.clone();
        let mut visited = Vec::new();
        for _ in 0..4096 {
            let ip = preview.state().position;
            let Ok(event) = preview.current_event() else {
                break;
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
            visited.push(ip);
            if preview.step().is_err() || preview.state().position == ip || visual_event {
                break;
            }
        }
        return (preview, visited);
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
    let mut preview = Engine::from_compiled(
        engine.script().clone(),
        engine.policy().clone(),
        engine.limits(),
    )
    .unwrap_or_else(|_| engine.clone());
    let mut visited = Vec::new();
    for node_id in route {
        if graph.event_ip_for_node(node_id).is_none() {
            continue;
        }
        if preview.jump_to_label(&format!("node_{node_id}")).is_err() {
            continue;
        }
        while graph.node_for_event_ip(preview.state().position) == Some(node_id)
            && visited.len() < 4096
        {
            let ip = preview.state().position;
            let Ok(event) = preview.current_event() else {
                break;
            };
            visited.push(ip);
            // These events need user or host input; inspection follows graph edges instead.
            if matches!(
                event,
                EventCompiled::Choice(_)
                    | EventCompiled::Dialogue(_)
                    | EventCompiled::ExtCall { .. }
            ) {
                break;
            }
            if preview.step().is_err() || preview.state().position == ip {
                break;
            }
        }
    }
    (preview, visited)
}
