use serde::Serialize;

use super::super::{
    composer::{BackgroundFit, LayerOverride},
    AuthoringCommand, AuthoringCommandBus, DiagnosticTarget, OperationKind,
};
use super::{
    AuthoringDocumentCommand, AuthoringDocumentDelta, AuthoringDocumentSession,
    DocumentApplyMetadata, DocumentApplyResult,
};

impl AuthoringDocumentSession {
    pub(super) fn apply_without_logging(
        &mut self,
        command: &AuthoringDocumentCommand,
    ) -> DocumentApplyResult {
        match command {
            AuthoringDocumentCommand::Graph(command) => self.apply_graph_command(command),
            AuthoringDocumentCommand::SetLayerVisible { object_id, visible } => {
                self.set_layer_visible(object_id, *visible)
            }
            AuthoringDocumentCommand::SetLayerLocked { object_id, locked } => {
                self.set_layer_locked(object_id, *locked)
            }
            AuthoringDocumentCommand::SetBackgroundFitOverride { node_id, fit } => {
                self.set_background_fit(*node_id, *fit)
            }
            AuthoringDocumentCommand::ClearBackgroundFitOverride { node_id } => {
                self.clear_background_fit(*node_id)
            }
            AuthoringDocumentCommand::RevertLast => unreachable!("handled by apply"),
        }
    }

    fn apply_graph_command(&mut self, command: &AuthoringCommand) -> DocumentApplyResult {
        if matches!(command, AuthoringCommand::RevertLast) {
            return Err("use AuthoringDocumentCommand::RevertLast for document undo".to_string());
        }

        let graph = std::mem::take(&mut self.document.graph);
        let mut graph_bus = AuthoringCommandBus::new(graph);
        let outcome = match graph_bus.apply(command.clone()) {
            Ok(outcome) => outcome,
            Err(error) => {
                let (graph, _, _) = graph_bus.into_parts();
                self.document.graph = graph;
                return Err(error);
            }
        };
        let (graph, _, _) = graph_bus.into_parts();
        self.document.graph = graph;
        let kind = outcome
            .operation
            .operation_kind_v2
            .clone()
            .ok_or_else(|| "graph command did not produce an operation kind v2".to_string())?;
        Ok((
            AuthoringDocumentDelta::Graph(Box::new(outcome.delta)),
            DocumentApplyMetadata {
                kind,
                field_paths: outcome
                    .operation
                    .field_paths
                    .into_iter()
                    .map(|path| path.value)
                    .collect(),
                targets: outcome.operation.affected_targets,
                before_value: outcome.operation.before_value,
                after_value: outcome.operation.after_value,
            },
        ))
    }

    fn set_layer_visible(&mut self, object_id: &str, visible: bool) -> DocumentApplyResult {
        let before = self
            .document
            .composer_layer_overrides
            .get(object_id)
            .copied();
        if before.unwrap_or_default().visible == visible {
            return Err(format!("no-op layer visibility command for '{object_id}'"));
        }
        let mut after = before.unwrap_or_default();
        after.visible = visible;
        let after = self.store_layer_override(object_id, after);
        Ok((
            AuthoringDocumentDelta::LayerVisibleChanged {
                object_id: object_id.to_string(),
                before,
                after,
            },
            DocumentApplyMetadata {
                kind: OperationKind::LayerVisibilityChanged,
                field_paths: vec![format!("composer.layers[{object_id}].visible")],
                targets: vec![DiagnosticTarget::Graph],
                before_value: json_string(&before),
                after_value: json_string(&after),
            },
        ))
    }

    fn set_layer_locked(&mut self, object_id: &str, locked: bool) -> DocumentApplyResult {
        let before = self
            .document
            .composer_layer_overrides
            .get(object_id)
            .copied();
        if before.unwrap_or_default().locked == locked {
            return Err(format!("no-op layer lock command for '{object_id}'"));
        }
        let mut after = before.unwrap_or_default();
        after.locked = locked;
        let after = self.store_layer_override(object_id, after);
        Ok((
            AuthoringDocumentDelta::LayerLockedChanged {
                object_id: object_id.to_string(),
                before,
                after,
            },
            DocumentApplyMetadata {
                kind: OperationKind::LayerLockChanged,
                field_paths: vec![format!("composer.layers[{object_id}].locked")],
                targets: vec![DiagnosticTarget::Graph],
                before_value: json_string(&before),
                after_value: json_string(&after),
            },
        ))
    }

    fn set_background_fit(&mut self, node_id: u32, fit: BackgroundFit) -> DocumentApplyResult {
        let key = node_id.to_string();
        let before = self
            .document
            .composer_background_fit_overrides
            .get(&key)
            .copied();
        if before == Some(fit) {
            return Err(format!("no-op background fit command for node {node_id}"));
        }
        self.document
            .composer_background_fit_overrides
            .insert(key, fit);
        Ok((
            AuthoringDocumentDelta::BackgroundFitChanged {
                node_id,
                before,
                after: Some(fit),
            },
            DocumentApplyMetadata {
                kind: OperationKind::FieldEdited,
                field_paths: vec![format!("composer.background_fit[{node_id}]")],
                targets: vec![DiagnosticTarget::Graph],
                before_value: json_string(&before),
                after_value: json_string(&Some(fit)),
            },
        ))
    }

    fn clear_background_fit(&mut self, node_id: u32) -> DocumentApplyResult {
        let key = node_id.to_string();
        let before = self
            .document
            .composer_background_fit_overrides
            .remove(&key)
            .ok_or_else(|| format!("no-op background fit clear for node {node_id}"))?;
        Ok((
            AuthoringDocumentDelta::BackgroundFitCleared {
                node_id,
                before: Some(before),
            },
            DocumentApplyMetadata {
                kind: OperationKind::FieldEdited,
                field_paths: vec![format!("composer.background_fit[{node_id}]")],
                targets: vec![DiagnosticTarget::Graph],
                before_value: json_string(&Some(before)),
                after_value: json_string(&Option::<BackgroundFit>::None),
            },
        ))
    }

    fn store_layer_override(
        &mut self,
        object_id: &str,
        override_state: LayerOverride,
    ) -> Option<LayerOverride> {
        if override_state == LayerOverride::default() {
            self.document.composer_layer_overrides.remove(object_id);
            None
        } else {
            self.document
                .composer_layer_overrides
                .insert(object_id.to_string(), override_state);
            Some(override_state)
        }
    }
}

fn json_string<T: Serialize>(value: &T) -> Option<String> {
    serde_json::to_string(value).ok()
}
