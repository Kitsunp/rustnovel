use super::{AuthoringPosition, NodeGraph, StoryNode};

impl NodeGraph {
    pub fn connect_or_branch(
        &mut self,
        from: u32,
        from_port: usize,
        to: u32,
        branch_pos: AuthoringPosition,
    ) -> bool {
        let before = self.connection_count();
        let Some(from_node) = self.get_node(from).cloned() else {
            return false;
        };
        let Some(to_node) = self.get_node(to).cloned() else {
            return false;
        };
        if !from_node.can_connect_from() || !to_node.can_connect_to() {
            return false;
        }
        if let StoryNode::Choice { options, .. } = &from_node {
            if from_port >= options.len() {
                return self
                    .connect_new_choice_option(from, to, "New route")
                    .is_some();
            }
            let already_connected = self
                .connections()
                .any(|conn| conn.from == from && conn.from_port == from_port && conn.to == to);
            if already_connected {
                return false;
            }
            self.connect_port(from, from_port, to);
            return self.connection_count() != before
                || self
                    .connections()
                    .any(|conn| conn.from == from && conn.from_port == from_port && conn.to == to);
        }

        if from_port != 0 || matches!(from_node, StoryNode::JumpIf { .. }) {
            let already_connected = self
                .connections()
                .any(|conn| conn.from == from && conn.from_port == from_port && conn.to == to);
            if already_connected {
                return false;
            }
            self.connect_port(from, from_port, to);
            return self.connection_count() != before
                || self
                    .connections()
                    .any(|conn| conn.from == from && conn.from_port == from_port && conn.to == to);
        }

        let existing_target = self
            .connections()
            .find(|conn| conn.from == from && conn.from_port == 0)
            .map(|conn| conn.to);
        let Some(existing_target) = existing_target else {
            self.connect_port(from, 0, to);
            return self.connection_count() != before
                || self
                    .connections()
                    .any(|conn| conn.from == from && conn.from_port == 0 && conn.to == to);
        };
        if existing_target == to {
            return false;
        }

        if matches!(to_node, StoryNode::Choice { .. }) {
            self.connect_port(from, 0, to);
            let already_routes_to_previous = self
                .connections()
                .any(|conn| conn.from == to && conn.to == existing_target);
            if !already_routes_to_previous {
                self.connect_new_choice_option(to, existing_target, "Continue");
            }
            return true;
        }

        if matches!(
            self.get_node(existing_target),
            Some(StoryNode::Choice { .. })
        ) {
            return self
                .connect_new_choice_option(existing_target, to, "New route")
                .is_some();
        }

        let choice_id = self.add_node(
            StoryNode::Choice {
                prompt: "Choose route:".to_string(),
                options: vec!["Continue".to_string(), "New route".to_string()],
            },
            branch_pos,
        );
        self.connect_port(from, 0, choice_id);
        self.connect_port(choice_id, 0, existing_target);
        self.connect_port(choice_id, 1, to);
        true
    }

    pub fn add_choice_option(&mut self, node_id: u32, text: impl Into<String>) -> Option<usize> {
        let Some(StoryNode::Choice { options, .. }) = self.get_node_mut(node_id) else {
            return None;
        };
        let option_idx = options.len();
        let text = text.into();
        options.push(if text.trim().is_empty() {
            format!("Option {}", option_idx + 1)
        } else {
            text
        });
        self.modified = true;
        Some(option_idx)
    }

    pub fn connect_new_choice_option(
        &mut self,
        choice_id: u32,
        to: u32,
        text: impl Into<String>,
    ) -> Option<usize> {
        if !matches!(self.get_node(choice_id), Some(StoryNode::Choice { .. }))
            || !self.get_node(to).is_some_and(StoryNode::can_connect_to)
        {
            return None;
        }
        let option_idx = self.add_choice_option(choice_id, text)?;
        self.connect_port(choice_id, option_idx, to);
        Some(option_idx)
    }

    pub(super) fn ensure_choice_option(&mut self, node_id: u32, option_idx: usize) {
        let Some(StoryNode::Choice { options, .. }) = self.get_node_mut(node_id) else {
            return;
        };
        let mut changed = false;
        while options.len() <= option_idx {
            let next = options.len() + 1;
            options.push(format!("Option {next}"));
            changed = true;
        }
        if changed {
            self.modified = true;
        }
    }
}
