use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::super::{
    validate_authoring_graph_no_io, AuthoringPosition, DiagnosticTarget, FieldPath,
    GraphConnection, LintCode, LintIssue, NodeGraph, SemanticValue, SemanticValueKind, StoryNode,
    ValidationPhase,
};
use super::AuthoringDelta;

#[derive(Clone, Debug, Default)]
pub(super) struct CommandBusDiagnosticState {
    ids: BTreeSet<String>,
    graph_ids: BTreeSet<String>,
    node_ids: BTreeMap<u32, BTreeSet<String>>,
    start_nodes: BTreeSet<u32>,
}

impl CommandBusDiagnosticState {
    pub(super) fn from_graph(graph: &NodeGraph) -> Self {
        let mut state = Self {
            ids: BTreeSet::new(),
            graph_ids: BTreeSet::new(),
            node_ids: BTreeMap::new(),
            start_nodes: graph
                .nodes()
                .filter_map(|(id, node, _)| matches!(node, StoryNode::Start).then_some(*id))
                .collect(),
        };
        for issue in validate_authoring_graph_no_io(graph) {
            let id = issue.diagnostic_id();
            if let Some(node_id) = issue.node_id {
                state
                    .node_ids
                    .entry(node_id)
                    .or_default()
                    .insert(id.clone());
            } else {
                state.graph_ids.insert(id.clone());
            }
            state.ids.insert(id);
        }
        state
    }

    pub(super) fn ids(&self) -> BTreeSet<String> {
        self.ids.clone()
    }

    pub(super) fn apply_delta(&mut self, graph: &NodeGraph, delta: &AuthoringDelta) {
        match delta {
            AuthoringDelta::NodeCreated { node_id, node, .. } => {
                if matches!(node, StoryNode::Start) {
                    self.start_nodes.insert(*node_id);
                    self.replace_graph_ids();
                }
                self.replace_node_ids(graph, *node_id);
            }
            AuthoringDelta::NodeRemoved {
                node_id,
                node,
                connections,
                ..
            } => {
                self.remove_node_ids(*node_id);
                if matches!(node, StoryNode::Start) {
                    self.start_nodes.remove(node_id);
                    self.replace_graph_ids();
                }
                for connection in connections {
                    self.replace_node_ids(graph, connection.from);
                    self.replace_node_ids(graph, connection.to);
                }
            }
            AuthoringDelta::Connected {
                connection,
                replaced,
            } => {
                for connection in replaced {
                    self.replace_node_ids(graph, connection.from);
                    self.replace_node_ids(graph, connection.to);
                }
                self.replace_node_ids(graph, connection.from);
                self.replace_node_ids(graph, connection.to);
            }
            AuthoringDelta::Disconnected { removed } => {
                for connection in removed {
                    self.replace_node_ids(graph, connection.from);
                    self.replace_node_ids(graph, connection.to);
                }
            }
            AuthoringDelta::ChoiceOptionConnected {
                choice_id,
                connection,
                ..
            } => {
                self.replace_node_ids(graph, *choice_id);
                self.replace_node_ids(graph, connection.to);
            }
            AuthoringDelta::ChoiceOptionTargetSet {
                node_id,
                before,
                after,
                ..
            } => {
                self.replace_node_ids(graph, *node_id);
                for connection in before {
                    self.replace_node_ids(graph, connection.to);
                }
                if let Some(connection) = after {
                    self.replace_node_ids(graph, connection.to);
                }
            }
            AuthoringDelta::BranchConnected { from, to, .. } => {
                self.reconcile_node_ids(graph);
                self.replace_node_ids(graph, *from);
                self.replace_node_ids(graph, *to);
            }
            AuthoringDelta::NodeEdited {
                node_id,
                before,
                after,
            } => {
                if matches!(before, StoryNode::Start) || matches!(after, StoryNode::Start) {
                    self.start_nodes = graph
                        .nodes()
                        .filter_map(|(id, node, _)| matches!(node, StoryNode::Start).then_some(*id))
                        .collect();
                    self.replace_graph_ids();
                }
                self.replace_node_ids(graph, *node_id);
            }
            AuthoringDelta::FragmentCreated { .. }
            | AuthoringDelta::FragmentRemoved { .. }
            | AuthoringDelta::FragmentPortsRefreshed { .. }
            | AuthoringDelta::QuickFixApplied { .. } => self.reconcile_node_ids(graph),
            AuthoringDelta::FragmentEntered { .. }
            | AuthoringDelta::FragmentLeft { .. }
            | AuthoringDelta::AssetImported { .. } => {}
            AuthoringDelta::ChoiceOptionReordered {
                node_id,
                before_connections,
                after_connections,
                ..
            }
            | AuthoringDelta::ChoiceOptionRemoved {
                node_id,
                before_connections,
                after_connections,
                ..
            } => {
                self.replace_node_ids(graph, *node_id);
                for connection in before_connections.iter().chain(after_connections) {
                    self.replace_node_ids(graph, connection.to);
                }
            }
            AuthoringDelta::LayerMoved { object_id, .. } => {
                if let Some(node_id) = node_id_from_character_object_id(object_id) {
                    self.replace_node_ids(graph, node_id);
                }
            }
            AuthoringDelta::Reverted { reverted } => self.apply_reverted_delta(graph, reverted),
        }
    }

    pub(super) fn apply_reverted_delta(&mut self, graph: &NodeGraph, delta: &AuthoringDelta) {
        match delta {
            AuthoringDelta::NodeCreated { node_id, node, .. } => {
                self.remove_node_ids(*node_id);
                if matches!(node, StoryNode::Start) {
                    self.start_nodes.remove(node_id);
                    self.replace_graph_ids();
                }
            }
            AuthoringDelta::NodeRemoved {
                node_id,
                node,
                connections,
                ..
            } => {
                if matches!(node, StoryNode::Start) {
                    self.start_nodes.insert(*node_id);
                    self.replace_graph_ids();
                }
                self.replace_node_ids(graph, *node_id);
                for connection in connections {
                    self.replace_node_ids(graph, connection.from);
                    self.replace_node_ids(graph, connection.to);
                }
            }
            AuthoringDelta::Connected {
                connection,
                replaced,
            } => {
                self.replace_node_ids(graph, connection.from);
                self.replace_node_ids(graph, connection.to);
                for connection in replaced {
                    self.replace_node_ids(graph, connection.from);
                    self.replace_node_ids(graph, connection.to);
                }
            }
            AuthoringDelta::Disconnected { removed } => {
                for connection in removed {
                    self.replace_node_ids(graph, connection.from);
                    self.replace_node_ids(graph, connection.to);
                }
            }
            AuthoringDelta::ChoiceOptionConnected {
                choice_id,
                connection,
                ..
            } => {
                self.replace_node_ids(graph, *choice_id);
                self.replace_node_ids(graph, connection.to);
            }
            AuthoringDelta::ChoiceOptionTargetSet {
                node_id,
                before,
                after,
                ..
            } => {
                self.replace_node_ids(graph, *node_id);
                for connection in before {
                    self.replace_node_ids(graph, connection.to);
                }
                if let Some(connection) = after {
                    self.replace_node_ids(graph, connection.to);
                }
            }
            AuthoringDelta::BranchConnected { .. }
            | AuthoringDelta::FragmentCreated { .. }
            | AuthoringDelta::FragmentRemoved { .. }
            | AuthoringDelta::FragmentPortsRefreshed { .. }
            | AuthoringDelta::QuickFixApplied { .. }
            | AuthoringDelta::ChoiceOptionReordered { .. }
            | AuthoringDelta::ChoiceOptionRemoved { .. } => self.reconcile_node_ids(graph),
            AuthoringDelta::NodeEdited { node_id, .. } => self.replace_node_ids(graph, *node_id),
            AuthoringDelta::FragmentEntered { .. }
            | AuthoringDelta::FragmentLeft { .. }
            | AuthoringDelta::AssetImported { .. } => {}
            AuthoringDelta::LayerMoved { object_id, .. } => {
                if let Some(node_id) = node_id_from_character_object_id(object_id) {
                    self.replace_node_ids(graph, node_id);
                }
            }
            AuthoringDelta::Reverted { reverted } => self.apply_delta(graph, reverted),
        }
    }

    fn replace_graph_ids(&mut self) {
        for id in &self.graph_ids {
            self.ids.remove(id);
        }
        self.graph_ids.clear();
        match self.start_nodes.len() {
            0 => self.insert_graph_issue(
                LintIssue::error(
                    None,
                    ValidationPhase::Graph,
                    LintCode::MissingStart,
                    "Missing Start node",
                )
                .with_target(DiagnosticTarget::Graph)
                .with_evidence_trace(),
            ),
            1 => {}
            count => self.insert_graph_issue(
                LintIssue::error(
                    None,
                    ValidationPhase::Graph,
                    LintCode::MultipleStart,
                    format!("Multiple Start nodes found ({count})"),
                )
                .with_target(DiagnosticTarget::Graph)
                .with_evidence_trace(),
            ),
        }
    }

    fn insert_graph_issue(&mut self, issue: LintIssue) {
        let id = issue.diagnostic_id();
        self.graph_ids.insert(id.clone());
        self.ids.insert(id);
    }

    fn reconcile_node_ids(&mut self, graph: &NodeGraph) {
        let current_ids = graph.nodes().map(|(id, _, _)| *id).collect::<BTreeSet<_>>();
        let previous_ids = self.node_ids.keys().copied().collect::<Vec<_>>();
        for node_id in previous_ids {
            if !current_ids.contains(&node_id) {
                self.remove_node_ids(node_id);
            }
        }
        for node_id in current_ids {
            self.replace_node_ids(graph, node_id);
        }
    }

    fn replace_node_ids(&mut self, graph: &NodeGraph, node_id: u32) {
        self.remove_node_ids(node_id);
        let Some((node, position)) = graph.get_node(node_id).zip(graph.get_node_pos(node_id))
        else {
            return;
        };
        let ids = node_diagnostic_ids(graph, node_id, node, position);
        for id in &ids {
            self.ids.insert(id.clone());
        }
        self.node_ids.insert(node_id, ids);
    }

    fn remove_node_ids(&mut self, node_id: u32) {
        if let Some(ids) = self.node_ids.remove(&node_id) {
            for id in ids {
                self.ids.remove(&id);
            }
        }
    }
}

fn node_diagnostic_ids(
    graph: &NodeGraph,
    node_id: u32,
    node: &StoryNode,
    position: AuthoringPosition,
) -> BTreeSet<String> {
    let mut issues = Vec::new();
    validate_layout_position(node_id, position, &mut issues);
    validate_reachability(graph, node_id, &mut issues);
    if !node.is_marker() && !node.export_supported() {
        issues.push(
            LintIssue::error(
                Some(node_id),
                ValidationPhase::Graph,
                LintCode::ContractUnsupportedExport,
                "Node is not export-compatible",
            )
            .with_target(DiagnosticTarget::Node { node_id })
            .with_field_path(format!("graph.nodes[{node_id}]"))
            .with_evidence_trace(),
        );
    }
    match node {
        StoryNode::Dialogue { speaker, .. } if speaker.trim().is_empty() => {
            issues.push(
                LintIssue::warning(
                    Some(node_id),
                    ValidationPhase::Graph,
                    LintCode::EmptySpeakerName,
                    "Dialogue speaker is empty",
                )
                .with_target(DiagnosticTarget::Character {
                    node_id: Some(node_id),
                    name: speaker.clone(),
                    field_path: Some(FieldPath::new(format!("graph.nodes[{node_id}].speaker"))),
                })
                .with_field_path(format!("graph.nodes[{node_id}].speaker"))
                .with_semantic_value(SemanticValue::new(
                    SemanticValueKind::CharacterRef,
                    speaker.clone(),
                    format!("graph.nodes[{node_id}].speaker"),
                ))
                .with_evidence_trace(),
            );
        }
        StoryNode::Choice { options, .. } => validate_choice(graph, node_id, options, &mut issues),
        StoryNode::SetVariable { key, .. } | StoryNode::SetFlag { key, .. }
            if key.trim().is_empty() =>
        {
            issues.push(
                LintIssue::error(
                    Some(node_id),
                    ValidationPhase::Graph,
                    LintCode::EmptyStateKey,
                    "State key is empty",
                )
                .with_field_path(format!("graph.nodes[{node_id}].key"))
                .with_semantic_value(SemanticValue::new(
                    SemanticValueKind::VariableRef,
                    key.clone(),
                    format!("graph.nodes[{node_id}].key"),
                ))
                .with_evidence_trace(),
            );
        }
        _ => {}
    }
    if !matches!(node, StoryNode::End) && !graph.connections().any(|conn| conn.from == node_id) {
        issues.push(
            LintIssue::warning(
                Some(node_id),
                ValidationPhase::Graph,
                LintCode::DeadEnd,
                "Node has no outgoing transition",
            )
            .with_target(DiagnosticTarget::Node { node_id })
            .with_evidence_trace(),
        );
    }
    issues
        .into_iter()
        .map(|issue| issue.diagnostic_id())
        .collect()
}

fn validate_layout_position(
    node_id: u32,
    position: AuthoringPosition,
    issues: &mut Vec<LintIssue>,
) {
    const MAX_AUTHORING_COORD: f32 = 1_000_000.0;
    if !position.x.is_finite()
        || !position.y.is_finite()
        || position.x.abs() > MAX_AUTHORING_COORD
        || position.y.abs() > MAX_AUTHORING_COORD
    {
        issues.push(
            LintIssue::error(
                Some(node_id),
                ValidationPhase::Graph,
                LintCode::InvalidLayoutPosition,
                "Node layout position is invalid",
            )
            .with_target(DiagnosticTarget::Node { node_id })
            .with_field_path(format!("graph.nodes[{node_id}].position"))
            .with_evidence_trace(),
        );
    }
}

fn validate_reachability(graph: &NodeGraph, node_id: u32, issues: &mut Vec<LintIssue>) {
    if matches!(graph.get_node(node_id), Some(StoryNode::Start)) || is_reachable(graph, node_id) {
        return;
    }
    let incoming = graph.incoming_nodes(node_id);
    let (edge_from, blocked_by) = if incoming.is_empty() {
        (
            None,
            "no incoming edges from any reachable path".to_string(),
        )
    } else {
        let reachable = reachable_nodes(graph);
        if let Some(from_id) = incoming
            .iter()
            .copied()
            .find(|candidate| reachable.contains(candidate))
        {
            (
                Some(from_id),
                format!("reachable predecessor {from_id} cannot advance into this branch"),
            )
        } else {
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
    };
    let mut issue = LintIssue::warning(
        Some(node_id),
        ValidationPhase::Graph,
        LintCode::UnreachableNode,
        "Unreachable node",
    )
    .with_blocked_by(blocked_by);
    if let Some(from_id) = edge_from {
        issue = issue.with_edge(Some(from_id), Some(node_id));
    }
    issues.push(
        issue
            .with_target(DiagnosticTarget::Node { node_id })
            .with_evidence_trace(),
    );
}

fn is_reachable(graph: &NodeGraph, target: u32) -> bool {
    if graph.incoming_nodes(target).is_empty() {
        return false;
    }
    reachable_nodes(graph).contains(&target)
}

fn reachable_nodes(graph: &NodeGraph) -> BTreeSet<u32> {
    let starts = graph
        .nodes()
        .filter_map(|(id, node, _)| matches!(node, StoryNode::Start).then_some(*id))
        .collect::<Vec<_>>();
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::from(starts);
    while let Some(node_id) = queue.pop_front() {
        if !reachable.insert(node_id) {
            continue;
        }
        for next in graph.outgoing_nodes(node_id) {
            if !reachable.contains(&next) {
                queue.push_back(next);
            }
        }
    }
    reachable
}

fn validate_choice(
    graph: &NodeGraph,
    node_id: u32,
    options: &[String],
    issues: &mut Vec<LintIssue>,
) {
    if options.is_empty() {
        issues.push(
            LintIssue::error(
                Some(node_id),
                ValidationPhase::Graph,
                LintCode::ChoiceNoOptions,
                "Choice has no options",
            )
            .with_target(DiagnosticTarget::Node { node_id })
            .with_field_path(format!("graph.nodes[{node_id}].options"))
            .with_evidence_trace(),
        );
    }
    for (idx, option) in options.iter().enumerate() {
        if option.trim() == format!("Option {}", idx + 1) {
            issues.push(
                LintIssue::warning(
                    Some(node_id),
                    ValidationPhase::Graph,
                    LintCode::PlaceholderChoiceOption,
                    format!("Choice option {idx} still uses placeholder text"),
                )
                .with_target(DiagnosticTarget::ChoiceOption {
                    node_id,
                    option_index: idx,
                })
                .with_field_path(format!("graph.nodes[{node_id}].options[{idx}].text"))
                .with_semantic_value(SemanticValue::new(
                    SemanticValueKind::Text,
                    option.clone(),
                    format!("graph.nodes[{node_id}].options[{idx}].text"),
                ))
                .with_evidence_trace(),
            );
        }
    }
    let outgoing = graph
        .connections()
        .filter(|conn| conn.from == node_id)
        .collect::<Vec<&GraphConnection>>();
    for idx in 0..options.len() {
        if !outgoing.iter().any(|conn| conn.from_port == idx) {
            issues.push(
                LintIssue::warning(
                    Some(node_id),
                    ValidationPhase::Graph,
                    LintCode::ChoiceOptionUnlinked,
                    format!("Choice option {idx} is unlinked"),
                )
                .with_edge(Some(node_id), None)
                .with_target(DiagnosticTarget::ChoiceOption {
                    node_id,
                    option_index: idx,
                })
                .with_field_path(format!("graph.nodes[{node_id}].options[{idx}].target"))
                .with_evidence_trace(),
            );
        }
    }
    for conn in outgoing {
        if conn.from_port >= options.len() {
            issues.push(
                LintIssue::warning(
                    Some(node_id),
                    ValidationPhase::Graph,
                    LintCode::ChoicePortOutOfRange,
                    "Choice connection port is out of range",
                )
                .with_edge(Some(conn.from), Some(conn.to))
                .with_target(DiagnosticTarget::Edge {
                    from: conn.from,
                    from_port: conn.from_port,
                    to: Some(conn.to),
                })
                .with_evidence_trace(),
            );
        }
    }
}

fn node_id_from_character_object_id(object_id: &str) -> Option<u32> {
    let mut parts = object_id.split(':');
    (parts.next()? == "node").then_some(())?;
    let node_id = parts.next()?.parse().ok()?;
    (parts.next()? == "character").then_some(())?;
    Some(node_id)
}
