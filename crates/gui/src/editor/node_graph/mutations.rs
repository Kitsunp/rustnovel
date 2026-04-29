use super::*;

impl NodeGraph {
    /// Inserts a new node before the target node, re-routing incoming ports.
    pub fn insert_before(&mut self, target_id: u32, node: StoryNode) {
        let Some(pos) = self.get_node_pos(target_id) else {
            debug_assert!(
                false,
                "Precondition warning: target_id {} not found in insert_before",
                target_id
            );
            return;
        };
        let new_id = self.add_node(node, egui::pos2(pos.x, pos.y - NODE_VERTICAL_SPACING));
        let incoming = self
            .connections()
            .filter(|conn| conn.to == target_id)
            .map(|conn| (conn.from, conn.from_port))
            .collect::<Vec<_>>();
        for (from, port) in incoming {
            self.connect_port(from, port, new_id);
        }
        self.connect(new_id, target_id);
    }

    /// Inserts a new node after the target node, re-routing the default output.
    pub fn insert_after(&mut self, target_id: u32, node: StoryNode) {
        let Some(pos) = self.get_node_pos(target_id) else {
            return;
        };
        let new_id = self.add_node(node, egui::pos2(pos.x, pos.y + NODE_VERTICAL_SPACING));
        let old_target = self
            .connections()
            .find(|conn| conn.from == target_id && conn.from_port == 0)
            .map(|conn| conn.to);
        if let Some(old_target) = old_target {
            self.connect(new_id, old_target);
        }
        self.connect_port(target_id, 0, new_id);
    }

    /// Converts a node to a Choice node with default options.
    pub fn convert_to_choice(&mut self, node_id: u32) {
        let Some(node) = self.get_node_mut(node_id) else {
            return;
        };
        *node = StoryNode::Choice {
            prompt: "Choose an option:".to_string(),
            options: vec!["Option 1".to_string(), "Option 2".to_string()],
        };
        self.mark_modified();
    }

    /// Creates a branch from a node (adds a Choice with two paths).
    pub fn create_branch(&mut self, node_id: u32) {
        let Some(node) = self.get_node(node_id).cloned() else {
            return;
        };
        if matches!(node, StoryNode::End) {
            return;
        }
        let Some(pos) = self.get_node_pos(node_id) else {
            return;
        };
        let choice_pos = egui::pos2(pos.x, pos.y + 120.0);
        let choice_id = self.add_node(
            StoryNode::Choice {
                prompt: "Which path?".to_string(),
                options: vec!["Path A".to_string(), "Path B".to_string()],
            },
            choice_pos,
        );
        let branch_a = self.add_path_dialogue("Path A", choice_pos.x - 120.0, choice_pos.y + 140.0);
        let branch_b = self.add_path_dialogue("Path B", choice_pos.x + 120.0, choice_pos.y + 140.0);

        self.connect_port(node_id, 0, choice_id);
        self.connect_port(choice_id, 0, branch_a);
        self.connect_port(choice_id, 1, branch_b);
    }

    pub fn save_scene_profile(&mut self, profile_id: impl Into<String>, node_id: u32) -> bool {
        self.authoring.save_scene_profile(profile_id, node_id)
    }

    pub fn apply_scene_profile(&mut self, profile_id: &str, node_id: u32) -> bool {
        self.authoring.apply_scene_profile(profile_id, node_id)
    }

    pub fn detach_scene_profile(&mut self, node_id: u32) -> bool {
        self.authoring.detach_scene_profile(node_id)
    }

    pub fn set_scene_character_pose(
        &mut self,
        node_id: u32,
        character_name: &str,
        pose: &str,
    ) -> bool {
        self.authoring
            .set_scene_character_pose(node_id, character_name, pose)
    }

    pub fn scene_profile_names(&self) -> Vec<String> {
        self.authoring.scene_profile_names()
    }

    pub fn scene_profile(&self, profile_id: &str) -> Option<&SceneProfile> {
        self.authoring.scene_profile(profile_id)
    }

    pub fn set_bookmark(&mut self, name: impl Into<String>, node_id: u32) -> bool {
        self.authoring.set_bookmark(name, node_id)
    }

    pub fn remove_bookmark(&mut self, name: &str) -> bool {
        self.authoring.remove_bookmark(name)
    }

    pub fn bookmarked_node(&self, name: &str) -> Option<u32> {
        self.authoring.bookmarked_node(name)
    }

    pub fn bookmarks(&self) -> impl Iterator<Item = (&String, &u32)> {
        self.authoring.bookmarks()
    }

    fn add_path_dialogue(&mut self, speaker: &str, x: f32, y: f32) -> u32 {
        let branch_label = speaker.to_ascii_lowercase();
        self.add_node(
            StoryNode::Dialogue {
                speaker: speaker.to_string(),
                text: format!("Content for {branch_label}..."),
            },
            egui::pos2(x, y),
        )
    }
}
