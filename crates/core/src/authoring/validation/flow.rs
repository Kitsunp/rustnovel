use std::collections::BTreeSet;

use crate::authoring::NodeGraph;

pub(super) fn unreachable_blocker_context(
    graph: &NodeGraph,
    node_id: u32,
    reachable: &BTreeSet<u32>,
) -> (Option<u32>, String) {
    let incoming = graph.incoming_nodes(node_id);
    if incoming.is_empty() {
        return (
            None,
            "no incoming edges from any reachable path".to_string(),
        );
    }

    if let Some(from_id) = incoming
        .iter()
        .copied()
        .find(|candidate| reachable.contains(candidate))
    {
        return (
            Some(from_id),
            format!("reachable predecessor {from_id} cannot advance into this branch"),
        );
    }

    let incoming_summary = incoming
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    (
        incoming.first().copied(),
        format!("all predecessors are unreachable [{incoming_summary}]"),
    )
}
