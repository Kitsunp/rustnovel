use super::*;

impl NodeGraph {
    /// Inserts a new node before the target node, re-routing connections.
    pub fn insert_before(&mut self, target_id: u32, node: StoryNode) {
        let Some((_, _, pos)) = self.nodes.iter().find(|(id, _, _)| *id == target_id) else {
            debug_assert!(
                false,
                "Precondition warning: target_id {} not found in insert_before",
                target_id
            );
            return;
        };

        let new_pos = egui::pos2(pos.x, pos.y - NODE_VERTICAL_SPACING);
        let new_id = self.add_node(node, new_pos);

        for conn in &mut self.connections {
            if conn.to == target_id {
                conn.to = new_id;
            }
        }

        self.connections.push(GraphConnection {
            from: new_id,
            from_port: 0,
            to: target_id,
        });

        self.modified = true;
    }

    /// Inserts a new node after the target node, re-routing connections.
    pub fn insert_after(&mut self, target_id: u32, node: StoryNode) {
        let Some((_, _, pos)) = self.nodes.iter().find(|(id, _, _)| *id == target_id) else {
            return;
        };

        let new_pos = egui::pos2(pos.x, pos.y + NODE_VERTICAL_SPACING);
        let new_id = self.add_node(node, new_pos);

        for conn in &mut self.connections {
            if conn.from == target_id && conn.from_port == 0 {
                conn.from = new_id;
                conn.from_port = 0;
            }
        }

        self.connections.push(GraphConnection {
            from: target_id,
            from_port: 0,
            to: new_id,
        });

        self.modified = true;
    }

    /// Converts a node to a Choice node with default options.
    pub fn convert_to_choice(&mut self, node_id: u32) {
        if let Some((_, node, _)) = self.nodes.iter_mut().find(|(id, _, _)| *id == node_id) {
            *node = StoryNode::Choice {
                prompt: "Choose an option:".to_string(),
                options: vec!["Option 1".to_string(), "Option 2".to_string()],
            };
            self.modified = true;
        }
    }

    /// Creates a branch from a node (adds a Choice with two paths).
    pub fn create_branch(&mut self, node_id: u32) {
        let Some((_, node, pos)) = self.nodes.iter().find(|(id, _, _)| *id == node_id).cloned()
        else {
            return;
        };

        if matches!(node, StoryNode::End) {
            return;
        }

        let choice_pos = egui::pos2(pos.x, pos.y + 120.0);
        let choice_id = self.add_node(
            StoryNode::Choice {
                prompt: "Which path?".to_string(),
                options: vec!["Path A".to_string(), "Path B".to_string()],
            },
            choice_pos,
        );

        let branch_a = self.add_node(
            StoryNode::Dialogue {
                speaker: "Path A".to_string(),
                text: "Content for path A...".to_string(),
            },
            egui::pos2(choice_pos.x - 120.0, choice_pos.y + 140.0),
        );

        let branch_b = self.add_node(
            StoryNode::Dialogue {
                speaker: "Path B".to_string(),
                text: "Content for path B...".to_string(),
            },
            egui::pos2(choice_pos.x + 120.0, choice_pos.y + 140.0),
        );

        self.connect_port(node_id, 0, choice_id);
        self.connect_port(choice_id, 0, branch_a);
        self.connect_port(choice_id, 1, branch_b);
    }

    /// Saves the current Scene node fields into a reusable profile.
    pub fn save_scene_profile(&mut self, profile_id: impl Into<String>, node_id: u32) -> bool {
        let profile_id = profile_id.into().trim().to_string();
        if profile_id.is_empty() {
            return false;
        }

        let Some(StoryNode::Scene {
            background,
            music,
            characters,
            ..
        }) = self.get_node(node_id)
        else {
            return false;
        };

        self.scene_profiles.insert(
            profile_id.clone(),
            SceneProfile {
                background: background.clone(),
                music: music.clone(),
                characters: characters.clone(),
                layers: scene_layers_from_parts(background, characters),
                poses: pose_bindings_from_characters(characters),
            },
        );

        if let Some(StoryNode::Scene { profile, .. }) = self.get_node_mut(node_id) {
            *profile = Some(profile_id);
        }
        self.modified = true;
        true
    }

    /// Applies a saved Scene profile to an existing Scene node.
    pub fn apply_scene_profile(&mut self, profile_id: &str, node_id: u32) -> bool {
        let Some(scene_profile) = self.scene_profiles.get(profile_id).cloned() else {
            return false;
        };

        let Some(StoryNode::Scene {
            background,
            music,
            characters,
            profile,
        }) = self.get_node_mut(node_id)
        else {
            return false;
        };

        let (profile_background, profile_characters) = flatten_scene_profile(&scene_profile);
        *background = profile_background;
        *music = scene_profile.music;
        *characters = profile_characters;
        *profile = Some(profile_id.to_string());
        self.modified = true;
        true
    }

    /// Unlinks a Scene node from a reusable profile while preserving its current fields.
    pub fn detach_scene_profile(&mut self, node_id: u32) -> bool {
        let Some(StoryNode::Scene { profile, .. }) = self.get_node_mut(node_id) else {
            return false;
        };
        if profile.take().is_some() {
            self.modified = true;
            return true;
        }
        false
    }

    /// Changes a character expression using the pose catalog saved in a Scene profile.
    pub fn set_scene_character_pose(
        &mut self,
        node_id: u32,
        character_name: &str,
        pose: &str,
    ) -> bool {
        let profile_id = match self.get_node(node_id) {
            Some(StoryNode::Scene {
                profile: Some(profile),
                ..
            }) => profile.clone(),
            _ => return false,
        };
        let Some(binding) = self
            .scene_profiles
            .get(&profile_id)
            .and_then(|profile| {
                profile
                    .poses
                    .iter()
                    .find(|binding| binding.character == character_name && binding.pose == pose)
            })
            .cloned()
        else {
            return false;
        };

        let Some(StoryNode::Scene { characters, .. }) = self.get_node_mut(node_id) else {
            return false;
        };
        if let Some(character) = characters
            .iter_mut()
            .find(|character| character.name == character_name)
        {
            if character.expression.as_deref() == Some(binding.image.as_str()) {
                return false;
            }
            character.expression = Some(binding.image);
            self.modified = true;
            return true;
        }
        characters.push(CharacterPlacementRaw {
            name: character_name.to_string(),
            expression: Some(binding.image),
            ..Default::default()
        });
        self.modified = true;
        true
    }

    /// Returns available scene profile names.
    pub fn scene_profile_names(&self) -> Vec<String> {
        self.scene_profiles.keys().cloned().collect()
    }

    /// Returns an immutable profile snapshot by id.
    pub fn scene_profile(&self, profile_id: &str) -> Option<&SceneProfile> {
        self.scene_profiles.get(profile_id)
    }

    /// Creates or updates a bookmark that points to an existing node.
    pub fn set_bookmark(&mut self, name: impl Into<String>, node_id: u32) -> bool {
        if self.get_node(node_id).is_none() {
            return false;
        }
        let normalized = name.into().trim().to_string();
        if normalized.is_empty() {
            return false;
        }
        self.bookmarks.insert(normalized, node_id);
        self.modified = true;
        true
    }

    /// Removes a bookmark by name.
    pub fn remove_bookmark(&mut self, name: &str) -> bool {
        if self.bookmarks.remove(name).is_some() {
            self.modified = true;
            true
        } else {
            false
        }
    }

    /// Resolves a bookmark name into its node id.
    pub fn bookmarked_node(&self, name: &str) -> Option<u32> {
        self.bookmarks.get(name).copied()
    }

    /// Returns bookmark names and targets in deterministic order.
    pub fn bookmarks(&self) -> impl Iterator<Item = (&String, &u32)> {
        self.bookmarks.iter()
    }
}

fn scene_layers_from_parts(
    background: &Option<String>,
    characters: &[CharacterPlacementRaw],
) -> Vec<SceneLayer> {
    let mut layers = Vec::new();
    if let Some(background) = background {
        layers.push(SceneLayer {
            name: "background".to_string(),
            visible: true,
            background: Some(background.clone()),
            characters: Vec::new(),
        });
    }
    for character in characters {
        layers.push(SceneLayer {
            name: format!("character:{}", character.name),
            visible: true,
            background: None,
            characters: vec![character.clone()],
        });
    }
    layers
}

fn pose_bindings_from_characters(
    characters: &[CharacterPlacementRaw],
) -> Vec<CharacterPoseBinding> {
    characters
        .iter()
        .filter_map(|character| {
            let image = character.expression.as_ref()?;
            Some(CharacterPoseBinding {
                character: character.name.clone(),
                pose: pose_name_from_path(image),
                image: image.clone(),
            })
        })
        .collect()
}

fn pose_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(path)
        .to_string()
}

fn flatten_scene_profile(profile: &SceneProfile) -> (Option<String>, Vec<CharacterPlacementRaw>) {
    if profile.layers.is_empty() {
        return (profile.background.clone(), profile.characters.clone());
    }
    let mut background = None;
    let mut characters = Vec::new();
    for layer in profile.layers.iter().filter(|layer| layer.visible) {
        if let Some(layer_background) = &layer.background {
            background = Some(layer_background.clone());
        }
        characters.extend(layer.characters.clone());
    }
    if characters.is_empty() {
        characters = profile.characters.clone();
    }
    (
        background.or_else(|| profile.background.clone()),
        characters,
    )
}
