use std::collections::HashMap;

use super::*;
use crate::editor::StoryNode;

impl EditorWorkbench {
    pub(super) fn refresh_scene_from_selected_node(&mut self) {
        self.scene.clear();
        self.composer_entity_owners.clear();
        let Some(node_id) = self.selected_node.or(self.node_graph.selected) else {
            self.selected_entity = None;
            return;
        };
        let Some(node) = self.node_graph.get_node(node_id) else {
            self.selected_entity = None;
            return;
        };
        let mut owners = HashMap::new();
        match node {
            StoryNode::Scene {
                background,
                music,
                characters,
                ..
            } => {
                spawn_background_entity(
                    &mut self.scene,
                    &mut owners,
                    background.as_deref(),
                    node_id,
                );
                spawn_audio_entity(&mut self.scene, &mut owners, music.as_deref(), node_id);
                for (index, character) in characters.iter().enumerate() {
                    spawn_character_entity(&mut self.scene, &mut owners, character, index, node_id);
                }
            }
            StoryNode::ScenePatch(patch) => {
                spawn_background_entity(
                    &mut self.scene,
                    &mut owners,
                    patch.background.as_deref(),
                    node_id,
                );
                spawn_audio_entity(
                    &mut self.scene,
                    &mut owners,
                    patch.music.as_deref(),
                    node_id,
                );
                for (index, character) in patch.add.iter().enumerate() {
                    spawn_character_entity(&mut self.scene, &mut owners, character, index, node_id);
                }
                for (index, character) in patch.update.iter().enumerate() {
                    spawn_character_patch_entity(
                        &mut self.scene,
                        &mut owners,
                        character,
                        index,
                        node_id,
                    );
                }
            }
            StoryNode::AudioAction {
                asset: Some(asset), ..
            } => {
                spawn_audio_entity(&mut self.scene, &mut owners, Some(asset), node_id);
            }
            StoryNode::CharacterPlacement { name, x, y, scale } => {
                let placement = visual_novel_engine::CharacterPlacementRaw {
                    name: name.clone(),
                    expression: None,
                    position: None,
                    x: Some(*x),
                    y: Some(*y),
                    scale: *scale,
                };
                spawn_character_entity(&mut self.scene, &mut owners, &placement, 0, node_id);
            }
            _ => {}
        }
        self.composer_entity_owners = owners;
        if self.scene.is_empty() {
            self.selected_entity = None;
        }
    }
}

fn spawn_background_entity(
    scene: &mut visual_novel_engine::SceneState,
    owners: &mut HashMap<u32, u32>,
    background: Option<&str>,
    node_id: u32,
) {
    let Some(background) = background else {
        return;
    };
    let mut transform = visual_novel_engine::Transform::at(0, 0);
    transform.z_order = -100;
    if let Some(entity_id) = scene.spawn_with_transform(
        transform,
        visual_novel_engine::EntityKind::Image(visual_novel_engine::ImageData {
            path: visual_novel_engine::SharedStr::from(background),
            tint: None,
        }),
    ) {
        owners.insert(entity_id.raw(), node_id);
    }
}

fn spawn_audio_entity(
    scene: &mut visual_novel_engine::SceneState,
    owners: &mut HashMap<u32, u32>,
    asset: Option<&str>,
    node_id: u32,
) {
    let Some(asset) = asset else {
        return;
    };
    let mut transform = visual_novel_engine::Transform::at(12, 12);
    transform.z_order = 500;
    if let Some(entity_id) = scene.spawn_with_transform(
        transform,
        visual_novel_engine::EntityKind::Audio(visual_novel_engine::AudioData {
            path: visual_novel_engine::SharedStr::from(asset),
            volume: 1000,
            looping: true,
        }),
    ) {
        owners.insert(entity_id.raw(), node_id);
    }
}

fn spawn_character_entity(
    scene: &mut visual_novel_engine::SceneState,
    owners: &mut HashMap<u32, u32>,
    character: &visual_novel_engine::CharacterPlacementRaw,
    index: usize,
    node_id: u32,
) {
    let default_x = 220 + (index as i32) * 180;
    let default_y = 260;
    let mut transform = visual_novel_engine::Transform::at(
        character.x.unwrap_or(default_x),
        character.y.unwrap_or(default_y),
    );
    transform.z_order = index as i32;
    transform.scale = (character.scale.unwrap_or(1.0).clamp(0.1, 4.0) * 1000.0) as u32;
    if let Some(entity_id) = scene.spawn_with_transform(
        transform,
        visual_novel_engine::EntityKind::Character(visual_novel_engine::CharacterData {
            name: visual_novel_engine::SharedStr::from(character.name.as_str()),
            expression: character
                .expression
                .as_deref()
                .map(visual_novel_engine::SharedStr::from),
        }),
    ) {
        owners.insert(entity_id.raw(), node_id);
    }
}

fn spawn_character_patch_entity(
    scene: &mut visual_novel_engine::SceneState,
    owners: &mut HashMap<u32, u32>,
    character: &visual_novel_engine::CharacterPatchRaw,
    index: usize,
    node_id: u32,
) {
    let default_x = 220 + (index as i32) * 180;
    let default_y = 260;
    let mut transform = visual_novel_engine::Transform::at(default_x, default_y);
    transform.z_order = index as i32;
    if let Some(entity_id) = scene.spawn_with_transform(
        transform,
        visual_novel_engine::EntityKind::Character(visual_novel_engine::CharacterData {
            name: visual_novel_engine::SharedStr::from(character.name.as_str()),
            expression: character
                .expression
                .as_deref()
                .map(visual_novel_engine::SharedStr::from),
        }),
    ) {
        owners.insert(entity_id.raw(), node_id);
    }
}
