use super::super::{AuthoringCommandBus, AuthoringDelta};
use super::{AuthoringDocumentDelta, AuthoringDocumentSession};

impl AuthoringDocumentSession {
    pub(super) fn apply_inverse_delta(
        &mut self,
        delta: &AuthoringDocumentDelta,
    ) -> Result<(), String> {
        match delta {
            AuthoringDocumentDelta::Graph(delta) => self.apply_inverse_graph_delta(delta),
            AuthoringDocumentDelta::GraphReplaced { before } => {
                self.document.graph = before.as_ref().clone();
                Ok(())
            }
            AuthoringDocumentDelta::LayerVisibleChanged {
                object_id, before, ..
            }
            | AuthoringDocumentDelta::LayerLockedChanged {
                object_id, before, ..
            } => {
                restore_layer_override(
                    &mut self.document.composer_layer_overrides,
                    object_id,
                    *before,
                );
                Ok(())
            }
            AuthoringDocumentDelta::BackgroundFitChanged {
                node_id, before, ..
            }
            | AuthoringDocumentDelta::BackgroundFitCleared { node_id, before } => {
                let key = node_id.to_string();
                if let Some(before) = before {
                    self.document
                        .composer_background_fit_overrides
                        .insert(key, *before);
                } else {
                    self.document.composer_background_fit_overrides.remove(&key);
                }
                Ok(())
            }
            AuthoringDocumentDelta::ComposerOverridesReset {
                before_layers,
                before_background_fits,
            } => {
                self.document
                    .composer_layer_overrides
                    .clone_from(before_layers);
                self.document
                    .composer_background_fit_overrides
                    .clone_from(before_background_fits);
                Ok(())
            }
            AuthoringDocumentDelta::Reverted { reverted } => self.apply_inverse_delta(reverted),
        }
    }

    fn apply_inverse_graph_delta(&mut self, delta: &AuthoringDelta) -> Result<(), String> {
        AuthoringCommandBus::revert_graph_unlogged(&mut self.document.graph, delta)
    }
}

fn restore_layer_override(
    overrides: &mut std::collections::BTreeMap<String, super::super::composer::LayerOverride>,
    object_id: &str,
    before: Option<super::super::composer::LayerOverride>,
) {
    if let Some(before) = before {
        overrides.insert(object_id.to_string(), before);
    } else {
        overrides.remove(object_id);
    }
}
