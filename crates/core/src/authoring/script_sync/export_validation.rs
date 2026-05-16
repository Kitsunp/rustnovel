use std::collections::{BTreeMap, BTreeSet};

use crate::authoring::{LintSeverity, NodeGraph, StoryNode};
use crate::{VnError, VnResult};

use super::export::to_script_lossy_for_diagnostics;

pub(super) fn validate_strict_graph_export(graph: &NodeGraph) -> VnResult<()> {
    let node_lookup = graph
        .nodes()
        .map(|(id, node, _)| (*id, node))
        .collect::<BTreeMap<_, _>>();
    let start_nodes = graph
        .nodes()
        .filter_map(|(id, node, _)| matches!(node, StoryNode::Start).then_some(*id))
        .collect::<Vec<_>>();
    let flow = graph.flow_analysis(&start_nodes);
    let script_labels = to_script_lossy_for_diagnostics(graph)
        .labels
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let connected_ports = graph
        .connections()
        .map(|conn| (conn.from, conn.from_port))
        .collect::<BTreeSet<_>>();
    let fragment_node_ids = graph
        .fragments()
        .flat_map(|(_, fragment)| fragment.node_ids.iter().copied())
        .collect::<BTreeSet<_>>();

    for (node_id, node, _) in graph.nodes() {
        validate_reachability(*node_id, node, &fragment_node_ids, &flow.reachable)?;
        if !node.is_marker() && !node.export_supported() {
            return Err(VnError::invalid_script(format!(
                "node {node_id} is not export-supported"
            )));
        }
        validate_node_for_strict_export(graph, *node_id, node, &script_labels, &connected_ports)?;
    }

    validate_fragment_issues(graph)?;
    validate_connections_exist(graph, &node_lookup)
}

fn validate_reachability(
    node_id: u32,
    node: &StoryNode,
    fragment_node_ids: &BTreeSet<u32>,
    reachable: &BTreeSet<u32>,
) -> VnResult<()> {
    if !node.is_marker() && !fragment_node_ids.contains(&node_id) && !reachable.contains(&node_id) {
        return Err(VnError::invalid_script(format!(
            "node {node_id} is unreachable/draft and cannot be exported in strict mode"
        )));
    }
    Ok(())
}

fn validate_fragment_issues(graph: &NodeGraph) -> VnResult<()> {
    for issue in graph.validate_fragments() {
        if issue.severity == LintSeverity::Error {
            return Err(VnError::invalid_script(format!(
                "fragment validation failed: {}",
                issue.message
            )));
        }
    }
    Ok(())
}

fn validate_connections_exist(
    graph: &NodeGraph,
    node_lookup: &BTreeMap<u32, &StoryNode>,
) -> VnResult<()> {
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

fn validate_node_for_strict_export(
    graph: &NodeGraph,
    node_id: u32,
    node: &StoryNode,
    script_labels: &BTreeSet<String>,
    connected_ports: &BTreeSet<(u32, usize)>,
) -> VnResult<()> {
    match node {
        StoryNode::Choice { options, .. } => validate_choice(node_id, options, connected_ports),
        StoryNode::Jump { target } => validate_jump(node_id, target, script_labels),
        StoryNode::JumpIf { target, .. } => {
            validate_jump_if(node_id, target, script_labels, connected_ports)
        }
        StoryNode::SubgraphCall {
            fragment_id,
            entry_port,
            exit_port,
        } => validate_subgraph_call(graph, node_id, fragment_id, entry_port, exit_port),
        _ => Ok(()),
    }
}

fn validate_choice(
    node_id: u32,
    options: &[String],
    connected_ports: &BTreeSet<(u32, usize)>,
) -> VnResult<()> {
    if options.is_empty() {
        return Err(VnError::invalid_script(format!(
            "choice node {node_id} has no options"
        )));
    }
    for (port, option) in options.iter().enumerate() {
        if option.trim() == format!("Option {}", port + 1) {
            return Err(VnError::invalid_script(format!(
                "choice node {node_id} option {port} still uses placeholder text"
            )));
        }
        if !connected_ports.contains(&(node_id, port)) {
            return Err(VnError::invalid_script(format!(
                "choice node {node_id} option {port} has no target"
            )));
        }
    }
    Ok(())
}

fn validate_jump(node_id: u32, target: &str, script_labels: &BTreeSet<String>) -> VnResult<()> {
    if target.trim().is_empty() {
        return Err(VnError::invalid_script(format!(
            "jump node {node_id} has empty target"
        )));
    }
    if !script_labels.contains(target.trim()) {
        return Err(VnError::invalid_script(format!(
            "jump node {node_id} points to missing target '{}'",
            target.trim()
        )));
    }
    Ok(())
}

fn validate_jump_if(
    node_id: u32,
    target: &str,
    script_labels: &BTreeSet<String>,
    connected_ports: &BTreeSet<(u32, usize)>,
) -> VnResult<()> {
    let has_target_connection = connected_ports.contains(&(node_id, 0));
    if target.trim().is_empty() && !has_target_connection {
        return Err(VnError::invalid_script(format!(
            "jump_if node {node_id} has empty target"
        )));
    }
    if !has_target_connection && !target.trim().is_empty() && !script_labels.contains(target.trim())
    {
        return Err(VnError::invalid_script(format!(
            "jump_if node {node_id} points to missing target '{}'",
            target.trim()
        )));
    }
    Ok(())
}

fn validate_subgraph_call(
    graph: &NodeGraph,
    node_id: u32,
    fragment_id: &str,
    entry_port: &Option<String>,
    exit_port: &Option<String>,
) -> VnResult<()> {
    let fragment = graph.fragment(fragment_id).ok_or_else(|| {
        VnError::invalid_script(format!(
            "subgraph call node {node_id} references missing fragment '{fragment_id}'"
        ))
    })?;
    if entry_port.as_deref().is_some_and(|port| {
        !fragment
            .inputs
            .iter()
            .any(|candidate| candidate.port_id == port)
    }) {
        return Err(VnError::invalid_script(format!(
            "subgraph call node {node_id} references missing entry port"
        )));
    }
    if exit_port.as_deref().is_some_and(|port| {
        !fragment
            .outputs
            .iter()
            .any(|candidate| candidate.port_id == port)
    }) {
        return Err(VnError::invalid_script(format!(
            "subgraph call node {node_id} references missing exit port"
        )));
    }
    Ok(())
}
