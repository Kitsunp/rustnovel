use super::*;

impl NodeGraph {
    pub fn insert_before(&mut self, target_id: u32, node: StoryNode) {
        self.edit_graph("Insert node before", |graph| {
            graph.insert_before(target_id, node, NODE_VERTICAL_SPACING)
        });
    }
    pub fn insert_after(&mut self, target_id: u32, node: StoryNode) {
        self.edit_graph("Insert node after", |graph| {
            graph.insert_after(target_id, node, NODE_VERTICAL_SPACING)
        });
    }
    pub fn create_branch(&mut self, node_id: u32) {
        if let Some(choice) = self.edit_graph("Create branch", |graph| graph.create_branch(node_id))
        {
            self.set_single_selection(Some(choice));
        }
    }

    /// Converts a node to a Choice node with default options.
    pub fn convert_to_choice(&mut self, node_id: u32) {
        if self.get_node(node_id).is_none() {
            return;
        }
        let changed = self
            .apply_authoring_command(AuthoringCommand::EditNode {
                node_id,
                replacement: StoryNode::Choice {
                    prompt: "Choose an option:".to_string(),
                    options: vec!["Option 1".to_string(), "Option 2".to_string()],
                },
            })
            .is_some();
        if changed {
            self.queue_operation_hint(
                "field_edited",
                format!("Converted node {node_id} to choice"),
                Some(format!("graph.nodes[{node_id}]")),
                true,
            );
        }
    }

    pub fn save_scene_profile(&mut self, profile_id: impl Into<String>, node_id: u32) -> bool {
        self.edit_graph("Edit graph metadata", |graph| {
            graph.save_scene_profile(profile_id, node_id)
        })
    }

    pub fn apply_scene_profile(&mut self, profile_id: &str, node_id: u32) -> bool {
        self.edit_graph("Edit graph metadata", |graph| {
            graph.apply_scene_profile(profile_id, node_id)
        })
    }

    pub fn detach_scene_profile(&mut self, node_id: u32) -> bool {
        self.edit_graph("Edit graph metadata", |graph| {
            graph.detach_scene_profile(node_id)
        })
    }

    pub fn set_scene_character_pose(
        &mut self,
        node_id: u32,
        character_name: &str,
        pose: &str,
    ) -> bool {
        self.edit_graph("Change character pose", |graph| {
            graph.set_scene_character_pose(node_id, character_name, pose)
        })
    }

    pub fn scene_profile_names(&self) -> Vec<String> {
        self.authoring_graph().scene_profile_names()
    }

    pub fn scene_profile(&self, profile_id: &str) -> Option<&SceneProfile> {
        self.authoring_graph().scene_profile(profile_id)
    }

    pub fn set_bookmark(&mut self, name: impl Into<String>, node_id: u32) -> bool {
        self.edit_graph("Edit graph metadata", |graph| {
            graph.set_bookmark(name, node_id)
        })
    }

    pub fn remove_bookmark(&mut self, name: &str) -> bool {
        self.edit_graph("Edit graph metadata", |graph| graph.remove_bookmark(name))
    }

    pub fn bookmarked_node(&self, name: &str) -> Option<u32> {
        self.authoring_graph().bookmarked_node(name)
    }

    pub fn bookmarks(&self) -> impl Iterator<Item = (&String, &u32)> {
        self.authoring_graph().bookmarks()
    }
}
