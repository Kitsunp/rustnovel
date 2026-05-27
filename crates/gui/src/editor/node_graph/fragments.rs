use super::*;

impl NodeGraph {
    pub fn create_fragment_from_selection(&mut self, fragment_id: &str, title: &str) -> bool {
        let node_ids = self.selected_node_ids();
        let changed = self
            .apply_authoring_command(AuthoringCommand::CreateFragment {
                fragment_id: fragment_id.to_string(),
                title: title.to_string(),
                node_ids,
            })
            .is_some();
        if changed {
            self.queue_operation_hint(
                "fragment_created",
                format!("Created fragment {fragment_id}"),
                Some(format!("graph.fragments[{fragment_id}]")),
                true,
            );
        }
        changed
    }

    pub fn remove_fragment(&mut self, fragment_id: &str) -> bool {
        let changed = self
            .apply_authoring_command(AuthoringCommand::RemoveFragment {
                fragment_id: fragment_id.to_string(),
            })
            .is_some();
        if changed {
            self.queue_operation_hint(
                "fragment_removed",
                format!("Removed fragment {fragment_id}"),
                Some(format!("graph.fragments[{fragment_id}]")),
                true,
            );
        }
        changed
    }

    pub fn refresh_fragment_ports(&mut self, fragment_id: &str) -> bool {
        let changed = self
            .apply_authoring_command(AuthoringCommand::RefreshFragmentPorts {
                fragment_id: fragment_id.to_string(),
            })
            .is_some();
        if changed {
            self.queue_operation_hint(
                "field_edited",
                format!("Refreshed ports for fragment {fragment_id}"),
                Some(format!("graph.fragments[{fragment_id}].ports")),
                true,
            );
        }
        changed
    }

    pub fn enter_fragment(&mut self, fragment_id: &str) -> bool {
        let changed = self
            .apply_authoring_command(AuthoringCommand::EnterFragment {
                fragment_id: fragment_id.to_string(),
            })
            .is_some();
        if changed {
            self.queue_operation_hint(
                "fragment_entered",
                format!("Entered fragment {fragment_id}"),
                Some(format!("graph.fragments[{fragment_id}]")),
                false,
            );
            self.mark_modified();
        }
        changed
    }

    pub fn leave_fragment(&mut self) -> bool {
        let changed = self
            .apply_authoring_command(AuthoringCommand::LeaveFragment)
            .is_some();
        if changed {
            self.queue_operation_hint(
                "fragment_left",
                "Left active fragment",
                Some("graph.active_fragment".to_string()),
                false,
            );
            self.mark_modified();
        }
        changed
    }

    pub fn fragments(&self) -> Vec<visual_novel_engine::authoring::GraphFragment> {
        self.authoring.list_fragments()
    }

    pub fn active_fragment(&self) -> Option<&str> {
        self.authoring.active_fragment()
    }

    pub fn visible_nodes(&self) -> impl Iterator<Item = (u32, StoryNode, egui::Pos2)> + '_ {
        let visible_ids = self
            .authoring
            .visible_node_ids()
            .into_iter()
            .collect::<BTreeSet<_>>();
        self.nodes()
            .filter(move |(id, _, _)| visible_ids.contains(id))
    }

    pub fn visible_connections(&self) -> impl Iterator<Item = GraphConnection> + '_ {
        self.authoring.visible_connections().into_iter()
    }

    pub fn fragment_validation_issues(&self) -> Vec<visual_novel_engine::authoring::LintIssue> {
        self.authoring.validate_fragments()
    }
}
