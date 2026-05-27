mod apply;
mod choice;
mod fragments;
mod inverse;
mod node_edits;
mod types;

use super::{
    build_authoring_report_fingerprint, validate_authoring_graph_no_io, DiagnosticTarget,
    NodeGraph, OperationKind, OperationLogEntry, OperationStatus, VerificationRun,
};

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
}

impl AuthoringCommandBus {
    pub fn new(graph: NodeGraph) -> Self {
        Self {
            graph,
            operation_log: Vec::new(),
            verification_runs: Vec::new(),
            undo_deltas: Vec::new(),
            redo_deltas: Vec::new(),
            recorded_commands: Vec::new(),
        }
    }

    pub fn with_history(
        graph: NodeGraph,
        operation_log: Vec<OperationLogEntry>,
        verification_runs: Vec<VerificationRun>,
    ) -> Self {
        Self {
            graph,
            operation_log,
            verification_runs,
            undo_deltas: Vec::new(),
            redo_deltas: Vec::new(),
            recorded_commands: Vec::new(),
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

    pub fn apply(&mut self, command: AuthoringCommand) -> Result<AuthoringCommandOutcome, String> {
        if matches!(command, AuthoringCommand::RevertLast) {
            return self.revert_last();
        }

        let before_issues = validate_authoring_graph_no_io(&self.graph);
        let before_fingerprint = build_authoring_report_fingerprint(
            &self.graph,
            &self.graph.to_script_lossy_for_diagnostics(),
        );
        let (delta, kind, field_path, target) = self.apply_without_logging(&command)?;
        let after_fingerprint = build_authoring_report_fingerprint(
            &self.graph,
            &self.graph.to_script_lossy_for_diagnostics(),
        );
        let after_issues = validate_authoring_graph_no_io(&self.graph);
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
        let verification = VerificationRun::from_diagnostics(
            operation.operation_id.clone(),
            "command_bus",
            &after_fingerprint,
            &before_issues,
            &after_issues,
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
        let before_issues = validate_authoring_graph_no_io(&self.graph);
        let before_fingerprint = build_authoring_report_fingerprint(
            &self.graph,
            &self.graph.to_script_lossy_for_diagnostics(),
        );
        self.apply_inverse_delta(&delta)?;
        let after_fingerprint = build_authoring_report_fingerprint(
            &self.graph,
            &self.graph.to_script_lossy_for_diagnostics(),
        );
        let after_issues = validate_authoring_graph_no_io(&self.graph);
        let operation = OperationLogEntry::new_typed(
            OperationKind::Revert,
            OperationStatus::Applied,
            "reverted last command delta",
        )
        .with_before_after_fingerprints(&before_fingerprint, &after_fingerprint);
        let verification = VerificationRun::from_diagnostics(
            operation.operation_id.clone(),
            "command_bus",
            &after_fingerprint,
            &before_issues,
            &after_issues,
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
