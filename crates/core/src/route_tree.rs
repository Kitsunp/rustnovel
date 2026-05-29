use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::event::EventCompiled;
use crate::script::ScriptCompiled;
use crate::visual::VisualState;

pub type RouteNodeId = u32;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadModelSnapshot {
    pub visited_dialogue_ips: BTreeSet<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteProgressSnapshot {
    pub current_ip: u32,
    pub visited_ips: BTreeSet<u32>,
    pub selected_choices: Vec<ChoiceProgressSnapshot>,
    pub reached_endings: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChoiceProgressSnapshot {
    pub event_ip: u32,
    pub prompt: String,
    pub option_index: usize,
    pub option_text: String,
    pub target_ip: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteTree {
    pub root: RouteNodeId,
    pub current: Option<RouteNodeId>,
    pub nodes: Vec<RouteNode>,
    pub edges: Vec<RouteEdge>,
    pub coverage: RouteCoverage,
    pub progress: RouteProgressSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteNode {
    pub id: RouteNodeId,
    pub ip: u32,
    pub label: Option<String>,
    pub kind: RouteNodeKind,
    pub visited: bool,
    pub current: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteNodeKind {
    Dialogue,
    Choice,
    Scene,
    Jump,
    Conditional,
    Ending,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteEdge {
    pub from: RouteNodeId,
    pub to: Option<RouteNodeId>,
    pub kind: RouteEdgeKind,
    pub label: Option<String>,
    pub condition: Option<String>,
    pub selected: bool,
    pub discovered: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteEdgeKind {
    Sequential,
    Choice,
    Jump,
    Conditional,
    Ending,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteCoverage {
    pub total_nodes: usize,
    pub visited_nodes: usize,
    pub total_endings: usize,
    pub reached_endings: usize,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualResolveStrategy {
    LinearReplay,
}

pub fn build_route_tree(script: &ScriptCompiled) -> RouteTree {
    build_route_tree_with_progress(script, RouteProgressSnapshot::default())
}

pub fn build_route_tree_with_progress(
    script: &ScriptCompiled,
    progress: RouteProgressSnapshot,
) -> RouteTree {
    let label_by_ip = labels_by_ip(script);
    let selected = selected_choice_edges(&progress);
    let mut nodes = Vec::with_capacity(script.events.len());
    let mut edges = Vec::new();

    for (ip, event) in script.events.iter().enumerate() {
        let ip = ip as u32;
        let visited = progress.visited_ips.contains(&ip);
        let current = progress.current_ip == ip;
        nodes.push(RouteNode {
            id: ip,
            ip,
            label: label_by_ip.get(&ip).cloned(),
            kind: node_kind(event),
            visited,
            current,
        });
        push_edges_for_event(script, ip, event, &progress, &selected, &mut edges);
    }

    let visited_nodes = nodes.iter().filter(|node| node.visited).count();
    let total_endings = edges
        .iter()
        .filter(|edge| edge.kind == RouteEdgeKind::Ending)
        .count();
    let reached_endings = progress.reached_endings.len();
    RouteTree {
        root: script.start_ip,
        current: (progress.current_ip < script.events.len() as u32).then_some(progress.current_ip),
        coverage: RouteCoverage {
            total_nodes: nodes.len(),
            visited_nodes,
            total_endings,
            reached_endings,
        },
        nodes,
        edges,
        progress,
    }
}

pub fn resolve_visual_at_ip(
    script: &ScriptCompiled,
    ip: u32,
    strategy: VisualResolveStrategy,
) -> VisualState {
    match strategy {
        VisualResolveStrategy::LinearReplay => resolve_visual_linear(script, ip),
    }
}

fn resolve_visual_linear(script: &ScriptCompiled, ip: u32) -> VisualState {
    let mut visual = VisualState::default();
    let end = (ip as usize).min(script.events.len().saturating_sub(1));
    for event in script.events.iter().take(end.saturating_add(1)) {
        match event {
            EventCompiled::Scene(scene) => visual.apply_scene(scene),
            EventCompiled::Patch(patch) => visual.apply_patch(patch),
            EventCompiled::SetCharacterPosition(pos)
                if visual.set_character_position(pos).is_err() =>
            {
                break;
            }
            EventCompiled::SetCharacterPosition(_) => {}
            _ => {}
        }
    }
    visual
}

fn push_edges_for_event(
    script: &ScriptCompiled,
    ip: u32,
    event: &EventCompiled,
    progress: &RouteProgressSnapshot,
    selected: &BTreeSet<(u32, usize)>,
    edges: &mut Vec<RouteEdge>,
) {
    match event {
        EventCompiled::Choice(choice) => {
            for (option_index, option) in choice.options.iter().enumerate() {
                let target = valid_target(script, option.target_ip);
                edges.push(RouteEdge {
                    from: ip,
                    to: target,
                    kind: RouteEdgeKind::Choice,
                    label: Some(option.text.as_ref().to_string()),
                    condition: None,
                    selected: selected.contains(&(ip, option_index)),
                    discovered: progress.visited_ips.contains(&ip)
                        || target.is_some_and(|target| progress.visited_ips.contains(&target)),
                });
            }
        }
        EventCompiled::Jump { target_ip } => edges.push(RouteEdge {
            from: ip,
            to: valid_target(script, *target_ip),
            kind: RouteEdgeKind::Jump,
            label: None,
            condition: None,
            selected: progress.visited_ips.contains(&ip),
            discovered: progress.visited_ips.contains(&ip),
        }),
        EventCompiled::JumpIf { cond, target_ip } => {
            edges.push(RouteEdge {
                from: ip,
                to: valid_target(script, *target_ip),
                kind: RouteEdgeKind::Conditional,
                label: Some("true".to_string()),
                condition: serde_json::to_string(cond).ok(),
                selected: false,
                discovered: progress.visited_ips.contains(&ip),
            });
            edges.push(RouteEdge {
                from: ip,
                to: next_ip(script, ip),
                kind: RouteEdgeKind::Conditional,
                label: Some("false".to_string()),
                condition: serde_json::to_string(cond).ok(),
                selected: false,
                discovered: progress.visited_ips.contains(&ip),
            });
        }
        _ => {
            let to = next_ip(script, ip);
            edges.push(RouteEdge {
                from: ip,
                to,
                kind: to.map_or(RouteEdgeKind::Ending, |_| RouteEdgeKind::Sequential),
                label: None,
                condition: None,
                selected: progress.visited_ips.contains(&ip),
                discovered: progress.visited_ips.contains(&ip),
            });
        }
    }
}

fn node_kind(event: &EventCompiled) -> RouteNodeKind {
    match event {
        EventCompiled::Dialogue(_) => RouteNodeKind::Dialogue,
        EventCompiled::Choice(_) => RouteNodeKind::Choice,
        EventCompiled::Scene(_)
        | EventCompiled::Patch(_)
        | EventCompiled::SetCharacterPosition(_) => RouteNodeKind::Scene,
        EventCompiled::Jump { .. } => RouteNodeKind::Jump,
        EventCompiled::JumpIf { .. } => RouteNodeKind::Conditional,
        _ => RouteNodeKind::System,
    }
}

fn labels_by_ip(script: &ScriptCompiled) -> BTreeMap<u32, String> {
    let mut labels = BTreeMap::new();
    for (label, ip) in &script.labels {
        labels.entry(*ip).or_insert_with(|| label.clone());
    }
    labels
}

fn selected_choice_edges(progress: &RouteProgressSnapshot) -> BTreeSet<(u32, usize)> {
    progress
        .selected_choices
        .iter()
        .map(|choice| (choice.event_ip, choice.option_index))
        .collect()
}

fn next_ip(script: &ScriptCompiled, ip: u32) -> Option<u32> {
    valid_target(script, ip.saturating_add(1))
}

fn valid_target(script: &ScriptCompiled, ip: u32) -> Option<u32> {
    (ip < script.events.len() as u32).then_some(ip)
}
