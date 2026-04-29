use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AudioActionRaw, ChoiceOptionRaw, ChoiceRaw, DialogueRaw, EventRaw, SceneTransitionRaw,
    SceneUpdateRaw, ScriptRaw, SetCharacterPositionRaw, VnError, VnResult,
};

use super::{AuthoringPosition, NodeGraph, StoryNode, NODE_VERTICAL_SPACING};

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

    graph.clear_modified();
    graph
}

pub fn to_script(graph: &NodeGraph) -> ScriptRaw {
    let mut events = Vec::new();
    let mut labels = BTreeMap::new();
    let node_lookup = graph
        .nodes()
        .map(|(id, node, _)| (*id, node))
        .collect::<BTreeMap<_, _>>();
    let choice_targets = graph
        .connections()
        .map(|conn| ((conn.from, conn.from_port), conn.to))
        .collect::<BTreeMap<_, _>>();

    for id in graph.script_order_node_ids() {
        let Some(node) = node_lookup.get(&id).copied() else {
            continue;
        };
        if node.is_marker() {
            continue;
        }
        let event_idx = events.len();
        labels.insert(format!("node_{id}"), event_idx);
        if let Some(event) = event_from_node(id, node, &node_lookup, &choice_targets) {
            events.push(event);
        }
    }

    if events.iter().any(targets_end_label) {
        labels.insert("__end".to_string(), events.len());
    }
    if !events.is_empty() {
        labels.entry("start".to_string()).or_insert(0);
    }
    ScriptRaw::new(events, labels)
}

pub fn to_script_lossy_for_diagnostics(graph: &NodeGraph) -> ScriptRaw {
    to_script(graph)
}

pub fn to_script_strict(graph: &NodeGraph) -> VnResult<ScriptRaw> {
    validate_strict_graph_export(graph)?;
    let script = to_script(graph);
    script
        .compile()
        .map(|_| script)
        .map_err(|err| VnError::invalid_script(format!("strict authoring export failed: {err}")))
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

fn event_from_node(
    id: u32,
    node: &StoryNode,
    node_lookup: &BTreeMap<u32, &StoryNode>,
    choice_targets: &BTreeMap<(u32, usize), u32>,
) -> Option<EventRaw> {
    Some(match node {
        StoryNode::Dialogue { speaker, text } => EventRaw::Dialogue(DialogueRaw {
            speaker: speaker.clone(),
            text: text.clone(),
        }),
        StoryNode::Choice { prompt, options } => EventRaw::Choice(ChoiceRaw {
            prompt: prompt.clone(),
            options: options
                .iter()
                .enumerate()
                .map(|(port, text)| ChoiceOptionRaw {
                    text: text.clone(),
                    target: target_label(id, port, node_lookup, choice_targets),
                })
                .collect(),
        }),
        StoryNode::Scene {
            background,
            music,
            characters,
            ..
        } => EventRaw::Scene(SceneUpdateRaw {
            background: background.clone(),
            music: music.clone(),
            characters: characters.clone(),
        }),
        StoryNode::Jump { target } => EventRaw::Jump {
            target: target.clone(),
        },
        StoryNode::SetVariable { key, value } => EventRaw::SetVar {
            key: key.clone(),
            value: *value,
        },
        StoryNode::SetFlag { key, value } => EventRaw::SetFlag {
            key: key.clone(),
            value: *value,
        },
        StoryNode::JumpIf { cond, target } => EventRaw::JumpIf {
            cond: cond.clone(),
            target: jump_if_target_label(id, target, node_lookup, choice_targets),
        },
        StoryNode::ScenePatch(patch) => EventRaw::Patch(patch.clone()),
        StoryNode::AudioAction {
            channel,
            action,
            asset,
            volume,
            fade_duration_ms,
            loop_playback,
        } => EventRaw::AudioAction(AudioActionRaw {
            channel: channel.clone(),
            action: action.clone(),
            asset: asset.clone(),
            volume: *volume,
            fade_duration_ms: *fade_duration_ms,
            loop_playback: *loop_playback,
        }),
        StoryNode::Transition {
            kind,
            duration_ms,
            color,
        } => EventRaw::Transition(SceneTransitionRaw {
            kind: kind.clone(),
            duration_ms: *duration_ms,
            color: color.clone(),
        }),
        StoryNode::CharacterPlacement { name, x, y, scale } => {
            EventRaw::SetCharacterPosition(SetCharacterPositionRaw {
                name: name.clone(),
                x: *x,
                y: *y,
                scale: *scale,
            })
        }
        StoryNode::Generic(event) => event.clone(),
        StoryNode::Start | StoryNode::End => return None,
    })
}

fn jump_if_target_label(
    node_id: u32,
    fallback_target: &str,
    node_lookup: &BTreeMap<u32, &StoryNode>,
    choice_targets: &BTreeMap<(u32, usize), u32>,
) -> String {
    choice_targets
        .get(&(node_id, 0))
        .and_then(|target_id| {
            node_lookup.get(target_id).map(|node| match node {
                StoryNode::Start => "start".to_string(),
                StoryNode::End => "__end".to_string(),
                _ => format!("node_{target_id}"),
            })
        })
        .unwrap_or_else(|| fallback_target.to_string())
}

fn target_label(
    node_id: u32,
    port: usize,
    node_lookup: &BTreeMap<u32, &StoryNode>,
    choice_targets: &BTreeMap<(u32, usize), u32>,
) -> String {
    choice_targets
        .get(&(node_id, port))
        .and_then(|target_id| {
            node_lookup.get(target_id).map(|node| match node {
                StoryNode::Start => "start".to_string(),
                StoryNode::End => "__end".to_string(),
                _ => format!("node_{target_id}"),
            })
        })
        .unwrap_or_else(|| format!("__unlinked_node_{node_id}_option_{port}"))
}

fn targets_end_label(event: &EventRaw) -> bool {
    match event {
        EventRaw::Jump { target } | EventRaw::JumpIf { target, .. } => target == "__end",
        EventRaw::Choice(choice) => choice.options.iter().any(|option| option.target == "__end"),
        _ => false,
    }
}

fn validate_strict_graph_export(graph: &NodeGraph) -> VnResult<()> {
    let node_lookup = graph
        .nodes()
        .map(|(id, node, _)| (*id, node))
        .collect::<BTreeMap<_, _>>();
    let connected_ports = graph
        .connections()
        .map(|conn| (conn.from, conn.from_port))
        .collect::<BTreeSet<_>>();

    for (node_id, node, _) in graph.nodes() {
        match node {
            StoryNode::Choice { options, .. } => {
                if options.is_empty() {
                    return Err(VnError::invalid_script(format!(
                        "choice node {node_id} has no options"
                    )));
                }
                for port in 0..options.len() {
                    if options[port].trim() == format!("Option {}", port + 1) {
                        return Err(VnError::invalid_script(format!(
                            "choice node {node_id} option {port} still uses placeholder text"
                        )));
                    }
                    if !connected_ports.contains(&(*node_id, port)) {
                        return Err(VnError::invalid_script(format!(
                            "choice node {node_id} option {port} has no target"
                        )));
                    }
                }
            }
            StoryNode::Jump { target } => {
                if target.trim().is_empty() {
                    return Err(VnError::invalid_script(format!(
                        "jump node {node_id} has empty target"
                    )));
                }
            }
            StoryNode::JumpIf { target, .. } => {
                let has_target_connection = connected_ports.contains(&(*node_id, 0));
                if target.trim().is_empty() && !has_target_connection {
                    return Err(VnError::invalid_script(format!(
                        "jump_if node {node_id} has empty target"
                    )));
                }
            }
            _ => {}
        }
    }

    for connection in graph.connections() {
        if !node_lookup.contains_key(&connection.from) || !node_lookup.contains_key(&connection.to)
        {
            return Err(VnError::invalid_script(format!(
                "connection {}:{} -> {} references a missing node",
                connection.from, connection.from_port, connection.to
            )));
        }
    }
    Ok(())
}
