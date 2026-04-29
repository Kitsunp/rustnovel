use super::*;

impl NodeGraph {
    /// Connects two nodes.
    pub fn connect(&mut self, from: u32, to: u32) {
        self.connect_port(from, 0, to)
    }

    /// Connects a specific output port to a target node.
    pub fn connect_port(&mut self, from: u32, from_port: usize, to: u32) {
        self.authoring.connect_port(from, from_port, to);
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
        self.authoring.disconnect_port(from, from_port);
    }

    /// Returns the number of connections.
    #[inline]
    pub fn connection_count(&self) -> usize {
        self.authoring.connection_count()
    }

    /// Removes a specific option from a Choice node and updates connections.
    pub fn remove_choice_option(&mut self, node_id: u32, option_idx: usize) {
        self.authoring.remove_choice_option(node_id, option_idx);
    }
}
