use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::collections::BTreeMap;
use visual_novel_engine::authoring::composer::LayerOverride;
use visual_novel_engine::authoring::{
    parse_authoring_document_or_script, validate_authoring_graph, validate_authoring_graph_no_io,
    validate_authoring_graph_with_project_root, AuthoringDocument, AuthoringPosition,
    AuthoringValidationReport, NodeGraph, OperationKind, OperationLogEntry, VerificationRun,
    NODE_VERTICAL_SPACING,
};

use super::api_v2::{
    PyAuthoringValidationReport, PyComposerPreviewSession, PyComposerSnapshot, PyFragmentPort,
    PyGraphFragment, PyLayeredSceneObject, PyOperationLogEntry, PyVerificationRun,
};
use super::diagnostics::{PyLintIssue, PyQuickFixCandidate};
use super::story_node::PyStoryNode;
use operation_trace::{PythonOperation, PythonOperationTrace};

/// A graph of story nodes with connections.
#[pyclass(name = "NodeGraph")]
pub struct PyNodeGraph {
    inner: NodeGraph,
    layer_overrides: BTreeMap<String, LayerOverride>,
    operation_log: Vec<OperationLogEntry>,
    verification_runs: Vec<VerificationRun>,
}

#[path = "editor_node_graph/composer.rs"]
mod composer;
#[path = "editor_node_graph/fragments.rs"]
mod fragments;
#[path = "editor_node_graph/internal.rs"]
mod internal;
#[path = "editor_node_graph/misc.rs"]
mod misc;
#[path = "editor_node_graph/operation_trace.rs"]
mod operation_trace;

#[pymethods]
impl PyNodeGraph {
    #[new]
    fn new() -> Self {
        Self {
            inner: NodeGraph::new(),
            layer_overrides: BTreeMap::new(),
            operation_log: Vec::new(),
            verification_runs: Vec::new(),
        }
    }

    fn add_node(&mut self, node: PyStoryNode, x: f32, y: f32) -> u32 {
        let before = self.trace_before_mutation();
        let node = node.into_inner();
        let after_value = serde_json::to_string(&node).ok();
        let id = self.inner.add_node(node, AuthoringPosition::new(x, y));
        self.record_python_operation(
            PythonOperation::new(
                OperationKind::NodeCreated,
                format!("Created node {id} from Python"),
            )
            .with_field_path(format!("graph.nodes[{id}]"))
            .with_values(None, after_value),
            before,
        );
        id
    }

    fn connect(&mut self, from_id: u32, to_id: u32) {
        let before_connections = self.connections();
        let before = self.trace_before_mutation();
        self.inner.connect(from_id, to_id);
        if self.connections() != before_connections {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::NodeConnected,
                    format!("Connected node {from_id} to node {to_id} from Python"),
                )
                .with_field_path(format!("graph.edges[{from_id}:0]"))
                .with_values(None, Some(format!("{from_id}:0->{to_id}"))),
                before,
            );
        }
    }

    fn connect_port(&mut self, from_id: u32, from_port: usize, to_id: u32) {
        let before_connections = self.connections();
        let before = self.trace_before_mutation();
        self.inner.connect_port(from_id, from_port, to_id);
        if self.connections() != before_connections {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::NodeConnected,
                    format!(
                        "Connected node {from_id} port {from_port} to node {to_id} from Python"
                    ),
                )
                .with_field_path(format!("graph.edges[{from_id}:{from_port}]"))
                .with_values(None, Some(format!("{from_id}:{from_port}->{to_id}"))),
                before,
            );
        }
    }

    #[pyo3(signature = (choice_id, to_id, text="New route"))]
    fn connect_new_choice_option(
        &mut self,
        choice_id: u32,
        to_id: u32,
        text: &str,
    ) -> PyResult<usize> {
        let before = self.trace_before_mutation();
        self.inner
            .connect_new_choice_option(choice_id, to_id, text)
            .inspect(|port| {
                self.record_python_operation(
                    PythonOperation::new(
                        OperationKind::NodeConnected,
                        format!(
                            "Connected choice {choice_id} option {port} to node {to_id} from Python"
                        ),
                    )
                    .with_field_path(format!("graph.nodes[{choice_id}].options[{port}]"))
                    .with_values(None, Some(text.to_string())),
                    before,
                );
            })
            .ok_or_else(|| {
                PyValueError::new_err("source node is not a choice or target is invalid")
            })
    }

    #[pyo3(signature = (from_id, from_port, to_id, branch_x=None, branch_y=None))]
    fn connect_or_branch(
        &mut self,
        from_id: u32,
        from_port: usize,
        to_id: u32,
        branch_x: Option<f32>,
        branch_y: Option<f32>,
    ) -> bool {
        let branch_pos = match (branch_x, branch_y) {
            (Some(x), Some(y)) => AuthoringPosition::new(x, y),
            _ => self
                .inner
                .get_node_pos(from_id)
                .map(|pos| AuthoringPosition::new(pos.x, pos.y + NODE_VERTICAL_SPACING))
                .unwrap_or_default(),
        };
        let before = self.trace_before_mutation();
        let changed = self
            .inner
            .connect_or_branch(from_id, from_port, to_id, branch_pos);
        if changed {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::NodeConnected,
                    format!(
                        "Connected or branched node {from_id} port {from_port} to node {to_id} from Python"
                    ),
                )
                .with_field_path(format!("graph.edges[{from_id}:{from_port}]"))
                .with_values(None, Some(format!("{from_id}:{from_port}->{to_id}"))),
                before,
            );
        }
        changed
    }

    fn remove_node(&mut self, node_id: u32) {
        let before_value = self
            .inner
            .get_node(node_id)
            .and_then(|node| serde_json::to_string(node).ok());
        let before = self.trace_before_mutation();
        let existed = before_value.is_some();
        self.inner.remove_node(node_id);
        if existed {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::NodeRemoved,
                    format!("Removed node {node_id} from Python"),
                )
                .with_field_path(format!("graph.nodes[{node_id}]"))
                .with_values(before_value, None),
                before,
            );
        }
    }

    fn node_count(&self) -> usize {
        self.inner.len()
    }

    fn connection_count(&self) -> usize {
        self.inner.connection_count()
    }

    fn node_ids(&self) -> Vec<u32> {
        self.inner.nodes().map(|(id, _, _)| *id).collect()
    }

    fn get_node(&self, node_id: u32) -> Option<PyStoryNode> {
        self.inner.get_node(node_id).cloned().map(Into::into)
    }

    fn node_position(&self, node_id: u32) -> Option<(f32, f32)> {
        self.inner
            .get_node_pos(node_id)
            .map(|position| (position.x, position.y))
    }

    fn nodes(&self) -> Vec<(u32, PyStoryNode, f32, f32)> {
        self.inner
            .nodes()
            .map(|(id, node, position)| (*id, node.clone().into(), position.x, position.y))
            .collect()
    }

    fn connections(&self) -> Vec<(u32, usize, u32)> {
        self.inner
            .connections()
            .map(|connection| (connection.from, connection.from_port, connection.to))
            .collect()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn to_script_json(&self) -> PyResult<String> {
        let script = self
            .inner
            .to_script_strict()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        script
            .to_json()
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[staticmethod]
    fn from_script_json(script_json: &str) -> PyResult<Self> {
        let inner = parse_authoring_document_or_script(script_json)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(Self {
            inner,
            layer_overrides: BTreeMap::new(),
            operation_log: Vec::new(),
            verification_runs: Vec::new(),
        })
    }

    fn to_lossy_script_json_for_diagnostics(&self) -> PyResult<String> {
        let script = self.inner.to_script_lossy_for_diagnostics();
        script
            .to_json()
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn validate_no_io(&self) -> Vec<PyLintIssue> {
        validate_authoring_graph_no_io(&self.inner)
            .into_iter()
            .map(PyLintIssue::from)
            .collect()
    }

    #[pyo3(signature = (project_root=None))]
    fn validate(&self, project_root: Option<&str>) -> Vec<PyLintIssue> {
        let issues = if let Some(project_root) = project_root {
            validate_authoring_graph_with_project_root(
                &self.inner,
                std::path::Path::new(project_root),
            )
        } else {
            validate_authoring_graph(&self.inner)
        };
        issues.into_iter().map(PyLintIssue::from).collect()
    }

    #[pyo3(signature = (project_root=None))]
    fn validation_report(
        &self,
        project_root: Option<&str>,
    ) -> PyResult<PyAuthoringValidationReport> {
        let issues = if let Some(project_root) = project_root {
            validate_authoring_graph_with_project_root(
                &self.inner,
                std::path::Path::new(project_root),
            )
        } else {
            validate_authoring_graph_no_io(&self.inner)
        };
        let script = self.inner.to_script_lossy_for_diagnostics();
        let document = self.to_authoring_document();
        Ok(AuthoringValidationReport::from_document_and_issues(&document, &script, &issues).into())
    }

    #[staticmethod]
    fn from_authoring_or_script_json(source: &str) -> PyResult<Self> {
        if let Ok(document) = AuthoringDocument::from_json(source) {
            return Ok(Self {
                inner: document.graph,
                layer_overrides: document.composer_layer_overrides,
                operation_log: document.operation_log,
                verification_runs: document.verification_runs,
            });
        }
        let inner = parse_authoring_document_or_script(source)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(Self {
            inner,
            layer_overrides: BTreeMap::new(),
            operation_log: Vec::new(),
            verification_runs: Vec::new(),
        })
    }

    fn search_nodes(&self, query: &str) -> Vec<u32> {
        self.inner.search_nodes(query)
    }

    fn create_fragment(&mut self, fragment_id: String, title: String, node_ids: Vec<u32>) -> bool {
        self.py_create_fragment(fragment_id, title, node_ids)
    }
    fn remove_fragment(&mut self, fragment_id: &str) -> bool {
        self.py_remove_fragment(fragment_id)
    }
    fn list_fragments(&self) -> Vec<PyGraphFragment> {
        self.py_list_fragments()
    }
    fn get_fragment(&self, fragment_id: &str) -> Option<PyGraphFragment> {
        self.py_get_fragment(fragment_id)
    }
    fn enter_fragment(&mut self, fragment_id: &str) -> bool {
        self.py_enter_fragment(fragment_id)
    }
    fn leave_fragment(&mut self) -> bool {
        self.py_leave_fragment()
    }
    fn active_fragment(&self) -> Option<String> {
        self.py_active_fragment()
    }

    fn fragment_ports(
        &self,
        fragment_id: &str,
    ) -> Option<(Vec<PyFragmentPort>, Vec<PyFragmentPort>)> {
        self.py_fragment_ports(fragment_id)
    }

    fn refresh_fragment_ports(&mut self, fragment_id: &str) -> bool {
        self.py_refresh_fragment_ports(fragment_id)
    }

    fn validate_fragments(&self) -> Vec<PyLintIssue> {
        self.py_validate_fragments()
    }

    fn operation_log(&self) -> Vec<PyOperationLogEntry> {
        self.py_operation_log()
    }

    fn verification_runs(&self) -> Vec<PyVerificationRun> {
        self.py_verification_runs()
    }

    #[pyo3(signature = (selected_node_id=None, stage_width=None, stage_height=None, locale=None))]
    fn compose_scene_snapshot(
        &self,
        selected_node_id: Option<u32>,
        stage_width: Option<u32>,
        stage_height: Option<u32>,
        locale: Option<&str>,
    ) -> PyComposerSnapshot {
        self.py_compose_scene_snapshot(selected_node_id, stage_width, stage_height, locale)
    }

    #[pyo3(signature = (selected_node_id=None))]
    fn list_layered_objects(&self, selected_node_id: Option<u32>) -> Vec<PyLayeredSceneObject> {
        self.py_list_layered_objects(selected_node_id)
    }

    fn list_stage_layers(&self) -> Vec<String> {
        self.py_list_stage_layers()
    }

    fn set_layer_visible(&mut self, object_id: &str, visible: bool) {
        self.py_set_layer_visible(object_id, visible);
    }

    fn set_layer_locked(&mut self, object_id: &str, locked: bool) {
        self.py_set_layer_locked(object_id, locked);
    }

    #[pyo3(signature = (object_id, x, y, scale=None))]
    fn move_scene_object(&mut self, object_id: &str, x: i32, y: i32, scale: Option<f32>) -> bool {
        self.py_move_scene_object(object_id, x, y, scale)
    }
    fn edit_dialogue(&mut self, node_id: u32, speaker: &str, text: &str) -> bool {
        self.py_edit_dialogue(node_id, speaker, text)
    }
    fn edit_choice_prompt(&mut self, node_id: u32, prompt: &str) -> bool {
        self.py_edit_choice_prompt(node_id, prompt)
    }
    fn edit_choice_option_text(&mut self, node_id: u32, option_index: usize, text: &str) -> bool {
        self.py_edit_choice_option_text(node_id, option_index, text)
    }
    fn reorder_choice_option(&mut self, node_id: u32, from_index: usize, to_index: usize) -> bool {
        self.py_reorder_choice_option(node_id, from_index, to_index)
    }
    #[pyo3(signature = (node_id, option_index, target_node_id=None))]
    fn set_choice_option_target(
        &mut self,
        node_id: u32,
        option_index: usize,
        target_node_id: Option<u32>,
    ) -> bool {
        self.py_set_choice_option_target(node_id, option_index, target_node_id)
    }

    fn preview_start_from_node(&self, node_id: u32) -> PyResult<PyComposerPreviewSession> {
        self.py_preview_start_from_node(node_id)
    }

    fn fix_candidates(&self, issue_index: usize) -> PyResult<Vec<PyQuickFixCandidate>> {
        self.py_fix_candidates(issue_index)
    }

    #[pyo3(signature = (issue_index, include_review=false))]
    fn autofix_issue(
        &mut self,
        issue_index: usize,
        include_review: bool,
    ) -> PyResult<Option<String>> {
        self.py_autofix_issue(issue_index, include_review)
    }

    fn autofix_safe(&mut self) -> PyResult<usize> {
        self.py_autofix_safe()
    }

    fn autofix_full(&mut self) -> PyResult<usize> {
        self.py_autofix_full()
    }

    fn set_bookmark(&mut self, name: String, node_id: u32) -> bool {
        self.py_set_bookmark(name, node_id)
    }

    fn remove_bookmark(&mut self, name: &str) -> bool {
        self.py_remove_bookmark(name)
    }

    fn bookmark_target(&self, name: &str) -> Option<u32> {
        self.py_bookmark_target(name)
    }

    fn list_bookmarks(&self) -> Vec<(String, u32)> {
        self.py_list_bookmarks()
    }

    fn save(&self, path: &str) -> PyResult<()> {
        self.py_save(path)
    }

    #[staticmethod]
    fn load(path: &str) -> PyResult<Self> {
        Self::py_load(path)
    }

    fn __repr__(&self) -> String {
        self.py_repr()
    }
}
