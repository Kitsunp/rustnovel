use std::collections::{BTreeMap, BTreeSet};

use crate::{EventRaw, ScriptRaw};

use super::{AuthoringPosition, NodeGraph, StoryNode, NODE_VERTICAL_SPACING};

mod export;
mod export_validation;
mod labels;

pub use export::{to_script, to_script_lossy_for_diagnostics, to_script_strict};

pub fn from_script(script: &ScriptRaw) -> NodeGraph {
    let mut graph = NodeGraph::new();
    if script.events.is_empty() {
        return graph;
    }

    let start_id = graph.add_node(StoryNode::Start, AuthoringPosition::new(50.0, 30.0));
    let mut index_to_id: BTreeMap<usize, u32> = BTreeMap::new();

    for (idx, event) in script.events.iter().enumerate() {
        let y = 100.0 + (idx as f32) * NODE_VERTICAL_SPACING;
        let node = node_from_event(event);
        let id = graph.add_node(node, AuthoringPosition::new(100.0, y));
        index_to_id.insert(idx, id);
    }

    let last_y = 100.0 + (script.events.len() as f32) * NODE_VERTICAL_SPACING;
    let end_id = graph.add_node(StoryNode::End, AuthoringPosition::new(100.0, last_y));
    if let Some(first_id) = index_to_id.get(&0).copied() {
        graph.connect(start_id, first_id);
    }

    let label_to_index = script
        .labels
        .iter()
        .map(|(name, idx)| (name.as_str(), *idx))
        .collect::<BTreeMap<_, _>>();
    let flow_targets = FlowTargetLookup {
        end_id,
        label_to_index: &label_to_index,
        index_to_id: &index_to_id,
        event_count: script.events.len(),
    };

    for (idx, event) in script.events.iter().enumerate() {
        let Some(from_id) = index_to_id.get(&idx).copied() else {
            continue;
        };
        connect_event_flow(&mut graph, event, idx, from_id, &flow_targets);
    }

    autoconnect_dangling_nodes(&mut graph, script, &index_to_id, end_id);
    graph.clear_modified();
    graph
}

fn autoconnect_dangling_nodes(
    graph: &mut NodeGraph,
    script: &ScriptRaw,
    index_to_id: &BTreeMap<usize, u32>,
    end_id: u32,
) {
    let nodes_with_outgoing = graph
        .connections()
        .map(|conn| conn.from)
        .collect::<BTreeSet<_>>();
    let dangling = graph
        .nodes()
        .map(|(id, _, _)| *id)
        .filter(|id| {
            !nodes_with_outgoing.contains(id)
                && !matches!(graph.get_node(*id), Some(StoryNode::End))
        })
        .collect::<Vec<_>>();
    for id in dangling {
        let Some(index) = index_to_id
            .iter()
            .find_map(|(idx, node_id)| (*node_id == id).then_some(*idx))
        else {
            continue;
        };
        if should_autoconnect_dangling(&script.events[index]) {
            graph.connect(id, end_id);
        }
    }
}

fn should_autoconnect_dangling(event: &EventRaw) -> bool {
    !matches!(
        event,
        EventRaw::Jump { .. } | EventRaw::JumpIf { .. } | EventRaw::Choice(_)
    )
}

fn node_from_event(event: &EventRaw) -> StoryNode {
    match event {
        EventRaw::Dialogue(dialogue) => StoryNode::Dialogue {
            speaker: dialogue.speaker.clone(),
            text: dialogue.text.clone(),
        },
        EventRaw::Choice(choice) => StoryNode::Choice {
            prompt: choice.prompt.clone(),
            options: choice
                .options
                .iter()
                .map(|option| option.text.clone())
                .collect(),
        },
        EventRaw::Scene(scene) => StoryNode::Scene {
            profile: None,
            background: scene.background.clone(),
            music: scene.music.clone(),
            characters: scene.characters.clone(),
        },
        EventRaw::Jump { target } => StoryNode::Jump {
            target: target.clone(),
        },
        EventRaw::SetFlag { key, value } => StoryNode::SetFlag {
            key: key.clone(),
            value: *value,
        },
        EventRaw::SetVar { key, value } => StoryNode::SetVariable {
            key: key.clone(),
            value: *value,
        },
        EventRaw::JumpIf { cond, target } => StoryNode::JumpIf {
            target: target.clone(),
            cond: cond.clone(),
        },
        EventRaw::Patch(patch) => StoryNode::ScenePatch(patch.clone()),
        EventRaw::AudioAction(action) => StoryNode::AudioAction {
            channel: action.channel.clone(),
            action: action.action.clone(),
            asset: action.asset.clone(),
            volume: action.volume,
            fade_duration_ms: action.fade_duration_ms,
            loop_playback: action.loop_playback,
        },
        EventRaw::Transition(transition) => StoryNode::Transition {
            kind: transition.kind.clone(),
            duration_ms: transition.duration_ms,
            color: transition.color.clone(),
        },
        EventRaw::SetCharacterPosition(pos) => StoryNode::CharacterPlacement {
            name: pos.name.clone(),
            x: pos.x,
            y: pos.y,
            scale: pos.scale,
        },
        EventRaw::ExtCall { .. } => StoryNode::Generic(event.clone()),
    }
}

fn connect_event_flow(
    graph: &mut NodeGraph,
    event: &EventRaw,
    idx: usize,
    from_id: u32,
    targets: &FlowTargetLookup<'_>,
) {
    match event {
        EventRaw::Jump { target } => connect_target(graph, from_id, 0, target, targets),
        EventRaw::Choice(choice) => {
            for (port, option) in choice.options.iter().enumerate() {
                connect_target(graph, from_id, port, &option.target, targets);
            }
        }
        EventRaw::JumpIf { target, .. } => {
            connect_target(graph, from_id, 0, target, targets);
            if let Some(next_id) = targets.index_to_id.get(&(idx + 1)).copied() {
                graph.connect_port(from_id, 1, next_id);
            }
        }
        _ => {
            if let Some(next_id) = targets.index_to_id.get(&(idx + 1)).copied() {
                graph.connect(from_id, next_id);
            }
        }
    }
}

struct FlowTargetLookup<'a> {
    end_id: u32,
    label_to_index: &'a BTreeMap<&'a str, usize>,
    index_to_id: &'a BTreeMap<usize, u32>,
    event_count: usize,
}

fn connect_target(
    graph: &mut NodeGraph,
    from_id: u32,
    from_port: usize,
    target: &str,
    targets: &FlowTargetLookup<'_>,
) {
    let Some(target_idx) = targets.label_to_index.get(target).copied() else {
        return;
    };
    if target_idx == targets.event_count {
        graph.connect_port(from_id, from_port, targets.end_id);
    } else if let Some(target_id) = targets.index_to_id.get(&target_idx).copied() {
        graph.connect_port(from_id, from_port, target_id);
    }
}
