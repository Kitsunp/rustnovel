mod apply;
mod choice;
mod diagnostics;
mod fingerprint;
mod fragments;
mod inverse;
mod node_edits;
mod types;

use super::{
    DiagnosticTarget, NodeGraph, OperationKind, OperationLogEntry, OperationStatus, StoryNode,
    VerificationRun,
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

pub(super) struct CommandApplyMetadata {
    pub(super) kind: OperationKind,
    pub(super) diagnostic_id: Option<String>,
    pub(super) diagnostic_target: Option<DiagnosticTarget>,
    pub(super) field_paths: Vec<String>,
    pub(super) targets: Vec<DiagnosticTarget>,
    pub(super) before_value: Option<String>,
    pub(super) after_value: Option<String>,
}

impl CommandApplyMetadata {
    pub(super) fn graph_details(&self, command: &AuthoringCommand) -> String {
        // Text edits are losslessly represented by the structured values.
        // Other payloads keep Debug details, including non-finite numeric inputs.
        let text_edit = matches!(
            command,
            AuthoringCommand::EditDialogue { .. }
                | AuthoringCommand::EditChoicePrompt { .. }
                | AuthoringCommand::EditChoiceOptionText { .. }
                | AuthoringCommand::EditNode {
                    replacement: StoryNode::Dialogue { .. } | StoryNode::Choice { .. },
                    ..
                }
        );
        if text_edit && self.before_value.is_some() && self.after_value.is_some() {
            format!(
                "authoring command applied: {} (see before_value/after_value)",
                self.kind.label()
            )
        } else {
            format!("authoring command applied: {command:?}")
        }
    }

    fn for_graph(
        command: &AuthoringCommand,
        delta: &AuthoringDelta,
        kind: OperationKind,
        field_path: Option<String>,
        target: Option<DiagnosticTarget>,
    ) -> Self {
        let mut metadata = Self {
            kind,
            diagnostic_id: None,
            diagnostic_target: None,
            field_paths: field_path.into_iter().collect(),
            targets: target.into_iter().collect(),
            before_value: None,
            after_value: None,
        };
        if let Some((before, after)) = delta_before_after_values(delta) {
            metadata.before_value = Some(before);
            metadata.after_value = Some(after);
        }
        if let AuthoringCommand::ApplyQuickFix { issue, .. } = command {
            metadata.diagnostic_id = Some(issue.diagnostic_id());
            metadata.diagnostic_target = issue.target.clone();
            metadata.targets.extend(issue.target.iter().cloned());
            metadata
                .field_paths
                .extend(issue.field_path.iter().map(|path| path.value.clone()));
        }
        metadata
    }

    pub(super) fn into_operation(
        self,
        kind_override: Option<OperationKind>,
        details: String,
    ) -> OperationLogEntry {
        let mut operation = OperationLogEntry::new_typed(
            kind_override.unwrap_or(self.kind),
            OperationStatus::Applied,
            details,
        );
        for field_path in self.field_paths {
            operation = operation.with_field_path(field_path);
        }
        for target in self.targets {
            operation = operation.with_target(target);
        }
        operation.diagnostic_id = self.diagnostic_id;
        operation.diagnostic_target = self.diagnostic_target;
        operation.before_value = self.before_value;
        operation.after_value = self.after_value;
        operation
    }
}

#[derive(Clone, Debug)]
pub struct AuthoringCommandBus {
    graph: NodeGraph,
    operation_log: Vec<OperationLogEntry>,
    verification_runs: Vec<VerificationRun>,
    undo_deltas: Vec<(AuthoringDelta, usize)>,
    redo_command_indices: Vec<usize>,
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
            redo_command_indices: Vec::new(),
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
            redo_command_indices: Vec::new(),
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
        Self::mutate_graph_unlogged(graph, &command).map(|(delta, _, _, _)| delta)
    }

    pub(super) fn apply_to_graph_unlogged_with_metadata(
        graph: &mut NodeGraph,
        command: &AuthoringCommand,
    ) -> Result<(AuthoringDelta, CommandApplyMetadata), String> {
        let (delta, kind, field_path, target) = Self::mutate_graph_unlogged(graph, command)?;
        let metadata = CommandApplyMetadata::for_graph(command, &delta, kind, field_path, target);
        Ok((delta, metadata))
    }

    fn mutate_graph_unlogged(
        graph: &mut NodeGraph,
        command: &AuthoringCommand,
    ) -> CommandApplyResult {
        if matches!(
            command,
            AuthoringCommand::RevertLast | AuthoringCommand::RedoLast
        ) {
            return Err("history commands require a command bus session".to_string());
        }
        let mut bus = Self::unlogged(std::mem::take(graph));
        let result = bus.apply_without_logging(command);
        let (updated_graph, _, _) = bus.into_parts();
        *graph = updated_graph;
        result
    }

    pub(super) fn revert_graph_unlogged(
        graph: &mut NodeGraph,
        delta: &AuthoringDelta,
    ) -> Result<(), String> {
        let mut bus = Self::unlogged(std::mem::take(graph));
        let result = bus.apply_inverse_delta(delta);
        *graph = bus.graph;
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
            redo_command_indices: Vec::new(),
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
        self.redo_command_indices.len()
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
        if matches!(command, AuthoringCommand::RedoLast) {
            return self.redo_last();
        }
        self.apply_forward(command, None)
    }

    fn apply_forward(
        &mut self,
        command: AuthoringCommand,
        redo_index: Option<usize>,
    ) -> Result<AuthoringCommandOutcome, String> {
        let before_diagnostic_ids = self.current_diagnostic_ids();
        let before_fingerprint = self.current_fingerprint();
        let (delta, kind, field_path, target) = self.apply_without_logging(&command)?;
        self.fingerprint_state.apply_delta(&self.graph, &delta);
        self.diagnostic_state.apply_delta(&self.graph, &delta);
        let after_fingerprint = self.current_fingerprint();
        let after_diagnostic_ids = self.current_diagnostic_ids();
        let metadata = CommandApplyMetadata::for_graph(&command, &delta, kind, field_path, target);
        let details = metadata.graph_details(&command);
        let operation = metadata
            .into_operation(redo_index.map(|_| OperationKind::Redo), details)
            .with_before_after_fingerprints(&before_fingerprint, &after_fingerprint);
        let verification = VerificationRun::from_diagnostic_id_sets(
            operation.operation_id.clone(),
            "command_bus",
            &after_fingerprint,
            &before_diagnostic_ids,
            &after_diagnostic_ids,
        );

        self.undo_deltas.push((
            delta.clone(),
            redo_index.unwrap_or(self.recorded_commands.len()),
        ));
        self.redo_command_indices.clear();
        self.recorded_commands.push(if redo_index.is_some() {
            AuthoringCommand::RedoLast
        } else {
            command
        });
        self.operation_log.push(operation.clone());
        self.verification_runs.push(verification.clone());
        Ok(AuthoringCommandOutcome {
            delta,
            operation,
            verification,
        })
    }

    fn revert_last(&mut self) -> Result<AuthoringCommandOutcome, String> {
        let (delta, command_index) = self
            .undo_deltas
            .pop()
            .ok_or_else(|| "no command delta to revert".to_string())?;
        let before_diagnostic_ids = self.current_diagnostic_ids();
        let before_fingerprint = self.current_fingerprint();
        if let Err(error) = self.apply_inverse_delta(&delta) {
            self.undo_deltas.push((delta, command_index));
            return Err(error);
        }
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
            reverted: Box::new(delta),
        };
        self.redo_command_indices.push(command_index);
        self.recorded_commands.push(AuthoringCommand::RevertLast);
        self.operation_log.push(operation.clone());
        self.verification_runs.push(verification.clone());
        Ok(AuthoringCommandOutcome {
            delta: reverted,
            operation,
            verification,
        })
    }

    fn redo_last(&mut self) -> Result<AuthoringCommandOutcome, String> {
        let index = self
            .redo_command_indices
            .pop()
            .ok_or_else(|| "no command to redo".to_string())?;
        let pending = std::mem::take(&mut self.redo_command_indices);
        let result = self.apply_forward(self.recorded_commands[index].clone(), Some(index));
        self.redo_command_indices = pending;
        if result.is_err() {
            self.redo_command_indices.push(index);
        }
        result
    }
}

fn delta_before_after_values(delta: &AuthoringDelta) -> Option<(String, String)> {
    match delta {
        AuthoringDelta::NodeEdited { before, after, .. } => Some((
            serde_json::to_string(before).ok()?,
            serde_json::to_string(after).ok()?,
        )),
        AuthoringDelta::LayerMoved { before, after, .. } => {
            let pose = |value: &(Option<i32>, Option<i32>, Option<f32>)| {
                serde_json::json!({"x": value.0, "y": value.1, "scale": value.2}).to_string()
            };
            Some((pose(before), pose(after)))
        }
        _ => None,
    }
}
