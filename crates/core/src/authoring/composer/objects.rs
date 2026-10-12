use std::collections::BTreeMap;

use crate::event::{CharacterPlacementRaw, ScenePatchRaw};

use super::super::{NodeGraph, StoryNode};
use super::{compose_scene_snapshot, LayerOverride, LayeredSceneObject, StageLayerKind};

pub fn list_layered_objects(
    graph: &NodeGraph,
    selected_node_id: Option<u32>,
) -> Vec<LayeredSceneObject> {
    compose_scene_snapshot(graph, selected_node_id, None, None, None, None).objects
}

pub fn set_layer_visible(
    overrides: &mut BTreeMap<String, LayerOverride>,
    object_id: &str,
    visible: bool,
) {
    overrides.entry(object_id.to_string()).or_default().visible = visible;
}

pub fn set_layer_locked(
    overrides: &mut BTreeMap<String, LayerOverride>,
    object_id: &str,
    locked: bool,
) {
    overrides.entry(object_id.to_string()).or_default().locked = locked;
}

pub fn apply_layer_overrides(
    objects: &mut [LayeredSceneObject],
    overrides: &BTreeMap<String, LayerOverride>,
) {
    for object in objects {
        if let Some(override_state) = overrides.get(&object.object_id) {
            object.visible = override_state.visible;
            object.locked = override_state.locked;
        }
    }
}

pub fn move_scene_object(
    graph: &mut NodeGraph,
    object_id: &str,
    x: i32,
    y: i32,
    scale: Option<f32>,
) -> bool {
    set_scene_object_pose(graph, object_id, Some(x), Some(y), scale)
}

pub fn set_scene_object_pose(
    graph: &mut NodeGraph,
    object_id: &str,
    x: Option<i32>,
    y: Option<i32>,
    scale: Option<f32>,
) -> bool {
    let Some((node_id, index, update)) = parse_character_object_id(object_id) else {
        return false;
    };
    let Some(node) = graph.get_node_mut(node_id) else {
        return false;
    };
    match node {
        StoryNode::Scene { characters, .. } => set_character_pose(characters, index, x, y, scale),
        StoryNode::ScenePatch(ScenePatchRaw {
            update: characters, ..
        }) if update => {
            let Some(character) = characters.get_mut(index) else {
                return false;
            };
            character.x = x;
            character.y = y;
            character.scale = scale;
            true
        }
        StoryNode::ScenePatch(ScenePatchRaw { add, .. }) => {
            set_character_pose(add, index, x, y, scale)
        }
        _ => false,
    }
}

pub(super) fn collect_authoring_objects(
    graph: &NodeGraph,
    selected_node_id: Option<u32>,
    objects: &mut Vec<LayeredSceneObject>,
) {
    for (node_id, node, _) in graph.nodes() {
        if selected_node_id.is_some() && selected_node_id != Some(*node_id) {
            continue;
        }
        collect_node_objects(*node_id, node, objects);
    }
}

fn collect_node_objects(node_id: u32, node: &StoryNode, objects: &mut Vec<LayeredSceneObject>) {
    match node {
        StoryNode::Scene {
            background,
            characters,
            ..
        } => {
            if let Some(background) = background {
                objects.push(background_object(node_id, background));
            }
            collect_characters(node_id, "characters", characters, objects);
        }
        StoryNode::ScenePatch(patch) => {
            if let Some(background) = &patch.background {
                objects.push(background_object(node_id, background));
            }
            collect_characters(node_id, "patch.add", &patch.add, objects);
            let updates = patch
                .update
                .iter()
                .map(|character| CharacterPlacementRaw {
                    name: character.name.clone(),
                    expression: character.expression.clone(),
                    position: character.position.clone(),
                    x: character.x,
                    y: character.y,
                    scale: character.scale,
                })
                .collect::<Vec<_>>();
            collect_characters(node_id, "patch.update", &updates, objects);
        }
        StoryNode::CharacterPlacement { name, x, y, scale } => {
            collect_characters(
                node_id,
                "position",
                &[CharacterPlacementRaw {
                    name: name.clone(),
                    x: Some(*x),
                    y: Some(*y),
                    scale: *scale,
                    ..Default::default()
                }],
                objects,
            );
        }
        StoryNode::Generic(crate::event::EventRaw::SetCharacterPosition(position)) => {
            collect_characters(
                node_id,
                "position",
                &[CharacterPlacementRaw {
                    name: position.name.clone(),
                    x: Some(position.x),
                    y: Some(position.y),
                    scale: position.scale,
                    ..Default::default()
                }],
                objects,
            );
        }
        _ => {}
    }
}

fn background_object(node_id: u32, path: &str) -> LayeredSceneObject {
    LayeredSceneObject {
        object_id: stable_object_id(node_id, "background", path, 0),
        layer_id: "background".to_string(),
        source_node_id: Some(node_id),
        source_field_path: format!("graph.nodes[{node_id}].background"),
        asset_path: Some(path.to_string()),
        character_name: None,
        expression: None,
        object_index: 0,
        x: Some(0),
        y: Some(0),
        scale: Some(1.0),
        z_index: -100,
        visible: true,
        locked: false,
        kind: StageLayerKind::Background,
    }
}

fn collect_characters(
    node_id: u32,
    field: &str,
    characters: &[CharacterPlacementRaw],
    objects: &mut Vec<LayeredSceneObject>,
) {
    for (index, character) in characters.iter().enumerate() {
        let name = character.name.clone();
        let expression = character.expression.clone();
        objects.push(LayeredSceneObject {
            object_id: stable_object_id(
                node_id,
                if field == "patch.update" {
                    "character_update"
                } else {
                    "character"
                },
                &format!("{}:{}", name, expression.as_deref().unwrap_or("")),
                index,
            ),
            layer_id: "character_main".to_string(),
            source_node_id: Some(node_id),
            source_field_path: format!("graph.nodes[{node_id}].{field}[{index}]"),
            asset_path: expression.clone(),
            character_name: Some(name),
            expression,
            object_index: index,
            x: character.x,
            y: character.y,
            scale: character.scale,
            z_index: 0,
            visible: true,
            locked: false,
            kind: StageLayerKind::CharacterMain,
        });
    }
}

fn stable_object_id(node_id: u32, kind: &str, value: &str, index: usize) -> String {
    let mut token = value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    while token.contains("__") {
        token = token.replace("__", "_");
    }
    format!("node:{node_id}:{kind}:{index}:{}", token.trim_matches('_'))
}

fn parse_character_object_id(object_id: &str) -> Option<(u32, usize, bool)> {
    let mut parts = object_id.split(':');
    (parts.next()? == "node").then_some(())?;
    let node_id = parts.next()?.parse().ok()?;
    let update = match parts.next()? {
        "character" => false,
        "character_update" => true,
        _ => return None,
    };
    let index = parts.next()?.parse().ok()?;
    Some((node_id, index, update))
}

fn set_character_pose(
    characters: &mut [CharacterPlacementRaw],
    index: usize,
    x: Option<i32>,
    y: Option<i32>,
    scale: Option<f32>,
) -> bool {
    let Some(character) = characters.get_mut(index) else {
        return false;
    };
    character.x = x;
    character.y = y;
    character.scale = scale;
    true
}
