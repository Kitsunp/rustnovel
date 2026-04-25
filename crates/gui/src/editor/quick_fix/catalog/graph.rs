use std::collections::HashSet;

use crate::editor::{LintIssue, NodeGraph, StoryNode};
use eframe::egui;

use super::support::require_node_id;

pub(crate) fn apply_missing_start(
    graph: &mut NodeGraph,
    _issue: &LintIssue,
) -> Result<bool, String> {
    Ok(apply_add_missing_start(graph))
}

pub(crate) fn apply_dead_end(graph: &mut NodeGraph, issue: &LintIssue) -> Result<bool, String> {
    apply_connect_dead_end_to_end(
        graph,
        require_node_id(issue, "node_connect_dead_end_to_end")?,
    )
}

pub(crate) fn apply_choice_no_options(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_choice_add_default_option(graph, require_node_id(issue, "choice_add_default_option")?)
}

pub(crate) fn apply_choice_option_unlinked(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_choice_link_unlinked_to_end(
        graph,
        require_node_id(issue, "choice_link_unlinked_to_end")?,
    )
}

pub(crate) fn apply_choice_port_out_of_range(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_choice_expand_options_to_ports(
        graph,
        require_node_id(issue, "choice_expand_options_to_ports")?,
    )
}

pub(crate) fn apply_empty_speaker(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_fill_speaker(graph, require_node_id(issue, "dialogue_fill_speaker")?)
}

pub(crate) fn apply_empty_jump_target(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_set_jump_target_start(graph, require_node_id(issue, "jump_set_start_target")?)
}

pub(crate) fn apply_invalid_transition_kind(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_set_transition_kind_fade(graph, require_node_id(issue, "transition_set_fade")?)
}

pub(crate) fn apply_invalid_transition_duration(
    graph: &mut NodeGraph,
    issue: &LintIssue,
) -> Result<bool, String> {
    apply_set_transition_duration(
        graph,
        require_node_id(issue, "transition_set_default_duration")?,
    )
}

fn apply_add_missing_start(graph: &mut NodeGraph) -> bool {
    if graph
        .nodes()
        .any(|(_, node, _)| matches!(node, StoryNode::Start))
    {
        return false;
    }
    graph.add_node(StoryNode::Start, egui::pos2(50.0, 30.0));
    true
}

fn apply_connect_dead_end_to_end(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(node) = graph.get_node(node_id) else {
        return Err(format!("node_id {node_id} not found"));
    };
    if matches!(node, StoryNode::End) {
        return Ok(false);
    }
    if graph.connections().any(|c| c.from == node_id) {
        return Ok(false);
    }
    let end_id = ensure_end_node(graph, node_id)?;
    graph.connect(node_id, end_id);
    Ok(true)
}

fn apply_choice_add_default_option(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::Choice { options, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Choice"));
    };
    if !options.is_empty() {
        return Ok(false);
    }
    options.push("Option 1".to_string());
    graph.mark_modified();
    Ok(true)
}

fn apply_choice_link_unlinked_to_end(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let options_len = match graph.get_node(node_id) {
        Some(StoryNode::Choice { options, .. }) => options.len(),
        _ => return Err(format!("node_id {node_id} is not Choice")),
    };
    if options_len == 0 {
        return Ok(false);
    }
    let linked_ports: HashSet<usize> = graph
        .connections()
        .filter(|conn| conn.from == node_id)
        .map(|conn| conn.from_port)
        .collect();
    let unlinked: Vec<usize> = (0..options_len)
        .filter(|idx| !linked_ports.contains(idx))
        .collect();
    if unlinked.is_empty() {
        return Ok(false);
    }
    let end_id = ensure_end_node(graph, node_id)?;
    for port in unlinked {
        graph.connect_port(node_id, port, end_id);
    }
    Ok(true)
}

fn apply_choice_expand_options_to_ports(
    graph: &mut NodeGraph,
    node_id: u32,
) -> Result<bool, String> {
    let options_len = match graph.get_node(node_id) {
        Some(StoryNode::Choice { options, .. }) => options.len(),
        _ => return Err(format!("node_id {node_id} is not Choice")),
    };
    let max_port = graph
        .connections()
        .filter(|conn| conn.from == node_id)
        .map(|conn| conn.from_port)
        .max()
        .unwrap_or(0);

    let Some(StoryNode::Choice { options, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Choice"));
    };
    let before = options_len;
    while options.len() <= max_port {
        let next = options.len() + 1;
        options.push(format!("Option {next}"));
    }
    if options.len() != before {
        graph.mark_modified();
        return Ok(true);
    }
    Ok(false)
}

fn apply_fill_speaker(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::Dialogue { speaker, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Dialogue"));
    };
    if !speaker.trim().is_empty() {
        return Ok(false);
    }
    *speaker = "Narrator".to_string();
    graph.mark_modified();
    Ok(true)
}

fn apply_set_jump_target_start(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    if !graph
        .nodes()
        .any(|(_, node, _)| matches!(node, StoryNode::Start))
    {
        return Err("cannot set jump target to start: no Start node exists".to_string());
    }
    let Some(node) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} not found"));
    };
    match node {
        StoryNode::Jump { target } | StoryNode::JumpIf { target, .. } => {
            if target.trim().is_empty() {
                *target = "start".to_string();
                graph.mark_modified();
                Ok(true)
            } else {
                Ok(false)
            }
        }
        _ => Err(format!("node_id {node_id} is not Jump/JumpIf")),
    }
}

fn apply_set_transition_kind_fade(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::Transition { kind, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Transition"));
    };
    let normalized = kind.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "fade" | "fade_black" | "dissolve" | "cut"
    ) {
        return Ok(false);
    }
    *kind = "fade".to_string();
    graph.mark_modified();
    Ok(true)
}

fn apply_set_transition_duration(graph: &mut NodeGraph, node_id: u32) -> Result<bool, String> {
    let Some(StoryNode::Transition { duration_ms, .. }) = graph.get_node_mut(node_id) else {
        return Err(format!("node_id {node_id} is not Transition"));
    };
    if *duration_ms > 0 {
        return Ok(false);
    }
    *duration_ms = 300;
    graph.mark_modified();
    Ok(true)
}

fn ensure_end_node(graph: &mut NodeGraph, source_node_id: u32) -> Result<u32, String> {
    if let Some((id, _, _)) = graph
        .nodes()
        .find(|(_, node, _)| matches!(node, StoryNode::End))
        .cloned()
    {
        return Ok(id);
    }

    let source_pos = graph
        .nodes()
        .find(|(id, _, _)| *id == source_node_id)
        .map(|(_, _, pos)| *pos)
        .ok_or_else(|| format!("source node {source_node_id} not found"))?;
    Ok(graph.add_node(
        StoryNode::End,
        egui::pos2(source_pos.x + 140.0, source_pos.y + 120.0),
    ))
}
