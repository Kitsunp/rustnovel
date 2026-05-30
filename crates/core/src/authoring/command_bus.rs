mod apply;
mod choice;
mod diagnostics;
mod fingerprint;
mod fragments;
mod inverse;
mod node_edits;
mod types;

use super::{
    DiagnosticTarget, NodeGraph, OperationKind, OperationLogEntry, OperationStatus, VerificationRun,
};
use diagnostics::CommandBusDiagnosticState;
use fingerprint::CommandBusFingerprintState;

pub use types::{AuthoringCommand, AuthoringCommandOutcome, AuthoringDelta};

pub(super) type CommandApplyResult = Result<
    (
        AuthoringDelta,
        OperationKind,
        Option<String>,
        Option<DiagnosticTarget>,
    ),
    String,
>;

#[derive(Clone, Debug)]
pub struct AuthoringCommandBus {
    graph: NodeGraph,
    operation_log: Vec<OperationLogEntry>,
    verification_runs: Vec<VerificationRun>,
    undo_deltas: Vec<AuthoringDelta>,
    redo_deltas: Vec<AuthoringDelta>,
    recorded_commands: Vec<AuthoringCommand>,
    fingerprint_state: CommandBusFingerprintState,
    diagnostic_state: CommandBusDiagnosticState,
}

impl AuthoringCommandBus {
    pub fn new(graph: NodeGraph) -> Self {
        let fingerprint_state = CommandBusFingerprintState::from_graph(&graph);
        let diagnostic_state = CommandBusDiagnosticState::from_graph(&graph);
        Self {
            graph,
            operation_log: Vec::new(),
            verification_runs: Vec::new(),
            undo_deltas: Vec::new(),
            redo_deltas: Vec::new(),
            recorded_commands: Vec::new(),
            fingerprint_state,
            diagnostic_state,
        }
    }

    fn unlogged(graph: NodeGraph) -> Self {
        Self {
            graph,
            operation_log: Vec::new(),
            verification_runs: Vec::new(),
            undo_deltas: Vec::new(),
            redo_deltas: Vec::new(),
            recorded_commands: Vec::new(),
            fingerprint_state: CommandBusFingerprintState::default(),
            diagnostic_state: CommandBusDiagnosticState::default(),
        }
    }

    /// Applies the canonical graph command mutation and returns its delta without
    /// appending an operation entry. Editor/UI callers use this only for their
    /// transient graph state; the persisted operation trace must be recorded by
    /// the caller at the document/workbench boundary.
    pub fn apply_to_graph_unlogged(
        graph: &mut NodeGraph,
        command: AuthoringCommand,
    ) -> Result<AuthoringDelta, String> {
        let mut bus = Self::unlogged(std::mem::take(graph));
        let result = bus
            .apply_without_logging(&command)
            .map(|(delta, _, _, _)| delta);
        let (updated_graph, _, _) = bus.into_parts();
        *graph = updated_graph;
        result
    }

    pub fn with_history(
        graph: NodeGraph,
        operation_log: Vec<OperationLogEntry>,
        verification_runs: Vec<VerificationRun>,
    ) -> Self {
        let fingerprint_state = CommandBusFingerprintState::from_graph(&graph);
        let diagnostic_state = CommandBusDiagnosticState::from_graph(&graph);
        Self {
            graph,
            operation_log,
            verification_runs,
            undo_deltas: Vec::new(),
            redo_deltas: Vec::new(),
            recorded_commands: Vec::new(),
            fingerprint_state,
            diagnostic_state,
        }
    }

    pub fn replay(commands: &[AuthoringCommand]) -> Result<Self, String> {
        let mut bus = Self::new(NodeGraph::new());
        for command in commands {
            bus.apply(command.clone())?;
        }
        Ok(bus)
    }

    pub fn graph(&self) -> &NodeGraph {
        &self.graph
    }

    pub fn operation_log(&self) -> &[OperationLogEntry] {
        &self.operation_log
    }

    pub fn verification_runs(&self) -> &[VerificationRun] {
        &self.verification_runs
    }

    pub fn recorded_commands(&self) -> &[AuthoringCommand] {
        &self.recorded_commands
    }

    pub fn into_parts(self) -> (NodeGraph, Vec<OperationLogEntry>, Vec<VerificationRun>) {
        (self.graph, self.operation_log, self.verification_runs)
    }

    pub fn undo_delta_count(&self) -> usize {
        self.undo_deltas.len()
    }

    pub fn redo_delta_count(&self) -> usize {
        self.redo_deltas.len()
    }

    fn current_fingerprint(&self) -> super::AuthoringReportFingerprint {
        self.fingerprint_state.fingerprint()
    }

    fn current_diagnostic_ids(&self) -> std::collections::BTreeSet<String> {
        self.diagnostic_state.ids()
    }

    pub fn apply(&mut self, command: AuthoringCommand) -> Result<AuthoringCommandOutcome, String> {
        if matches!(command, AuthoringCommand::RevertLast) {
            return self.revert_last();
        }

        let before_diagnostic_ids = self.current_diagnostic_ids();
        let before_fingerprint = self.current_fingerprint();
        let (delta, kind, field_path, target) = self.apply_without_logging(&command)?;
        self.fingerprint_state.apply_delta(&self.graph, &delta);
        self.diagnostic_state.apply_delta(&self.graph, &delta);
        let after_fingerprint = self.current_fingerprint();
        let after_diagnostic_ids = self.current_diagnostic_ids();
        let mut operation = OperationLogEntry::new_typed(
            kind,
            OperationStatus::Applied,
            format!("authoring command applied: {command:?}"),
        )
        .with_before_after_fingerprints(&before_fingerprint, &after_fingerprint);
        if let Some(field_path) = field_path {
            operation = operation.with_field_path(field_path);
        }
        if let Some(target) = target {
            operation = operation.with_target(target);
        }
        if let AuthoringCommand::ApplyQuickFix { issue, .. } = &command {
            operation = operation.with_diagnostic(issue);
        }
        let verification = VerificationRun::from_diagnostic_id_sets(
            operation.operation_id.clone(),
            "command_bus",
            &after_fingerprint,
            &before_diagnostic_ids,
            &after_diagnostic_ids,
        );

        self.undo_deltas.push(delta.clone());
        self.redo_deltas.clear();
        self.recorded_commands.push(command);
        self.operation_log.push(operation.clone());
        self.verification_runs.push(verification.clone());
        Ok(AuthoringCommandOutcome {
            delta,
            operation,
            verification,
        })
    }

    fn revert_last(&mut self) -> Result<AuthoringCommandOutcome, String> {
        let delta = self
            .undo_deltas
            .pop()
            .ok_or_else(|| "no command delta to revert".to_string())?;
        let before_diagnostic_ids = self.current_diagnostic_ids();
        let before_fingerprint = self.current_fingerprint();
        self.apply_inverse_delta(&delta)?;
        self.fingerprint_state
            .apply_reverted_delta(&self.graph, &delta);
        self.diagnostic_state
            .apply_reverted_delta(&self.graph, &delta);
        let after_fingerprint = self.current_fingerprint();
        let after_diagnostic_ids = self.current_diagnostic_ids();
        let operation = OperationLogEntry::new_typed(
            OperationKind::Revert,
            OperationStatus::Applied,
            "reverted last command delta",
        )
        .with_before_after_fingerprints(&before_fingerprint, &after_fingerprint);
        let verification = VerificationRun::from_diagnostic_id_sets(
            operation.operation_id.clone(),
            "command_bus",
            &after_fingerprint,
            &before_diagnostic_ids,
            &after_diagnostic_ids,
        );
        let reverted = AuthoringDelta::Reverted {
            reverted: Box::new(delta.clone()),
        };
        self.redo_deltas.push(delta);
        self.recorded_commands.push(AuthoringCommand::RevertLast);
        self.operation_log.push(operation.clone());
        self.verification_runs.push(verification.clone());
        Ok(AuthoringCommandOutcome {
            delta: reverted,
            operation,
            verification,
        })
    }
}
