use crate::CharacterPlacementRaw;

use super::{CharacterPoseBinding, NodeGraph, SceneLayer, SceneProfile, StoryNode};

impl NodeGraph {
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

    pub fn apply_scene_profile(&mut self, profile_id: &str, node_id: u32) -> bool {
        let Some(scene_profile) = self.scene_profiles.get(profile_id).cloned() else {
            return false;
        };
        let Some(StoryNode::Scene {
            profile,
            background,
            music,
            characters,
        }) = self.get_node_mut(node_id)
        else {
            return false;
        };
        let (next_background, next_characters) = flatten_scene_profile(&scene_profile);
        *profile = Some(profile_id.to_string());
        *background = next_background;
        *music = scene_profile.music;
        *characters = next_characters;
        self.modified = true;
        true
    }

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

    pub fn scene_profile_names(&self) -> Vec<String> {
        self.scene_profiles.keys().cloned().collect()
    }

    pub fn scene_profile(&self, profile_id: &str) -> Option<&SceneProfile> {
        self.scene_profiles.get(profile_id)
    }

    pub fn scene_profiles(&self) -> impl Iterator<Item = (&String, &SceneProfile)> {
        self.scene_profiles.iter()
    }

    pub fn insert_scene_profile(
        &mut self,
        profile_id: impl Into<String>,
        profile: SceneProfile,
    ) -> bool {
        let profile_id = profile_id.into().trim().to_string();
        if profile_id.is_empty() {
            return false;
        }
        self.scene_profiles.insert(profile_id, profile);
        self.modified = true;
        true
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
