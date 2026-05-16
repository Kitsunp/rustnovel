use std::collections::{BTreeMap, HashSet};

use super::{FragmentPort, GraphFragment};
use crate::authoring::{
    DiagnosticTarget, LintCode, LintIssue, NodeGraph, StoryNode, ValidationPhase,
};

impl NodeGraph {
    pub fn create_fragment(
        &mut self,
        fragment_id: impl Into<String>,
        title: impl Into<String>,
        mut node_ids: Vec<u32>,
    ) -> bool {
        let fragment_id = fragment_id.into().trim().to_string();
        if fragment_id.is_empty() || self.fragments.contains_key(&fragment_id) {
            return false;
        }
        node_ids.retain(|node_id| self.get_node(*node_id).is_some());
        node_ids.sort_unstable();
        node_ids.dedup();
        if node_ids.is_empty()
            || node_ids
                .iter()
                .any(|node_id| self.fragment_for_node(*node_id).is_some())
        {
            return false;
        }
        let (inputs, outputs) = self.calculate_fragment_ports(&node_ids);
        self.fragments.insert(
            fragment_id.clone(),
            GraphFragment {
                fragment_id,
                title: title.into(),
                node_ids,
                inputs,
                outputs,
            },
        );
        self.modified = true;
        true
    }

    pub fn remove_fragment(&mut self, fragment_id: &str) -> Option<GraphFragment> {
        let removed = self.fragments.remove(fragment_id);
        if removed.is_some() {
            if self.graph_stack.active_fragment.as_deref() == Some(fragment_id) {
                self.graph_stack.active_fragment = None;
            }
            self.graph_stack
                .breadcrumb
                .retain(|candidate| candidate != fragment_id);
            self.modified = true;
        }
        removed
    }

    pub fn fragment(&self, fragment_id: &str) -> Option<&GraphFragment> {
        self.fragments.get(fragment_id)
    }

    pub fn get_fragment(&self, fragment_id: &str) -> Option<&GraphFragment> {
        self.fragment(fragment_id)
    }

    pub fn fragments(&self) -> impl Iterator<Item = (&String, &GraphFragment)> {
        self.fragments.iter()
    }

    pub fn list_fragments(&self) -> Vec<GraphFragment> {
        self.fragments.values().cloned().collect()
    }

    pub fn graph_stack(&self) -> &super::GraphStack {
        &self.graph_stack
    }

    pub fn active_fragment(&self) -> Option<&str> {
        self.graph_stack.active_fragment.as_deref()
    }

    pub fn fragment_ports(
        &self,
        fragment_id: &str,
    ) -> Option<(Vec<FragmentPort>, Vec<FragmentPort>)> {
        let fragment = self.fragments.get(fragment_id)?;
        Some((fragment.inputs.clone(), fragment.outputs.clone()))
    }

    pub fn refresh_fragment_ports(&mut self, fragment_id: &str) -> bool {
        let Some(node_ids) = self
            .fragments
            .get(fragment_id)
            .map(|fragment| fragment.node_ids.clone())
        else {
            return false;
        };
        let (inputs, outputs) = self.calculate_fragment_ports(&node_ids);
        let Some(fragment) = self.fragments.get_mut(fragment_id) else {
            return false;
        };
        if fragment.inputs == inputs && fragment.outputs == outputs {
            return false;
        }
        fragment.inputs = inputs;
        fragment.outputs = outputs;
        self.modified = true;
        true
    }

    pub fn validate_fragments(&self) -> Vec<LintIssue> {
        let mut issues = Vec::new();
        let mut ownership = BTreeMap::<u32, String>::new();
        let existing_nodes = self
            .nodes
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<HashSet<_>>();
        for (fragment_id, fragment) in &self.fragments {
            self.validate_fragment_container(
                fragment_id,
                fragment,
                &existing_nodes,
                &mut ownership,
                &mut issues,
            );
        }
        for (node_id, node, _) in &self.nodes {
            self.validate_subgraph_call_node(*node_id, node, &mut issues);
        }
        self.validate_fragment_recursion(&mut issues);
        issues
    }

    pub fn enter_fragment(&mut self, fragment_id: &str) -> bool {
        if !self.fragments.contains_key(fragment_id) {
            return false;
        }
        if let Some(active) = self.graph_stack.active_fragment.take() {
            self.graph_stack.breadcrumb.push(active);
        }
        self.graph_stack.active_fragment = Some(fragment_id.to_string());
        self.modified = true;
        true
    }

    pub fn leave_fragment(&mut self) -> bool {
        let Some(previous) = self.graph_stack.breadcrumb.pop() else {
            if self.graph_stack.active_fragment.take().is_some() {
                self.modified = true;
                return true;
            }
            return false;
        };
        self.graph_stack.active_fragment = Some(previous);
        self.modified = true;
        true
    }

    fn validate_fragment_container(
        &self,
        fragment_id: &str,
        fragment: &GraphFragment,
        existing_nodes: &HashSet<u32>,
        ownership: &mut BTreeMap<u32, String>,
        issues: &mut Vec<LintIssue>,
    ) {
        if fragment.node_ids.is_empty() {
            issues.push(fragment_issue(
                fragment_id,
                LintCode::FragmentEmpty,
                "Fragment contains no nodes",
            ));
        }
        for node_id in &fragment.node_ids {
            if !existing_nodes.contains(node_id) {
                issues.push(fragment_issue(
                    fragment_id,
                    LintCode::FragmentNodeMissing,
                    format!("Fragment references missing node {node_id}"),
                ));
            }
            if let Some(previous) = ownership.insert(*node_id, fragment_id.to_string()) {
                issues.push(fragment_issue(
                    fragment_id,
                    LintCode::FragmentOwnershipConflict,
                    format!("Node {node_id} belongs to fragments '{previous}' and '{fragment_id}'"),
                ));
            }
        }
        let (inputs, outputs) = self.calculate_fragment_ports(&fragment.node_ids);
        if fragment.inputs != inputs || fragment.outputs != outputs {
            issues.push(fragment_issue(
                fragment_id,
                LintCode::FragmentPortStale,
                "Fragment ports are stale",
            ));
        }
    }

    fn validate_subgraph_call_node(
        &self,
        node_id: u32,
        node: &StoryNode,
        issues: &mut Vec<LintIssue>,
    ) {
        let StoryNode::SubgraphCall {
            fragment_id,
            entry_port,
            exit_port,
        } = node
        else {
            return;
        };
        let Some(fragment) = self.fragments.get(fragment_id) else {
            issues.push(
                LintIssue::error(
                    Some(node_id),
                    ValidationPhase::Graph,
                    LintCode::SubgraphCallInvalid,
                    format!("SubgraphCall references missing fragment '{fragment_id}'"),
                )
                .with_target(DiagnosticTarget::Node { node_id })
                .with_field_path(format!("graph.nodes[{node_id}].fragment_id"))
                .with_evidence_trace(),
            );
            return;
        };
        let missing_entry = entry_port.as_deref().is_some_and(|port| {
            !fragment
                .inputs
                .iter()
                .any(|candidate| candidate.port_id == port)
        });
        let missing_exit = exit_port.as_deref().is_some_and(|port| {
            !fragment
                .outputs
                .iter()
                .any(|candidate| candidate.port_id == port)
        });
        if missing_entry || missing_exit {
            issues.push(
                LintIssue::error(
                    Some(node_id),
                    ValidationPhase::Graph,
                    LintCode::SubgraphCallInvalid,
                    "SubgraphCall references a missing port",
                )
                .with_target(DiagnosticTarget::Node { node_id })
                .with_field_path(format!("graph.nodes[{node_id}].subgraph_call"))
                .with_evidence_trace(),
            );
        }
        if fragment.node_ids.contains(&node_id) {
            issues.push(
                LintIssue::error(
                    Some(node_id),
                    ValidationPhase::Graph,
                    LintCode::FragmentRecursion,
                    "Fragment contains a call to itself",
                )
                .with_target(DiagnosticTarget::Fragment {
                    fragment_id: fragment_id.clone(),
                })
                .with_evidence_trace(),
            );
        }
    }

    fn calculate_fragment_ports(&self, node_ids: &[u32]) -> (Vec<FragmentPort>, Vec<FragmentPort>) {
        let node_set = node_ids.iter().copied().collect::<HashSet<_>>();
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for connection in &self.connections {
            let from_inside = node_set.contains(&connection.from);
            let to_inside = node_set.contains(&connection.to);
            match (from_inside, to_inside) {
                (false, true) => inputs.push(FragmentPort {
                    port_id: format!("in_{}_{}", connection.to, connection.from_port),
                    label: format!("from {}:{}", connection.from, connection.from_port),
                    node_id: Some(connection.to),
                }),
                (true, false) => outputs.push(FragmentPort {
                    port_id: format!("out_{}_{}", connection.from, connection.from_port),
                    label: format!("to {}", connection.to),
                    node_id: Some(connection.from),
                }),
                _ => {}
            }
        }
        inputs.sort_by(|a, b| a.port_id.cmp(&b.port_id));
        inputs.dedup_by(|a, b| a.port_id == b.port_id);
        outputs.sort_by(|a, b| a.port_id.cmp(&b.port_id));
        outputs.dedup_by(|a, b| a.port_id == b.port_id);
        (inputs, outputs)
    }

    fn fragment_for_node(&self, node_id: u32) -> Option<&str> {
        self.fragments.iter().find_map(|(fragment_id, fragment)| {
            fragment
                .node_ids
                .contains(&node_id)
                .then_some(fragment_id.as_str())
        })
    }

    fn validate_fragment_recursion(&self, issues: &mut Vec<LintIssue>) {
        let edges = self.fragment_call_edges();
        for fragment_id in self.fragments.keys() {
            let mut visited = HashSet::new();
            let mut path = Vec::new();
            if let Some(cycle) =
                find_cycle_to(fragment_id, fragment_id, &edges, &mut visited, &mut path)
            {
                issues.push(fragment_issue(
                    fragment_id,
                    LintCode::FragmentRecursion,
                    format!("Fragment recursion detected: {}", cycle.join(" -> ")),
                ));
            }
        }
    }

    fn fragment_call_edges(&self) -> BTreeMap<String, Vec<String>> {
        let mut edges = BTreeMap::<String, Vec<String>>::new();
        for (source_fragment_id, fragment) in &self.fragments {
            for node_id in &fragment.node_ids {
                let Some(StoryNode::SubgraphCall { fragment_id, .. }) = self.get_node(*node_id)
                else {
                    continue;
                };
                if self.fragments.contains_key(fragment_id) {
                    edges
                        .entry(source_fragment_id.clone())
                        .or_default()
                        .push(fragment_id.clone());
                }
            }
        }
        for targets in edges.values_mut() {
            targets.sort();
            targets.dedup();
        }
        edges
    }
}

fn find_cycle_to(
    current: &str,
    target: &str,
    edges: &BTreeMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    path: &mut Vec<String>,
) -> Option<Vec<String>> {
    if !visited.insert(current.to_string()) {
        return None;
    }
    path.push(current.to_string());
    if let Some(next_fragments) = edges.get(current) {
        for next in next_fragments {
            if next == target {
                let mut cycle = path.clone();
                cycle.push(target.to_string());
                return Some(cycle);
            }
            if let Some(cycle) = find_cycle_to(next, target, edges, visited, path) {
                return Some(cycle);
            }
        }
    }
    path.pop();
    visited.remove(current);
    None
}

fn fragment_issue(fragment_id: &str, code: LintCode, message: impl Into<String>) -> LintIssue {
    LintIssue::error(None, ValidationPhase::Graph, code, message)
        .with_target(DiagnosticTarget::Fragment {
            fragment_id: fragment_id.to_string(),
        })
        .with_field_path(format!("graph.fragments.{fragment_id}"))
        .with_evidence_trace()
}
