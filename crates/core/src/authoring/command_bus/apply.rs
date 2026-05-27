use super::super::{
    authoring_graph_sha256, composer, quick_fix, DiagnosticTarget, FieldPath, GraphConnection,
    NodeGraph, OperationKind,
};
use super::{AuthoringCommand, AuthoringCommandBus, AuthoringDelta, CommandApplyResult};

impl AuthoringCommandBus {
    pub(super) fn apply_without_logging(
        &mut self,
        command: &AuthoringCommand,
    ) -> CommandApplyResult {
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
            AuthoringCommand::RemoveNode { node_id } => {
                let node = self
                    .graph
                    .get_node(*node_id)
                    .cloned()
                    .ok_or_else(|| format!("node {node_id} not found"))?;
                let position = self
                    .graph
                    .get_node_pos(*node_id)
                    .ok_or_else(|| format!("node {node_id} position not found"))?;
                let connections = self
                    .graph
                    .connections()
                    .filter(|connection| connection.from == *node_id || connection.to == *node_id)
                    .cloned()
                    .collect::<Vec<_>>();
                self.graph.remove_node(*node_id);
                Ok((
                    AuthoringDelta::NodeRemoved {
                        node_id: *node_id,
                        node,
                        position,
                        connections,
                    },
                    OperationKind::NodeRemoved,
                    Some(format!("graph.nodes[{node_id}]")),
                    Some(DiagnosticTarget::Node { node_id: *node_id }),
                ))
            }
            AuthoringCommand::Connect {
                from,
                from_port,
                to,
            } => self.connect(*from, *from_port, *to),
            AuthoringCommand::Disconnect { from, from_port } => {
                let removed = self
                    .graph
                    .connections()
                    .filter(|conn| conn.from == *from && conn.from_port == *from_port)
                    .cloned()
                    .collect::<Vec<_>>();
                if removed.is_empty() {
                    return Err(format!("node {from} port {from_port} is not connected"));
                }
                self.graph.disconnect_port(*from, *from_port);
                Ok((
                    AuthoringDelta::Disconnected { removed },
                    OperationKind::NodeDisconnected,
                    Some(format!("graph.edges[{from}:{from_port}]")),
                    Some(DiagnosticTarget::Edge {
                        from: *from,
                        from_port: *from_port,
                        to: None,
                    }),
                ))
            }
            AuthoringCommand::ConnectNewChoiceOption {
                choice_id,
                to,
                text,
            } => {
                let before_node = self
                    .graph
                    .get_node(*choice_id)
                    .cloned()
                    .ok_or_else(|| format!("choice node {choice_id} not found"))?;
                let option_index = self
                    .graph
                    .connect_new_choice_option(*choice_id, *to, text.clone())
                    .ok_or_else(|| {
                        format!("choice node {choice_id} could not connect a new option to {to}")
                    })?;
                let connection = GraphConnection {
                    from: *choice_id,
                    from_port: option_index,
                    to: *to,
                };
                Ok((
                    AuthoringDelta::ChoiceOptionConnected {
                        choice_id: *choice_id,
                        option_index,
                        before_node,
                        connection: connection.clone(),
                    },
                    OperationKind::NodeConnected,
                    Some(format!("graph.nodes[{choice_id}].options[{option_index}]")),
                    Some(DiagnosticTarget::Edge {
                        from: *choice_id,
                        from_port: option_index,
                        to: Some(*to),
                    }),
                ))
            }
            AuthoringCommand::SetChoiceOptionTarget {
                node_id,
                option_index,
                target_node_id,
            } => self.set_choice_option_target(*node_id, *option_index, *target_node_id),
            AuthoringCommand::ConnectOrBranch {
                from,
                from_port,
                to,
                branch_position,
            } => {
                if !self
                    .graph
                    .connect_or_branch(*from, *from_port, *to, *branch_position)
                {
                    return Err(format!(
                        "node {from} port {from_port} could not branch to node {to}"
                    ));
                }
                Ok((
                    AuthoringDelta::BranchConnected {
                        from: *from,
                        from_port: *from_port,
                        to: *to,
                    },
                    OperationKind::NodeConnected,
                    Some(format!("graph.edges[{from}:{from_port}]")),
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
            AuthoringCommand::EditDialogue {
                node_id,
                speaker,
                text,
            } => self.edit_dialogue(*node_id, speaker, text),
            AuthoringCommand::EditChoicePrompt { node_id, prompt } => {
                self.edit_choice_prompt(*node_id, prompt)
            }
            AuthoringCommand::EditChoiceOptionText {
                node_id,
                option_index,
                text,
            } => self.edit_choice_option_text(*node_id, *option_index, text),
            AuthoringCommand::ReorderChoiceOption {
                node_id,
                from_index,
                to_index,
            } => self.reorder_choice_option(*node_id, *from_index, *to_index),
            AuthoringCommand::RemoveChoiceOption {
                node_id,
                option_index,
            } => self.remove_choice_option(*node_id, *option_index),
            AuthoringCommand::CreateFragment {
                fragment_id,
                title,
                node_ids,
            } => self.create_fragment(fragment_id, title, node_ids),
            AuthoringCommand::RemoveFragment { fragment_id } => self.remove_fragment(fragment_id),
            AuthoringCommand::EnterFragment { fragment_id } => self.enter_fragment(fragment_id),
            AuthoringCommand::LeaveFragment => self.leave_fragment(),
            AuthoringCommand::RefreshFragmentPorts { fragment_id } => {
                self.refresh_fragment_ports(fragment_id)
            }
            AuthoringCommand::ApplyQuickFix { issue, fix_id } => {
                let before_sha256 = authoring_graph_sha256(&self.graph);
                let changed = quick_fix::apply_fix(&mut self.graph, issue, fix_id)?;
                if !changed {
                    return Err(format!("quick-fix '{fix_id}' made no changes"));
                }
                let after_sha256 = authoring_graph_sha256(&self.graph);
                Ok((
                    AuthoringDelta::QuickFixApplied {
                        diagnostic_id: issue.diagnostic_id(),
                        fix_id: fix_id.clone(),
                        before_sha256,
                        after_sha256,
                    },
                    OperationKind::QuickFixApplied,
                    issue
                        .field_path
                        .as_ref()
                        .map(|path| path.value.clone())
                        .or_else(|| Some("graph".to_string())),
                    issue.target.clone(),
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
