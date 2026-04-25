use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::NodeId;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlowGraphAnalysis {
    pub reachable: BTreeSet<NodeId>,
    pub unreachable: Vec<NodeId>,
    pub reachable_cycle_nodes: Vec<NodeId>,
}

pub fn analyze_flow_graph(
    nodes: &[NodeId],
    edges: &[(NodeId, NodeId)],
    start_nodes: &[NodeId],
) -> FlowGraphAnalysis {
    let node_set = nodes.iter().copied().collect::<BTreeSet<_>>();
    let adjacency = adjacency_map(edges);
    let reachable = reachable_nodes(&node_set, &adjacency, start_nodes);
    let unreachable = node_set
        .iter()
        .copied()
        .filter(|node| !reachable.contains(node))
        .collect::<Vec<_>>();
    let reachable_cycle_nodes = detect_reachable_cycle_nodes(&adjacency, start_nodes);

    FlowGraphAnalysis {
        reachable,
        unreachable,
        reachable_cycle_nodes,
    }
}

fn adjacency_map(edges: &[(NodeId, NodeId)]) -> BTreeMap<NodeId, Vec<NodeId>> {
    let mut adjacency = BTreeMap::<NodeId, Vec<NodeId>>::new();
    for (from, to) in edges {
        adjacency.entry(*from).or_default().push(*to);
    }
    for targets in adjacency.values_mut() {
        targets.sort_unstable();
        targets.dedup();
    }
    adjacency
}

fn reachable_nodes(
    node_set: &BTreeSet<NodeId>,
    adjacency: &BTreeMap<NodeId, Vec<NodeId>>,
    start_nodes: &[NodeId],
) -> BTreeSet<NodeId> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();

    for start in start_nodes {
        if node_set.contains(start) && visited.insert(*start) {
            queue.push_back(*start);
        }
    }

    while let Some(node_id) = queue.pop_front() {
        if let Some(targets) = adjacency.get(&node_id) {
            for target in targets {
                if node_set.contains(target) && visited.insert(*target) {
                    queue.push_back(*target);
                }
            }
        }
    }

    visited
}

fn detect_reachable_cycle_nodes(
    adjacency: &BTreeMap<NodeId, Vec<NodeId>>,
    start_nodes: &[NodeId],
) -> Vec<NodeId> {
    let mut visited = BTreeSet::new();
    let mut active = BTreeSet::new();
    let mut cycle_nodes = BTreeSet::new();

    for start in start_nodes {
        detect_cycles_from(
            adjacency,
            *start,
            &mut visited,
            &mut active,
            &mut cycle_nodes,
        );
    }

    cycle_nodes.into_iter().collect()
}

fn detect_cycles_from(
    adjacency: &BTreeMap<NodeId, Vec<NodeId>>,
    node_id: NodeId,
    visited: &mut BTreeSet<NodeId>,
    active: &mut BTreeSet<NodeId>,
    cycle_nodes: &mut BTreeSet<NodeId>,
) {
    if active.contains(&node_id) {
        cycle_nodes.insert(node_id);
        return;
    }
    if !visited.insert(node_id) {
        return;
    }

    active.insert(node_id);
    if let Some(targets) = adjacency.get(&node_id) {
        for target in targets {
            if active.contains(target) {
                cycle_nodes.insert(node_id);
                cycle_nodes.insert(*target);
                continue;
            }
            detect_cycles_from(adjacency, *target, visited, active, cycle_nodes);
            if cycle_nodes.contains(target) {
                cycle_nodes.insert(node_id);
            }
        }
    }
    active.remove(&node_id);
}
