use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::collections::BTreeMap;
use visual_novel_engine::authoring::composer::{
    apply_layer_overrides, compose_scene_snapshot, list_layered_objects, move_scene_object,
    set_layer_locked, set_layer_visible, LayerOverride,
};
use visual_novel_engine::authoring::quick_fix::{apply_fix, suggest_fixes};
use visual_novel_engine::authoring::{
    build_authoring_document_report_fingerprint, load_authoring_document_or_script,
    parse_authoring_document_or_script, validate_authoring_graph, validate_authoring_graph_no_io,
    validate_authoring_graph_with_project_root, AuthoringDocument, AuthoringPosition,
    AuthoringReportFingerprint, AuthoringValidationReport, LintIssue, NodeGraph, OperationKind,
    OperationLogEntry, VerificationRun, NODE_VERTICAL_SPACING,
};

use super::api_v2::{
    stage_layer_names, PyAuthoringValidationReport, PyComposerPreviewSession, PyComposerSnapshot,
    PyFragmentPort, PyGraphFragment, PyLayeredSceneObject, PyOperationLogEntry, PyVerificationRun,
};
use super::diagnostics::{PyLintIssue, PyQuickFixCandidate};
use super::story_node::PyStoryNode;
use super::support::{apply_autofix_pass, select_fix_candidate};

/// A graph of story nodes with connections.
#[pyclass(name = "NodeGraph")]
pub struct PyNodeGraph {
    inner: NodeGraph,
    layer_overrides: BTreeMap<String, LayerOverride>,
    operation_log: Vec<OperationLogEntry>,
    verification_runs: Vec<VerificationRun>,
}

struct PythonOperationTrace {
    fingerprint: AuthoringReportFingerprint,
    issues: Vec<LintIssue>,
}

struct PythonOperation {
    kind: OperationKind,
    details: String,
    field_path: Option<String>,
    before_value: Option<String>,
    after_value: Option<String>,
}

impl PythonOperation {
    fn new(kind: OperationKind, details: impl Into<String>) -> Self {
        Self {
            kind,
            details: details.into(),
            field_path: None,
            before_value: None,
            after_value: None,
        }
    }

    fn with_field_path(mut self, field_path: impl Into<String>) -> Self {
        self.field_path = Some(field_path.into());
        self
    }

    fn with_values(mut self, before_value: Option<String>, after_value: Option<String>) -> Self {
        self.before_value = before_value;
        self.after_value = after_value;
        self
    }
}

impl PyNodeGraph {
    pub(super) fn inner(&self) -> &NodeGraph {
        &self.inner
    }

    fn to_authoring_document(&self) -> AuthoringDocument {
        let mut document = AuthoringDocument::new(self.inner.clone());
        document.composer_layer_overrides = self.layer_overrides.clone();
        document.operation_log = self.operation_log.clone();
        document.verification_runs = self.verification_runs.clone();
        document
    }

    fn current_fingerprint(&self) -> AuthoringReportFingerprint {
        let script = self.inner.to_script_lossy_for_diagnostics();
        build_authoring_document_report_fingerprint(&self.to_authoring_document(), &script)
    }

    fn layered_object_json(&self, object_id: &str) -> Option<String> {
        let mut objects = list_layered_objects(&self.inner, None);
        apply_layer_overrides(&mut objects, &self.layer_overrides);
        objects
            .into_iter()
            .find(|object| object.object_id == object_id)
            .and_then(|object| serde_json::to_string(&object).ok())
    }

    fn trace_before_mutation(&self) -> PythonOperationTrace {
        PythonOperationTrace {
            fingerprint: self.current_fingerprint(),
            issues: validate_authoring_graph_no_io(&self.inner),
        }
    }

    fn record_python_operation(
        &mut self,
        operation: PythonOperation,
        before: PythonOperationTrace,
    ) {
        let after_fingerprint = self.current_fingerprint();
        let after_issues = validate_authoring_graph_no_io(&self.inner);
        let mut entry = OperationLogEntry::new_typed(operation.kind, "applied", operation.details)
            .with_before_after_fingerprints(&before.fingerprint, &after_fingerprint);
        if let Some(field_path) = operation.field_path {
            entry = entry.with_field_path(field_path);
        }
        entry.before_value = operation.before_value;
        entry.after_value = operation.after_value;
        let operation_id = entry.operation_id.clone();
        self.operation_log.push(entry);
        self.verification_runs
            .push(VerificationRun::from_diagnostics(
                operation_id,
                "python-api",
                &after_fingerprint,
                &before.issues,
                &after_issues,
            ));
    }
}

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
        let before = self.trace_before_mutation();
        let changed = self
            .inner
            .create_fragment(fragment_id.clone(), title, node_ids.clone());
        if changed {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::FragmentCreated,
                    format!("Created fragment {fragment_id} from Python"),
                )
                .with_field_path(format!("graph.fragments[{fragment_id}]"))
                .with_values(None, serde_json::to_string(&node_ids).ok()),
                before,
            );
        }
        changed
    }

    fn remove_fragment(&mut self, fragment_id: &str) -> bool {
        let before_value = self
            .inner
            .get_fragment(fragment_id)
            .and_then(|fragment| serde_json::to_string(fragment).ok());
        let before = self.trace_before_mutation();
        let changed = self.inner.remove_fragment(fragment_id).is_some();
        if changed {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::FragmentRemoved,
                    format!("Removed fragment {fragment_id} from Python"),
                )
                .with_field_path(format!("graph.fragments[{fragment_id}]"))
                .with_values(before_value, None),
                before,
            );
        }
        changed
    }

    fn list_fragments(&self) -> Vec<PyGraphFragment> {
        self.inner
            .list_fragments()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    fn get_fragment(&self, fragment_id: &str) -> Option<PyGraphFragment> {
        self.inner
            .get_fragment(fragment_id)
            .cloned()
            .map(Into::into)
    }

    fn enter_fragment(&mut self, fragment_id: &str) -> bool {
        let before_value = self.inner.active_fragment().map(str::to_string);
        let before = self.trace_before_mutation();
        let changed = self.inner.enter_fragment(fragment_id);
        if changed {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::FragmentEntered,
                    format!("Entered fragment {fragment_id} from Python"),
                )
                .with_field_path("graph.active_fragment")
                .with_values(
                    before_value,
                    self.inner.active_fragment().map(str::to_string),
                ),
                before,
            );
        }
        changed
    }

    fn leave_fragment(&mut self) -> bool {
        let before_value = self.inner.active_fragment().map(str::to_string);
        let before = self.trace_before_mutation();
        let changed = self.inner.leave_fragment();
        if changed {
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::FragmentLeft,
                    "Left active fragment from Python",
                )
                .with_field_path("graph.active_fragment")
                .with_values(
                    before_value,
                    self.inner.active_fragment().map(str::to_string),
                ),
                before,
            );
        }
        changed
    }

    fn active_fragment(&self) -> Option<String> {
        self.inner.active_fragment().map(str::to_string)
    }

    fn fragment_ports(
        &self,
        fragment_id: &str,
    ) -> Option<(Vec<PyFragmentPort>, Vec<PyFragmentPort>)> {
        self.inner
            .fragment_ports(fragment_id)
            .map(|(inputs, outputs)| {
                (
                    inputs.into_iter().map(Into::into).collect(),
                    outputs.into_iter().map(Into::into).collect(),
                )
            })
    }

    fn refresh_fragment_ports(&mut self, fragment_id: &str) -> bool {
        let before_value = self
            .inner
            .get_fragment(fragment_id)
            .and_then(|fragment| serde_json::to_string(fragment).ok());
        let before = self.trace_before_mutation();
        let changed = self.inner.refresh_fragment_ports(fragment_id);
        if changed {
            let after_value = self
                .inner
                .get_fragment(fragment_id)
                .and_then(|fragment| serde_json::to_string(fragment).ok());
            self.record_python_operation(
                PythonOperation::new(
                    OperationKind::FieldEdited,
                    format!("Refreshed fragment {fragment_id} ports from Python"),
                )
                .with_field_path(format!("graph.fragments[{fragment_id}].ports"))
                .with_values(before_value, after_value),
                before,
            );
        }
        changed
    }

    fn validate_fragments(&self) -> Vec<PyLintIssue> {
        self.inner
            .validate_fragments()
            .into_iter()
            .map(PyLintIssue::from)
            .collect()
    }

    fn operation_log(&self) -> Vec<PyOperationLogEntry> {
        self.operation_log
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    fn verification_runs(&self) -> Vec<PyVerificationRun> {
        self.verification_runs
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    #[pyo3(signature = (selected_node_id=None, stage_width=None, stage_height=None, locale=None))]
    fn compose_scene_snapshot(
        &self,
        selected_node_id: Option<u32>,
        stage_width: Option<u32>,
        stage_height: Option<u32>,
        locale: Option<&str>,
    ) -> PyComposerSnapshot {
        let resolution = stage_width.zip(stage_height);
        let mut snapshot = compose_scene_snapshot(
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

    #[pyo3(signature = (selected_node_id=None))]
    fn list_layered_objects(&self, selected_node_id: Option<u32>) -> Vec<PyLayeredSceneObject> {
        let mut objects = list_layered_objects(&self.inner, selected_node_id);
        apply_layer_overrides(&mut objects, &self.layer_overrides);
        objects.into_iter().map(Into::into).collect()
    }

    fn list_stage_layers(&self) -> Vec<String> {
        stage_layer_names()
    }

    fn set_layer_visible(&mut self, object_id: &str, visible: bool) {
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

    fn set_layer_locked(&mut self, object_id: &str, locked: bool) {
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

    #[pyo3(signature = (object_id, x, y, scale=None))]
    fn move_scene_object(&mut self, object_id: &str, x: i32, y: i32, scale: Option<f32>) -> bool {
        if self
            .layer_overrides
            .get(object_id)
            .is_some_and(|override_| override_.locked || !override_.visible)
        {
            return false;
        }
        let before_value = self.layered_object_json(object_id);
        let before = self.trace_before_mutation();
        let changed = move_scene_object(&mut self.inner, object_id, x, y, scale);
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

    fn preview_start_from_node(&self, node_id: u32) -> PyResult<PyComposerPreviewSession> {
        PyComposerPreviewSession::start(&self.inner, node_id)
    }

    fn fix_candidates(&self, issue_index: usize) -> PyResult<Vec<PyQuickFixCandidate>> {
        let issues = validate_authoring_graph(&self.inner);
        let issue = issues
            .get(issue_index)
            .ok_or_else(|| PyValueError::new_err(format!("invalid issue index {issue_index}")))?;
        Ok(suggest_fixes(issue, &self.inner)
            .into_iter()
            .map(PyQuickFixCandidate::from)
            .collect())
    }

    #[pyo3(signature = (issue_index, include_review=false))]
    fn autofix_issue(
        &mut self,
        issue_index: usize,
        include_review: bool,
    ) -> PyResult<Option<String>> {
        let issues = validate_authoring_graph(&self.inner);
        let issue = issues
            .get(issue_index)
            .ok_or_else(|| PyValueError::new_err(format!("invalid issue index {issue_index}")))?;
        let candidate = select_fix_candidate(issue, &self.inner, include_review)
            .ok_or_else(|| PyValueError::new_err("no fix candidate available for issue"))?;
        let changed =
            apply_fix(&mut self.inner, issue, candidate.fix_id).map_err(PyValueError::new_err)?;
        if changed {
            Ok(Some(candidate.fix_id.to_string()))
        } else {
            Ok(None)
        }
    }

    fn autofix_safe(&mut self) -> PyResult<usize> {
        apply_autofix_pass(&mut self.inner, false).map_err(PyValueError::new_err)
    }

    fn autofix_full(&mut self) -> PyResult<usize> {
        apply_autofix_pass(&mut self.inner, true).map_err(PyValueError::new_err)
    }

    fn set_bookmark(&mut self, name: String, node_id: u32) -> bool {
        self.inner.set_bookmark(name, node_id)
    }

    fn remove_bookmark(&mut self, name: &str) -> bool {
        self.inner.remove_bookmark(name)
    }

    fn bookmark_target(&self, name: &str) -> Option<u32> {
        self.inner.bookmarked_node(name)
    }

    fn list_bookmarks(&self) -> Vec<(String, u32)> {
        self.inner
            .bookmarks()
            .map(|(name, node_id)| (name.clone(), *node_id))
            .collect()
    }

    fn save(&self, path: &str) -> PyResult<()> {
        let json = self
            .to_authoring_document()
            .to_json()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        std::fs::write(path, json).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[staticmethod]
    fn load(path: &str) -> PyResult<Self> {
        let source =
            std::fs::read_to_string(path).map_err(|e| PyValueError::new_err(e.to_string()))?;
        if let Ok(document) = AuthoringDocument::from_json(&source) {
            return Ok(Self {
                inner: document.graph,
                layer_overrides: document.composer_layer_overrides,
                operation_log: document.operation_log,
                verification_runs: document.verification_runs,
            });
        }
        let inner = load_authoring_document_or_script(path)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(Self {
            inner,
            layer_overrides: BTreeMap::new(),
            operation_log: Vec::new(),
            verification_runs: Vec::new(),
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "NodeGraph(nodes={}, connections={})",
            self.inner.len(),
            self.inner.connection_count()
        )
    }
}
