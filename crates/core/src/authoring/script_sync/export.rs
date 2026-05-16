use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AudioActionRaw, ChoiceOptionRaw, ChoiceRaw, DialogueRaw, EventRaw, SceneTransitionRaw,
    SceneUpdateRaw, ScriptRaw, SetCharacterPositionRaw, VnError, VnResult,
};

use super::export_validation::validate_strict_graph_export;
use super::labels::append_fragment_labels;
use crate::authoring::{GraphFragment, NodeGraph, StoryNode};

pub fn to_script(graph: &NodeGraph) -> ScriptRaw {
    let mut events = Vec::new();
    let mut labels = BTreeMap::new();
    let mut node_event_indices = BTreeMap::new();
    let node_lookup = graph
        .nodes()
        .map(|(id, node, _)| (*id, node))
        .collect::<BTreeMap<_, _>>();
    let choice_targets = graph
        .connections()
        .map(|conn| ((conn.from, conn.from_port), conn.to))
        .collect::<BTreeMap<_, _>>();
    let fragment_node_ids = graph
        .fragments()
        .flat_map(|(_, fragment)| fragment.node_ids.iter().copied())
        .collect::<BTreeSet<_>>();

    for id in graph.script_order_node_ids() {
        if fragment_node_ids.contains(&id) {
            continue;
        }
        let Some(node) = node_lookup.get(&id).copied() else {
            continue;
        };
        if node.is_marker() {
            continue;
        }
        if let StoryNode::SubgraphCall {
            fragment_id,
            entry_port,
            ..
        } = node
        {
            emit_subgraph_call(
                graph,
                id,
                fragment_id,
                entry_port.as_deref(),
                &mut ExportBuildContext {
                    node_lookup: &node_lookup,
                    choice_targets: &choice_targets,
                    events: &mut events,
                    labels: &mut labels,
                },
            );
            node_event_indices.insert(id, labels.get(&format!("node_{id}")).copied().unwrap_or(0));
            continue;
        }
        let event_idx = events.len();
        labels.insert(format!("node_{id}"), event_idx);
        node_event_indices.insert(id, event_idx);
        if let Some(event) = event_from_node(
            id,
            node,
            &node_lookup,
            &choice_targets,
            ExportLabelContext::top(),
        ) {
            events.push(event);
        }
    }

    append_fragment_labels(graph, &node_event_indices, &mut labels);
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

fn event_from_node(
    id: u32,
    node: &StoryNode,
    node_lookup: &BTreeMap<u32, &StoryNode>,
    choice_targets: &BTreeMap<(u32, usize), u32>,
    label_context: ExportLabelContext<'_>,
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
                    target: target_label(id, port, node_lookup, choice_targets, label_context),
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
            target: jump_if_target_label(id, target, node_lookup, choice_targets, label_context),
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
        StoryNode::SubgraphCall { .. } => return None,
        StoryNode::Generic(event) => event.clone(),
        StoryNode::Start | StoryNode::End => return None,
    })
}

#[derive(Clone, Copy)]
struct ExportLabelContext<'a> {
    namespace: Option<&'a str>,
    fragment_nodes: Option<&'a BTreeSet<u32>>,
    continuation_label: Option<&'a str>,
}

impl<'a> ExportLabelContext<'a> {
    fn top() -> Self {
        Self {
            namespace: None,
            fragment_nodes: None,
            continuation_label: None,
        }
    }

    fn fragment(
        namespace: &'a str,
        fragment_nodes: &'a BTreeSet<u32>,
        continuation_label: Option<&'a str>,
    ) -> Self {
        Self {
            namespace: Some(namespace),
            fragment_nodes: Some(fragment_nodes),
            continuation_label,
        }
    }
}

struct ExportBuildContext<'a> {
    node_lookup: &'a BTreeMap<u32, &'a StoryNode>,
    choice_targets: &'a BTreeMap<(u32, usize), u32>,
    events: &'a mut Vec<EventRaw>,
    labels: &'a mut BTreeMap<String, usize>,
}

fn emit_subgraph_call(
    graph: &NodeGraph,
    call_node_id: u32,
    fragment_id: &str,
    entry_port: Option<&str>,
    build: &mut ExportBuildContext<'_>,
) {
    let Some(fragment) = graph.fragment(fragment_id) else {
        return;
    };
    let Some(entry_node_id) = fragment_entry_node(fragment, entry_port) else {
        return;
    };
    let namespace = format!("__call_{call_node_id}");
    let continuation_label = build
        .choice_targets
        .get(&(call_node_id, 0))
        .map(|target| format!("node_{target}"));
    let ordered_ids = fragment_order_node_ids(graph, &fragment.node_ids, entry_node_id);
    let fragment_nodes = fragment.node_ids.iter().copied().collect::<BTreeSet<_>>();
    build
        .labels
        .insert(format!("node_{call_node_id}"), build.events.len());
    build
        .labels
        .insert(format!("{namespace}_entry"), build.events.len());
    for node_id in &ordered_ids {
        build.labels.insert(
            namespaced_node_label(&namespace, *node_id),
            build.events.len(),
        );
        if let Some(port) = fragment
            .inputs
            .iter()
            .chain(fragment.outputs.iter())
            .find(|port| port.node_id == Some(*node_id))
        {
            build.labels.insert(
                namespaced_port_label(&namespace, &port.port_id),
                build.events.len(),
            );
        }
        let Some(node) = build.node_lookup.get(node_id).copied() else {
            continue;
        };
        if node.is_marker() {
            continue;
        }
        let context = ExportLabelContext::fragment(
            &namespace,
            &fragment_nodes,
            continuation_label.as_deref(),
        );
        if let Some(event) = event_from_node(
            *node_id,
            node,
            build.node_lookup,
            build.choice_targets,
            context,
        ) {
            build.events.push(event);
        }
    }
}

fn fragment_entry_node(fragment: &GraphFragment, entry_port: Option<&str>) -> Option<u32> {
    if let Some(port_id) = entry_port.filter(|value| !value.trim().is_empty()) {
        return fragment
            .inputs
            .iter()
            .find(|port| port.port_id == port_id)
            .and_then(|port| port.node_id);
    }
    fragment
        .inputs
        .first()
        .and_then(|port| port.node_id)
        .or_else(|| fragment.node_ids.first().copied())
}

fn fragment_order_node_ids(
    graph: &NodeGraph,
    fragment_node_ids: &[u32],
    entry_id: u32,
) -> Vec<u32> {
    let allowed = fragment_node_ids.iter().copied().collect::<BTreeSet<_>>();
    let mut ordered = Vec::new();
    let mut visited = BTreeSet::new();
    let mut queue = std::collections::VecDeque::from([entry_id]);
    while let Some(node_id) = queue.pop_front() {
        if !allowed.contains(&node_id) || !visited.insert(node_id) {
            continue;
        }
        ordered.push(node_id);
        let mut outgoing = graph
            .connections()
            .filter(|connection| connection.from == node_id && allowed.contains(&connection.to))
            .collect::<Vec<_>>();
        outgoing.sort_by_key(|connection| (connection.from_port, connection.to));
        for connection in outgoing {
            queue.push_back(connection.to);
        }
    }
    for node_id in fragment_node_ids {
        if !visited.contains(node_id) {
            ordered.push(*node_id);
        }
    }
    ordered
}

fn namespaced_node_label(namespace: &str, node_id: u32) -> String {
    format!("{namespace}_node_{node_id}")
}

fn namespaced_port_label(namespace: &str, port_id: &str) -> String {
    let port_token = port_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    format!("{namespace}_port_{}", port_token.trim_matches('_'))
}

fn jump_if_target_label(
    node_id: u32,
    fallback_target: &str,
    node_lookup: &BTreeMap<u32, &StoryNode>,
    choice_targets: &BTreeMap<(u32, usize), u32>,
    label_context: ExportLabelContext<'_>,
) -> String {
    choice_targets
        .get(&(node_id, 0))
        .and_then(|target_id| {
            node_lookup
                .get(target_id)
                .map(|node| node_target_label(*target_id, node, label_context))
        })
        .unwrap_or_else(|| fallback_target.to_string())
}

fn target_label(
    node_id: u32,
    port: usize,
    node_lookup: &BTreeMap<u32, &StoryNode>,
    choice_targets: &BTreeMap<(u32, usize), u32>,
    label_context: ExportLabelContext<'_>,
) -> String {
    choice_targets
        .get(&(node_id, port))
        .and_then(|target_id| {
            node_lookup
                .get(target_id)
                .map(|node| node_target_label(*target_id, node, label_context))
        })
        .unwrap_or_else(|| format!("__unlinked_node_{node_id}_option_{port}"))
}

fn node_target_label(
    target_id: u32,
    target_node: &StoryNode,
    label_context: ExportLabelContext<'_>,
) -> String {
    if let Some(namespace) = label_context.namespace {
        if label_context
            .fragment_nodes
            .is_some_and(|nodes| nodes.contains(&target_id))
        {
            return namespaced_node_label(namespace, target_id);
        }
        if let Some(continuation_label) = label_context.continuation_label {
            return continuation_label.to_string();
        }
    }
    match target_node {
        StoryNode::Start => "start".to_string(),
        StoryNode::End => "__end".to_string(),
        _ => format!("node_{target_id}"),
    }
}

fn targets_end_label(event: &EventRaw) -> bool {
    match event {
        EventRaw::Jump { target } | EventRaw::JumpIf { target, .. } => target == "__end",
        EventRaw::Choice(choice) => choice.options.iter().any(|option| option.target == "__end"),
        _ => false,
    }
}
