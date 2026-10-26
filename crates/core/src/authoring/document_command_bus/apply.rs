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
            AuthoringDocumentCommand::ReplaceGraph {
                graph,
                kind,
                field_path,
                before_value,
                after_value,
                ..
            } => {
                if self.document.graph.has_same_content(graph) {
                    return Err("no-op graph replacement".to_string());
                }
                let before = std::mem::replace(&mut self.document.graph, graph.as_ref().clone());
                Ok((
                    AuthoringDocumentDelta::GraphReplaced {
                        before: Box::new(before),
                    },
                    DocumentApplyMetadata {
                        kind: kind.clone(),
                        diagnostic_id: None,
                        diagnostic_target: None,
                        field_paths: field_path.iter().cloned().collect(),
                        targets: vec![DiagnosticTarget::Graph],
                        before_value: before_value.clone(),
                        after_value: after_value.clone(),
                    },
                ))
            }
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
            AuthoringDocumentCommand::ResetComposerOverrides => self.reset_composer_overrides(),
            AuthoringDocumentCommand::RevertLast
            | AuthoringDocumentCommand::UndoLast
            | AuthoringDocumentCommand::RedoLast => {
                unreachable!("handled by apply")
            }
        }
    }

    fn apply_graph_command(&mut self, command: &AuthoringCommand) -> DocumentApplyResult {
        if matches!(command, AuthoringCommand::RevertLast) {
            return Err("use AuthoringDocumentCommand::RevertLast for document undo".to_string());
        }
        if matches!(command, AuthoringCommand::RedoLast) {
            return Err("use AuthoringDocumentCommand::RedoLast for document redo".to_string());
        }

        let (delta, metadata) = AuthoringCommandBus::apply_to_graph_unlogged_with_metadata(
            &mut self.document.graph,
            command,
        )?;
        Ok((AuthoringDocumentDelta::Graph(Box::new(delta)), metadata))
    }

    fn set_layer_visible(&mut self, object_id: &str, visible: bool) -> DocumentApplyResult {
        self.ensure_composer_layer_exists(object_id)?;
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
                diagnostic_id: None,
                diagnostic_target: None,
                field_paths: vec![format!("composer.layers[{object_id}].visible")],
                targets: vec![DiagnosticTarget::Graph],
                before_value: json_string(&before),
                after_value: json_string(&after),
            },
        ))
    }

    fn set_layer_locked(&mut self, object_id: &str, locked: bool) -> DocumentApplyResult {
        self.ensure_composer_layer_exists(object_id)?;
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
                diagnostic_id: None,
                diagnostic_target: None,
                field_paths: vec![format!("composer.layers[{object_id}].locked")],
                targets: vec![DiagnosticTarget::Graph],
                before_value: json_string(&before),
                after_value: json_string(&after),
            },
        ))
    }

    fn set_background_fit(&mut self, node_id: u32, fit: BackgroundFit) -> DocumentApplyResult {
        if self.document.graph.get_node(node_id).is_none() {
            return Err(format!("background fit node {node_id} not found"));
        }
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
                diagnostic_id: None,
                diagnostic_target: None,
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
                diagnostic_id: None,
                diagnostic_target: None,
                field_paths: vec![format!("composer.background_fit[{node_id}]")],
                targets: vec![DiagnosticTarget::Graph],
                before_value: json_string(&Some(before)),
                after_value: json_string(&Option::<BackgroundFit>::None),
            },
        ))
    }

    fn reset_composer_overrides(&mut self) -> DocumentApplyResult {
        if self.document.composer_layer_overrides.is_empty()
            && self.document.composer_background_fit_overrides.is_empty()
        {
            return Err("no-op composer overrides reset".to_string());
        }
        let before_layers = std::mem::take(&mut self.document.composer_layer_overrides);
        let before_background_fits =
            std::mem::take(&mut self.document.composer_background_fit_overrides);
        let metadata = DocumentApplyMetadata {
            kind: OperationKind::FieldEdited,
            diagnostic_id: None,
            diagnostic_target: None,
            field_paths: vec![
                "composer.layers".to_string(),
                "composer.background_fit".to_string(),
            ],
            targets: vec![DiagnosticTarget::Graph],
            before_value: json_string(&(&before_layers, &before_background_fits)),
            after_value: json_string(&(
                &self.document.composer_layer_overrides,
                &self.document.composer_background_fit_overrides,
            )),
        };
        Ok((
            AuthoringDocumentDelta::ComposerOverridesReset {
                before_layers,
                before_background_fits,
            },
            metadata,
        ))
    }

    fn ensure_composer_layer_exists(&self, object_id: &str) -> Result<(), String> {
        if self.read_model.composer_layer(object_id).is_some() {
            return Ok(());
        }
        Err(format!("unknown composer layer object '{object_id}'"))
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
