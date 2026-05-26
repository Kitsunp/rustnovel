use super::{
    build_authoring_report_fingerprint, composer, validate_authoring_graph_no_io,
    AuthoringPosition, DiagnosticTarget, FieldPath, GraphConnection, NodeGraph, OperationKind,
    OperationLogEntry, OperationStatus, StoryNode, VerificationRun,
};

#[derive(Clone, Debug, PartialEq)]
pub enum AuthoringCommand {
    CreateNode {
        node_id: u32,
        node: StoryNode,
        position: AuthoringPosition,
    },
    Connect {
        from: u32,
        from_port: usize,
        to: u32,
    },
    EditNode {
        node_id: u32,
        replacement: StoryNode,
    },
    ImportAsset {
        path: String,
    },
    MoveLayer {
        object_id: String,
        x: i32,
        y: i32,
        scale: Option<f32>,
    },
    RevertLast,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AuthoringDelta {
    NodeCreated {
        node_id: u32,
        node: StoryNode,
        position: AuthoringPosition,
    },
    Connected {
        connection: GraphConnection,
        replaced: Vec<GraphConnection>,
    },
    NodeEdited {
        node_id: u32,
        before: StoryNode,
        after: StoryNode,
    },
    AssetImported {
        path: String,
    },
    LayerMoved {
        object_id: String,
        before: (Option<i32>, Option<i32>, Option<f32>),
        after: (Option<i32>, Option<i32>, Option<f32>),
    },
    Reverted {
        reverted: Box<AuthoringDelta>,
    },
}

#[derive(Clone, Debug)]
pub struct AuthoringCommandOutcome {
    pub delta: AuthoringDelta,
    pub operation: OperationLogEntry,
    pub verification: VerificationRun,
}

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
            OperationStatus::Applied.label(),
            format!("authoring command applied: {command:?}"),
        )
        .with_before_after_fingerprints(&before_fingerprint, &after_fingerprint);
        if let Some(field_path) = field_path {
            operation = operation.with_field_path(field_path);
        }
        if let Some(target) = target {
            operation = operation.with_target(target);
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

    fn apply_without_logging(
        &mut self,
        command: &AuthoringCommand,
    ) -> Result<
        (
            AuthoringDelta,
            OperationKind,
            Option<String>,
            Option<DiagnosticTarget>,
        ),
        String,
    > {
        match command {
            AuthoringCommand::CreateNode {
                node_id,
                node,
                position,
            } => {
                if !self
                    .graph
                    .add_node_with_id(*node_id, node.clone(), *position)
                {
                    return Err(format!("node {node_id} already exists"));
                }
                Ok((
                    AuthoringDelta::NodeCreated {
                        node_id: *node_id,
                        node: node.clone(),
                        position: *position,
                    },
                    OperationKind::NodeCreated,
                    Some(format!("graph.nodes[{node_id}]")),
                    Some(DiagnosticTarget::Node { node_id: *node_id }),
                ))
            }
            AuthoringCommand::Connect {
                from,
                from_port,
                to,
            } => {
                let replaced = self
                    .graph
                    .connections()
                    .filter(|conn| conn.from == *from && conn.from_port == *from_port)
                    .cloned()
                    .collect::<Vec<_>>();
                self.graph.connect_port(*from, *from_port, *to);
                let connection = GraphConnection {
                    from: *from,
                    from_port: *from_port,
                    to: *to,
                };
                Ok((
                    AuthoringDelta::Connected {
                        connection: connection.clone(),
                        replaced,
                    },
                    OperationKind::NodeConnected,
                    Some(format!("graph.connections[{from}:{from_port}]")),
                    Some(DiagnosticTarget::Edge {
                        from: *from,
                        from_port: *from_port,
                        to: Some(*to),
                    }),
                ))
            }
            AuthoringCommand::EditNode {
                node_id,
                replacement,
            } => {
                let node = self
                    .graph
                    .get_node_mut(*node_id)
                    .ok_or_else(|| format!("node {node_id} not found"))?;
                let before = node.clone();
                *node = replacement.clone();
                Ok((
                    AuthoringDelta::NodeEdited {
                        node_id: *node_id,
                        before,
                        after: replacement.clone(),
                    },
                    OperationKind::FieldEdited,
                    Some(format!("graph.nodes[{node_id}]")),
                    Some(DiagnosticTarget::Node { node_id: *node_id }),
                ))
            }
            AuthoringCommand::ImportAsset { path } => Ok((
                AuthoringDelta::AssetImported { path: path.clone() },
                OperationKind::AssetImported,
                Some("assets".to_string()),
                Some(DiagnosticTarget::AssetRef {
                    node_id: None,
                    field_path: FieldPath::new("assets"),
                    asset_path: path.clone(),
                }),
            )),
            AuthoringCommand::MoveLayer {
                object_id,
                x,
                y,
                scale,
            } => {
                let before = object_pose(&self.graph, object_id)
                    .ok_or_else(|| format!("layer object '{object_id}' not found"))?;
                if !composer::move_scene_object(&mut self.graph, object_id, *x, *y, *scale) {
                    return Err(format!("layer object '{object_id}' could not be moved"));
                }
                let after = object_pose(&self.graph, object_id)
                    .ok_or_else(|| format!("layer object '{object_id}' missing after move"))?;
                Ok((
                    AuthoringDelta::LayerMoved {
                        object_id: object_id.clone(),
                        before,
                        after,
                    },
                    OperationKind::ComposerObjectMoved,
                    Some(format!("composer.objects[{object_id}]")),
                    Some(DiagnosticTarget::Graph),
                ))
            }
            AuthoringCommand::RevertLast => unreachable!("handled by apply"),
        }
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
            OperationStatus::Applied.label(),
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

    fn apply_inverse_delta(&mut self, delta: &AuthoringDelta) -> Result<(), String> {
        match delta {
            AuthoringDelta::NodeCreated { node_id, .. } => {
                self.graph.remove_node(*node_id);
                Ok(())
            }
            AuthoringDelta::Connected {
                connection,
                replaced,
            } => {
                self.graph
                    .disconnect_port(connection.from, connection.from_port);
                for old in replaced {
                    self.graph.connect_port(old.from, old.from_port, old.to);
                }
                Ok(())
            }
            AuthoringDelta::NodeEdited {
                node_id, before, ..
            } => {
                let node = self
                    .graph
                    .get_node_mut(*node_id)
                    .ok_or_else(|| format!("node {node_id} not found"))?;
                *node = before.clone();
                Ok(())
            }
            AuthoringDelta::AssetImported { .. } => Ok(()),
            AuthoringDelta::LayerMoved {
                object_id, before, ..
            } => {
                let (x, y, scale) = *before;
                composer::move_scene_object(
                    &mut self.graph,
                    object_id,
                    x.unwrap_or_default(),
                    y.unwrap_or_default(),
                    scale,
                )
                .then_some(())
                .ok_or_else(|| format!("layer object '{object_id}' could not be reverted"))
            }
            AuthoringDelta::Reverted { reverted } => self.apply_inverse_delta(reverted),
        }
    }
}

fn object_pose(
    graph: &NodeGraph,
    object_id: &str,
) -> Option<(Option<i32>, Option<i32>, Option<f32>)> {
    composer::list_layered_objects(graph, None)
        .into_iter()
        .find(|object| object.object_id == object_id)
        .map(|object| (object.x, object.y, object.scale))
}
