use super::*;

impl NodeGraph {
    /// Connects two nodes.
    pub fn connect(&mut self, from: u32, to: u32) {
        self.connect_port(from, 0, to)
    }

    /// Connects a specific output port to a target node.
    pub fn connect_port(&mut self, from: u32, from_port: usize, to: u32) {
        if from == to {
            return;
        }

        let Some(from_node) = self.get_node(from).cloned() else {
            return;
        };
        let Some(to_node) = self.get_node(to) else {
            return;
        };
        if !from_node.can_connect_from() || !to_node.can_connect_to() {
            return;
        }

        if matches!(from_node, StoryNode::Choice { .. }) {
            self.ensure_choice_option(from, from_port);
        } else if from_port != 0 {
            return;
        }

        if !self
            .connections
            .iter()
            .any(|c| c.from == from && c.from_port == from_port && c.to == to)
        {
            self.connections
                .retain(|c| !(c.from == from && c.from_port == from_port));
            self.connections.push(GraphConnection {
                from,
                from_port,
                to,
            });
            self.modified = true;
        }
    }

    /// Disconnects two nodes (any port).
    pub fn disconnect(&mut self, from: u32, to: u32) {
        self.connections.retain(|c| !(c.from == from && c.to == to));
        self.modified = true;
    }

    /// Disconnects all outbound connections from a source node.
    pub fn disconnect_all_from(&mut self, from: u32) {
        let before = self.connections.len();
        self.connections.retain(|c| c.from != from);
        if self.connections.len() != before {
            self.modified = true;
        }
    }

    /// Disconnects all outbound connections from a specific source port.
    pub fn disconnect_port(&mut self, from: u32, from_port: usize) {
        let before = self.connections.len();
        self.connections
            .retain(|c| !(c.from == from && c.from_port == from_port));
        if self.connections.len() != before {
            self.modified = true;
        }
    }

    /// Returns the number of connections.
    #[inline]
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Removes a specific option from a Choice node and updates connections.
    pub fn remove_choice_option(&mut self, node_id: u32, option_idx: usize) {
        if let Some(StoryNode::Choice { options, .. }) = self.get_node_mut(node_id) {
            if option_idx < options.len() {
                options.remove(option_idx);
            }
        }

        self.connections
            .retain(|c| !(c.from == node_id && c.from_port == option_idx));

        for conn in &mut self.connections {
            if conn.from == node_id && conn.from_port > option_idx {
                conn.from_port -= 1;
            }
        }

        self.modified = true;
    }

    pub(crate) fn ensure_choice_option(&mut self, node_id: u32, option_idx: usize) {
        let Some(StoryNode::Choice { options, .. }) = self.get_node_mut(node_id) else {
            return;
        };

        let mut changed = false;
        while options.len() <= option_idx {
            let next = options.len() + 1;
            options.push(format!("Option {}", next));
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }
}
