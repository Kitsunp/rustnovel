use visual_novel_engine::authoring::composer::{
    apply_layer_overrides, compose_scene_snapshot as build_composer_snapshot,
    list_layered_objects as collect_layered_objects, move_scene_object as apply_scene_object_move,
    set_layer_locked, set_layer_visible,
};
use visual_novel_engine::authoring::{OperationKind, StoryNode};

use super::super::api_v2::{
    stage_layer_names, PyComposerPreviewSession, PyComposerSnapshot, PyLayeredSceneObject,
    PyOperationLogEntry, PyVerificationRun,
};
use super::{PyNodeGraph, PythonOperation};

impl PyNodeGraph {
    pub(super) fn py_operation_log(&self) -> Vec<PyOperationLogEntry> {
        self.operation_log
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub(super) fn py_verification_runs(&self) -> Vec<PyVerificationRun> {
        self.verification_runs
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
        let mut objects = collect_layered_objects(&self.inner, selected_node_id);
        apply_layer_overrides(&mut objects, &self.layer_overrides);
        objects.into_iter().map(Into::into).collect()
    }

    pub(super) fn py_list_stage_layers(&self) -> Vec<String> {
        stage_layer_names()
    }

    pub(super) fn py_set_layer_visible(&mut self, object_id: &str, visible: bool) {
        let before_value = self
            .layer_overrides
            .get(object_id)
            .and_then(|override_| serde_json::to_string(override_).ok());
        let before = self.trace_before_mutation();
        set_layer_visible(&mut self.layer_overrides, object_id, visible);
        let after_value = self
            .layer_overrides
            .get(object_id)
            .and_then(|override_| serde_json::to_string(override_).ok());
        if before_value != after_value {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::LayerVisibilityChanged,
                    format!("Set layer {object_id} visible={visible} from Python"),
                )
                .with_field_path(format!("composer.objects[{object_id}].visible"))
                .with_values(before_value, after_value),
                before,
            );
        }
    }

    pub(super) fn py_set_layer_locked(&mut self, object_id: &str, locked: bool) {
        let before_value = self
            .layer_overrides
            .get(object_id)
            .and_then(|override_| serde_json::to_string(override_).ok());
        let before = self.trace_before_mutation();
        set_layer_locked(&mut self.layer_overrides, object_id, locked);
        let after_value = self
            .layer_overrides
            .get(object_id)
            .and_then(|override_| serde_json::to_string(override_).ok());
        if before_value != after_value {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::LayerLockChanged,
                    format!("Set layer {object_id} locked={locked} from Python"),
                )
                .with_field_path(format!("composer.objects[{object_id}].locked"))
                .with_values(before_value, after_value),
                before,
            );
        }
    }

    pub(super) fn py_move_scene_object(
        &mut self,
        object_id: &str,
        x: i32,
        y: i32,
        scale: Option<f32>,
    ) -> bool {
        if self
            .layer_overrides
            .get(object_id)
            .is_some_and(|override_| override_.locked || !override_.visible)
        {
            return false;
        }
        let before_value = self.layered_object_json(object_id);
        let before = self.trace_before_mutation();
        let changed = apply_scene_object_move(&mut self.inner, object_id, x, y, scale);
        if changed {
            let after_value = self.layered_object_json(object_id);
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::ComposerObjectMoved,
                    format!("Moved composer object {object_id} from Python"),
                )
                .with_field_path(format!("composer.objects[{object_id}].position"))
                .with_values(before_value, after_value),
                before,
            );
        }
        changed
    }

    pub(super) fn py_preview_start_from_node(
        &self,
        node_id: u32,
    ) -> pyo3::PyResult<PyComposerPreviewSession> {
        PyComposerPreviewSession::start(&self.inner, node_id)
    }

    pub(super) fn py_edit_dialogue(&mut self, node_id: u32, speaker: &str, text: &str) -> bool {
        let before_value = node_json(&self.inner, node_id);
        let before = self.trace_before_mutation();
        let Some(StoryNode::Dialogue {
            speaker: current_speaker,
            text: current_text,
        }) = self.inner.get_node_mut(node_id)
        else {
            return false;
        };
        if current_speaker == speaker && current_text == text {
            return false;
        }
        *current_speaker = speaker.to_string();
        *current_text = text.to_string();
        self.record_field_edit(
            node_id,
            "dialogue",
            "Edited dialogue overlay from Python",
            before_value,
            before,
        );
        true
    }

    pub(super) fn py_edit_choice_prompt(&mut self, node_id: u32, prompt: &str) -> bool {
        let before_value = node_json(&self.inner, node_id);
        let before = self.trace_before_mutation();
        let Some(StoryNode::Choice {
            prompt: current, ..
        }) = self.inner.get_node_mut(node_id)
        else {
            return false;
        };
        if current == prompt {
            return false;
        }
        *current = prompt.to_string();
        self.record_field_edit(
            node_id,
            "choice.prompt",
            "Edited choice prompt from Python",
            before_value,
            before,
        );
        true
    }

    pub(super) fn py_edit_choice_option_text(
        &mut self,
        node_id: u32,
        option_index: usize,
        text: &str,
    ) -> bool {
        let before_value = node_json(&self.inner, node_id);
        let before = self.trace_before_mutation();
        let Some(StoryNode::Choice { options, .. }) = self.inner.get_node_mut(node_id) else {
            return false;
        };
        let Some(option) = options.get_mut(option_index) else {
            return false;
        };
        if option == text {
            return false;
        }
        *option = text.to_string();
        self.record_field_edit(
            node_id,
            &format!("choice.options[{option_index}].text"),
            "Edited choice option text from Python",
            before_value,
            before,
        );
        true
    }

    pub(super) fn py_reorder_choice_option(
        &mut self,
        node_id: u32,
        from_index: usize,
        to_index: usize,
    ) -> bool {
        let before_value = node_json(&self.inner, node_id);
        let before = self.trace_before_mutation();
        let option_count = match self.inner.get_node_mut(node_id) {
            Some(StoryNode::Choice { options, .. }) => {
                if from_index >= options.len()
                    || to_index >= options.len()
                    || from_index == to_index
                {
                    return false;
                }
                let option = options.remove(from_index);
                options.insert(to_index, option);
                options.len()
            }
            _ => return false,
        };
        remap_choice_connections(&mut self.inner, node_id, option_count, from_index, to_index);
        self.record_field_edit(
            node_id,
            "choice.options",
            "Reordered choice options from Python",
            before_value,
            before,
        );
        true
    }

    pub(super) fn py_set_choice_option_target(
        &mut self,
        node_id: u32,
        option_index: usize,
        target_node_id: Option<u32>,
    ) -> bool {
        let Some(StoryNode::Choice { options, .. }) = self.inner.get_node(node_id) else {
            return false;
        };
        if option_index >= options.len() {
            return false;
        }
        let before_value = node_json(&self.inner, node_id);
        let before_connections = self.connections_tuple();
        let before = self.trace_before_mutation();
        match target_node_id {
            Some(target) if self.inner.get_node(target).is_some() => {
                self.inner.connect_port(node_id, option_index, target);
            }
            Some(_) => return false,
            None => self.inner.disconnect_port(node_id, option_index),
        }
        if self.connections_tuple() == before_connections {
            return false;
        }
        self.record_field_edit(
            node_id,
            &format!("choice.options[{option_index}].target"),
            "Edited choice option target from Python",
            before_value,
            before,
        );
        true
    }

    fn record_field_edit(
        &mut self,
        node_id: u32,
        field: &str,
        details: &str,
        before_value: Option<String>,
        before: super::PythonOperationTrace,
    ) {
        self.record_python_operation(
            PythonOperation::new(
                OperationKind::FieldEdited,
                format!("{details} on node {node_id}"),
            )
            .with_field_path(format!("graph.nodes[{node_id}].{field}"))
            .with_values(before_value, node_json(&self.inner, node_id)),
            before,
        );
    }

    fn connections_tuple(&self) -> Vec<(u32, usize, u32)> {
        self.inner
            .connections()
            .map(|connection| (connection.from, connection.from_port, connection.to))
            .collect()
    }
}

fn node_json(graph: &visual_novel_engine::authoring::NodeGraph, node_id: u32) -> Option<String> {
    graph
        .get_node(node_id)
        .and_then(|node| serde_json::to_string(node).ok())
}

fn remap_choice_connections(
    graph: &mut visual_novel_engine::authoring::NodeGraph,
    node_id: u32,
    option_count: usize,
    from_index: usize,
    to_index: usize,
) {
    let connections = graph
        .connections()
        .filter(|connection| connection.from == node_id && connection.from_port < option_count)
        .map(|connection| (connection.from_port, connection.to))
        .collect::<Vec<_>>();
    for (port, _) in &connections {
        graph.disconnect_port(node_id, *port);
    }
    for (old_port, target) in connections {
        graph.connect_port(
            node_id,
            remapped_port(old_port, from_index, to_index),
            target,
        );
    }
}

fn remapped_port(old_port: usize, from_index: usize, to_index: usize) -> usize {
    match from_index.cmp(&to_index) {
        std::cmp::Ordering::Less if old_port == from_index => to_index,
        std::cmp::Ordering::Less if old_port > from_index && old_port <= to_index => old_port - 1,
        std::cmp::Ordering::Greater if old_port == from_index => to_index,
        std::cmp::Ordering::Greater if old_port >= to_index && old_port < from_index => {
            old_port + 1
        }
        _ => old_port,
    }
}
