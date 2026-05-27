use super::*;

impl NodeGraph {
    pub fn start_connection_drag(&mut self, from: u32, from_port: usize) {
        self.connecting_from = Some((from, from_port));
        self.connecting_sticky = false;
    }

    pub fn start_connection_pick(&mut self, from: u32, from_port: usize) {
        self.connecting_from = Some((from, from_port));
        self.connecting_sticky = true;
    }

    pub fn cancel_connection(&mut self) {
        self.connecting_from = None;
        self.connecting_sticky = false;
    }

    pub fn finish_connection_to(&mut self, to: u32) -> bool {
        let Some((from, from_port)) = self.connecting_from else {
            return false;
        };
        let changed = self.connect_or_branch(from, from_port, to);
        self.cancel_connection();
        changed
    }

    /// Connects two nodes.
    pub fn connect(&mut self, from: u32, to: u32) {
        self.connect_port(from, 0, to)
    }

    /// Connects a specific output port to a target node.
    pub fn connect_port(&mut self, from: u32, from_port: usize, to: u32) {
        if self
            .apply_authoring_command(AuthoringCommand::Connect {
                from,
                from_port,
                to,
            })
            .is_none()
        {
            return;
        }
        self.queue_operation_hint(
            "node_connected",
            format!("Connected node {from} port {from_port} to node {to}"),
            Some(format!("graph.edges[{from}:{from_port}]")),
            true,
        );
    }

    /// Connects a node, or creates/reuses an explicit Choice hub when a
    /// deterministic single-output node already has a continuation.
    pub fn connect_or_branch(&mut self, from: u32, from_port: usize, to: u32) -> bool {
        let branch_pos = self
            .get_node_pos(from)
            .map(|pos| AuthoringPosition::new(pos.x, pos.y + NODE_VERTICAL_SPACING))
            .unwrap_or_default();
        let changed = self
            .apply_authoring_command(AuthoringCommand::ConnectOrBranch {
                from,
                from_port,
                to,
                branch_position: branch_pos,
            })
            .is_some();
        if changed {
            self.queue_operation_hint(
                "node_connected",
                format!("Connected node {from} port {from_port} to node {to}"),
                Some(format!("graph.edges[{from}:{from_port}]")),
                true,
            );
            let choice_hub = self
                .connections()
                .find(|conn| conn.from == from && conn.from_port == 0)
                .and_then(|conn| {
                    matches!(self.get_node(conn.to), Some(StoryNode::Choice { .. }))
                        .then_some(conn.to)
                });
            if let Some(choice_hub) = choice_hub {
                self.set_single_selection(Some(choice_hub));
            }
        }
        changed
    }

    pub fn connect_new_choice_option(
        &mut self,
        choice_id: u32,
        to: u32,
        text: impl Into<String>,
    ) -> Option<usize> {
        let delta = self.apply_authoring_command(AuthoringCommand::ConnectNewChoiceOption {
            choice_id,
            to,
            text: text.into(),
        })?;
        match delta {
            AuthoringDelta::ChoiceOptionConnected { option_index, .. } => {
                self.queue_operation_hint(
                    "node_connected",
                    format!("Connected choice {choice_id} option {option_index} to node {to}"),
                    Some(format!("graph.nodes[{choice_id}].options[{option_index}]")),
                    true,
                );
                Some(option_index)
            }
            _ => None,
        }
    }

    /// Disconnects two nodes (any port).
    pub fn disconnect(&mut self, from: u32, to: u32) {
        let ports = self
            .connections()
            .filter(|c| c.from == from && c.to == to)
            .map(|c| c.from_port)
            .collect::<Vec<_>>();
        for port in ports {
            self.disconnect_port(from, port);
        }
    }

    /// Disconnects all outbound connections from a source node.
    pub fn disconnect_all_from(&mut self, from: u32) {
        let ports = self
            .connections()
            .filter(|c| c.from == from)
            .map(|c| c.from_port)
            .collect::<Vec<_>>();
        for port in ports {
            self.disconnect_port(from, port);
        }
    }

    /// Disconnects all outbound connections from a specific source port.
    pub fn disconnect_port(&mut self, from: u32, from_port: usize) {
        if self
            .apply_authoring_command(AuthoringCommand::Disconnect { from, from_port })
            .is_none()
        {
            return;
        }
        self.queue_operation_hint(
            "node_disconnected",
            format!("Disconnected node {from} port {from_port}"),
            Some(format!("graph.edges[{from}:{from_port}]")),
            true,
        );
    }

    /// Returns the number of connections.
    #[inline]
    pub fn connection_count(&self) -> usize {
        self.authoring.connection_count()
    }

    /// Removes a specific option from a Choice node and updates connections.
    pub fn remove_choice_option(&mut self, node_id: u32, option_idx: usize) {
        let changed = self
            .apply_authoring_command(AuthoringCommand::RemoveChoiceOption {
                node_id,
                option_index: option_idx,
            })
            .is_some();
        if changed {
            self.queue_operation_hint(
                "field_edited",
                format!("Removed choice option {option_idx} from node {node_id}"),
                Some(format!(
                    "graph.nodes[{node_id}].choice.options[{option_idx}]"
                )),
                true,
            );
        }
    }
}
