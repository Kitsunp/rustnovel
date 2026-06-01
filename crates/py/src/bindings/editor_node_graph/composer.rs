use visual_novel_engine::authoring::composer::{
    apply_layer_overrides, compose_scene_snapshot as build_composer_snapshot,
    list_layered_objects as collect_layered_objects,
};
use visual_novel_engine::authoring::{AuthoringCommand, AuthoringDocumentCommand};

use super::super::api_v2::{
    stage_layer_names, PyComposerPreviewSession, PyComposerSnapshot, PyLayeredSceneObject,
    PyOperationLogEntry, PyVerificationRun,
};
use super::{py_command_error, PyNodeGraph};

impl PyNodeGraph {
    pub(super) fn py_operation_log(&self) -> Vec<PyOperationLogEntry> {
        self.session
            .document()
            .operation_log
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub(super) fn py_verification_runs(&self) -> Vec<PyVerificationRun> {
        self.session
            .document()
            .verification_runs
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub(super) fn py_compose_scene_snapshot(
        &self,
        selected_node_id: Option<u32>,
        stage_width: Option<u32>,
        stage_height: Option<u32>,
        locale: Option<&str>,
    ) -> PyComposerSnapshot {
        if stage_width.is_none() && stage_height.is_none() && locale.is_none() {
            if let Some(node_id) = selected_node_id {
                if let Some(snapshot) = self.session.read_model().preview_data_for_node(node_id) {
                    return snapshot.clone().into();
                }
            }
        }
        let resolution = stage_width.zip(stage_height);
        let mut snapshot = build_composer_snapshot(
            &self.inner,
            selected_node_id,
            resolution,
            None,
            locale,
            None,
        );
        apply_layer_overrides(&mut snapshot.objects, &self.layer_overrides);
        snapshot.into()
    }

    pub(super) fn py_list_layered_objects(
        &self,
        selected_node_id: Option<u32>,
    ) -> Vec<PyLayeredSceneObject> {
        if selected_node_id.is_none() {
            return self
                .session
                .read_model()
                .composer_layers()
                .iter()
                .cloned()
                .map(Into::into)
                .collect();
        }
        let mut objects = collect_layered_objects(&self.inner, selected_node_id);
        apply_layer_overrides(&mut objects, &self.layer_overrides);
        objects.into_iter().map(Into::into).collect()
    }

    pub(super) fn py_list_stage_layers(&self) -> Vec<String> {
        stage_layer_names()
    }

    pub(super) fn py_set_layer_visible(
        &mut self,
        object_id: &str,
        visible: bool,
    ) -> pyo3::PyResult<()> {
        if self
            .session
            .read_model()
            .composer_layer(object_id)
            .is_some_and(|layer| layer.visible == visible)
        {
            return Ok(());
        }
        self.apply_document_command(AuthoringDocumentCommand::SetLayerVisible {
            object_id: object_id.to_string(),
            visible,
        })
        .map_err(|err| py_command_error("set_layer_visible failed", err))?;
        Ok(())
    }

    pub(super) fn py_set_layer_locked(
        &mut self,
        object_id: &str,
        locked: bool,
    ) -> pyo3::PyResult<()> {
        if self
            .session
            .read_model()
            .composer_layer(object_id)
            .is_some_and(|layer| layer.locked == locked)
        {
            return Ok(());
        }
        self.apply_document_command(AuthoringDocumentCommand::SetLayerLocked {
            object_id: object_id.to_string(),
            locked,
        })
        .map_err(|err| py_command_error("set_layer_locked failed", err))?;
        Ok(())
    }

    pub(super) fn py_move_scene_object(
        &mut self,
        object_id: &str,
        x: i32,
        y: i32,
        scale: Option<f32>,
    ) -> pyo3::PyResult<bool> {
        if self
            .session
            .read_model()
            .composer_layer(object_id)
            .is_some_and(|layer| layer.locked || !layer.visible)
        {
            return Ok(false);
        }
        self.apply_authoring_command(AuthoringCommand::MoveLayer {
            object_id: object_id.to_string(),
            x,
            y,
            scale,
        })
        .map(|_| true)
        .map_err(|err| py_command_error("move_scene_object failed", err))
    }

    pub(super) fn py_preview_start_from_node(
        &self,
        node_id: u32,
    ) -> pyo3::PyResult<PyComposerPreviewSession> {
        PyComposerPreviewSession::start(&self.inner, node_id)
    }

    pub(super) fn py_edit_dialogue(
        &mut self,
        node_id: u32,
        speaker: &str,
        text: &str,
    ) -> pyo3::PyResult<bool> {
        self.apply_authoring_command(AuthoringCommand::EditDialogue {
            node_id,
            speaker: speaker.to_string(),
            text: text.to_string(),
        })
        .map(|_| true)
        .map_err(|err| py_command_error("edit_dialogue failed", err))
    }

    pub(super) fn py_edit_choice_prompt(
        &mut self,
        node_id: u32,
        prompt: &str,
    ) -> pyo3::PyResult<bool> {
        self.apply_authoring_command(AuthoringCommand::EditChoicePrompt {
            node_id,
            prompt: prompt.to_string(),
        })
        .map(|_| true)
        .map_err(|err| py_command_error("edit_choice_prompt failed", err))
    }

    pub(super) fn py_edit_choice_option_text(
        &mut self,
        node_id: u32,
        option_index: usize,
        text: &str,
    ) -> pyo3::PyResult<bool> {
        self.apply_authoring_command(AuthoringCommand::EditChoiceOptionText {
            node_id,
            option_index,
            text: text.to_string(),
        })
        .map(|_| true)
        .map_err(|err| py_command_error("edit_choice_option_text failed", err))
    }

    pub(super) fn py_reorder_choice_option(
        &mut self,
        node_id: u32,
        from_index: usize,
        to_index: usize,
    ) -> pyo3::PyResult<bool> {
        self.apply_authoring_command(AuthoringCommand::ReorderChoiceOption {
            node_id,
            from_index,
            to_index,
        })
        .map(|_| true)
        .map_err(|err| py_command_error("reorder_choice_option failed", err))
    }

    pub(super) fn py_set_choice_option_target(
        &mut self,
        node_id: u32,
        option_index: usize,
        target_node_id: Option<u32>,
    ) -> pyo3::PyResult<bool> {
        self.apply_authoring_command(AuthoringCommand::SetChoiceOptionTarget {
            node_id,
            option_index,
            target_node_id,
        })
        .map(|_| true)
        .map_err(|err| py_command_error("set_choice_option_target failed", err))
    }
}

#[cfg(test)]
#[cfg(test)]
#[path = "composer_tests.rs"]
mod tests;
