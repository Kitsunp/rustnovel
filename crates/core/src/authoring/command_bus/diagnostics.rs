use std::collections::BTreeSet;

use super::super::{validate_authoring_graph_no_io, NodeGraph};
use super::AuthoringDelta;

#[derive(Clone, Debug, Default)]
pub(super) struct CommandBusDiagnosticState {
    ids: BTreeSet<String>,
}

impl CommandBusDiagnosticState {
    pub(super) fn from_graph(graph: &NodeGraph) -> Self {
        Self {
            ids: validate_authoring_graph_no_io(graph)
                .iter()
                .map(|issue| issue.diagnostic_id())
                .collect(),
        }
    }

    pub(super) fn ids(&self) -> BTreeSet<String> {
        self.ids.clone()
    }

    pub(super) fn apply_delta(&mut self, graph: &NodeGraph, delta: &AuthoringDelta) {
        match delta {
            // These commands only record evidence or change the editor's view.
            AuthoringDelta::AssetImported { .. }
            | AuthoringDelta::FragmentEntered { .. }
            | AuthoringDelta::FragmentLeft { .. } => {}
            AuthoringDelta::Reverted { reverted } => self.apply_delta(graph, reverted),
            // Reachability, cycles, labels and fragment rules can affect nodes
            // beyond a mutation's endpoints. Use the canonical validator.
            _ => *self = Self::from_graph(graph),
        }
    }

    pub(super) fn apply_reverted_delta(&mut self, graph: &NodeGraph, delta: &AuthoringDelta) {
        self.apply_delta(graph, delta);
    }
}
