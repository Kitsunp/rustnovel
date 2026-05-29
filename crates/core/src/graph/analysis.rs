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
    let mut node_set = BTreeSet::new();
    for (from, targets) in adjacency {
        node_set.insert(*from);
        node_set.extend(targets.iter().copied());
    }
    node_set.extend(start_nodes.iter().copied());
    let reachable = reachable_nodes(&node_set, adjacency, start_nodes);
    let reverse = reverse_adjacency(adjacency);
    let finish_order = finish_order_iterative(adjacency, &reachable);
    let mut seen = BTreeSet::new();
    let mut cycle_nodes = BTreeSet::new();

    for node in finish_order.into_iter().rev() {
        if !reachable.contains(&node) || !seen.insert(node) {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            component.push(current);
            if let Some(targets) = reverse.get(&current) {
                for target in targets {
                    if reachable.contains(target) && seen.insert(*target) {
                        stack.push(*target);
                    }
                }
            }
        }
        let self_loop = component.iter().any(|member| {
            adjacency
                .get(member)
                .is_some_and(|targets| targets.contains(member))
        });
        if component.len() > 1 || self_loop {
            cycle_nodes.extend(component);
        }
    }

    cycle_nodes.into_iter().collect()
}

fn reverse_adjacency(adjacency: &BTreeMap<NodeId, Vec<NodeId>>) -> BTreeMap<NodeId, Vec<NodeId>> {
    let mut reverse = BTreeMap::<NodeId, Vec<NodeId>>::new();
    for (from, targets) in adjacency {
        reverse.entry(*from).or_default();
        for target in targets {
            reverse.entry(*target).or_default().push(*from);
        }
    }
    for targets in reverse.values_mut() {
        targets.sort_unstable();
        targets.dedup();
    }
    reverse
}

fn finish_order_iterative(
    adjacency: &BTreeMap<NodeId, Vec<NodeId>>,
    reachable: &BTreeSet<NodeId>,
) -> Vec<NodeId> {
    let mut visited = BTreeSet::new();
    let mut order = Vec::new();
    for start in reachable {
        if visited.contains(start) {
            continue;
        }
        let mut stack = vec![(*start, false)];
        while let Some((node, expanded)) = stack.pop() {
            if expanded {
                order.push(node);
                continue;
            }
            if !visited.insert(node) {
                continue;
            }
            stack.push((node, true));
            if let Some(targets) = adjacency.get(&node) {
                for target in targets.iter().rev() {
                    if reachable.contains(target) && !visited.contains(target) {
                        stack.push((*target, false));
                    }
                }
            }
        }
    }
    order
}
