use std::collections::HashMap;

use crate::editor::StoryNode;
pub use visual_novel_engine::authoring::composer::{
    LayerOverride, LayeredSceneObject, StageLayerKind,
};
use visual_novel_engine::{Engine, Entity, EntityKind, SceneState};

pub fn layered_scene_objects(
    scene: &SceneState,
    entity_owners: &HashMap<u32, u32>,
    engine: &Option<Engine>,
) -> Vec<LayeredSceneObject> {
    let mut objects = Vec::new();
    for (index, entity) in scene.iter_sorted().enumerate() {
        let raw_id = entity.id.raw();
        let (kind, field_suffix, asset_path, character_name, expression) =
            scene_entity_parts(entity, index);
        let source_node_id = entity_owners.get(&raw_id).copied();
        let source_field_path = source_node_id
            .map(|node_id| format!("graph.nodes[{node_id}].visual.{field_suffix}"))
            .unwrap_or_else(|| format!("runtime.scene.entities[{raw_id}]"));
        objects.push(LayeredSceneObject {
            object_id: scene_entity_object_id(entity, source_node_id, index),
            layer_id: format!("{kind:?}"),
            source_node_id,
            source_field_path,
            asset_path,
            character_name,
            expression,
            object_index: index,
            x: Some(entity.transform.x),
            y: Some(entity.transform.y),
            scale: Some(entity.transform.scale as f32 / 1000.0),
            z_index: entity.transform.z_order,
            visible: true,
            locked: false,
            kind,
        });
    }
    if let Some(engine) = engine {
        if let Ok(event) = engine.current_event() {
            match event {
                visual_novel_engine::EventCompiled::Dialogue(_) => objects.push(overlay_object(
                    "dialogue",
                    StageLayerKind::DialogueUi,
                    10_000,
                )),
                visual_novel_engine::EventCompiled::Choice(_) => objects.push(overlay_object(
                    "choice",
                    StageLayerKind::InteractionUi,
                    10_100,
                )),
                visual_novel_engine::EventCompiled::Transition(_) => {
                    objects.push(overlay_object("transition", StageLayerKind::Effects, 9_900))
                }
                _ => {}
            }
        }
    }
    objects
}

pub fn layered_scene_objects_with_authoring_overlay(
    scene: &SceneState,
    entity_owners: &HashMap<u32, u32>,
    engine: &Option<Engine>,
    selected_authoring_node_id: Option<u32>,
    selected_authoring_node: Option<&StoryNode>,
) -> Vec<LayeredSceneObject> {
    let mut objects = layered_scene_objects(scene, entity_owners, engine);
    if let (Some(node_id), Some(node)) = (selected_authoring_node_id, selected_authoring_node) {
        if let Some(overlay) = authoring_overlay_object(node_id, node) {
            objects.retain(|object| object.object_id != overlay.object_id);
            objects.push(overlay);
        }
    }
    objects
}

pub(crate) fn scene_entity_object_id(
    entity: &Entity,
    source_node_id: Option<u32>,
    index: usize,
) -> String {
    let (kind, field_suffix, _, _, _) = scene_entity_parts(entity, index);
    let raw_id = entity.id.raw();
    let source_field_path = source_node_id
        .map(|node_id| format!("graph.nodes[{node_id}].visual.{field_suffix}"))
        .unwrap_or_else(|| format!("runtime.scene.entities[{raw_id}]"));
    stable_entity_object_id(source_node_id, &kind, &source_field_path, index)
}

fn scene_entity_parts(
    entity: &Entity,
    index: usize,
) -> (
    StageLayerKind,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    match &entity.kind {
        EntityKind::Image(image) if entity.transform.z_order <= -50 => (
            StageLayerKind::Background,
            "background".to_string(),
            Some(image.path.as_ref().to_string()),
            None,
            None,
        ),
        EntityKind::Image(image) => (
            StageLayerKind::Environment,
            format!("image[{index}]"),
            Some(image.path.as_ref().to_string()),
            None,
            None,
        ),
        EntityKind::Character(character) if entity.transform.z_order < 0 => (
            StageLayerKind::CharacterBack,
            format!("characters[{index}]"),
            character
                .expression
                .as_ref()
                .map(|value| value.as_ref().to_string()),
            Some(character.name.as_ref().to_string()),
            character
                .expression
                .as_ref()
                .map(|value| value.as_ref().to_string()),
        ),
        EntityKind::Character(character) if entity.transform.z_order > 10 => (
            StageLayerKind::CharacterFront,
            format!("characters[{index}]"),
            character
                .expression
                .as_ref()
                .map(|value| value.as_ref().to_string()),
            Some(character.name.as_ref().to_string()),
            character
                .expression
                .as_ref()
                .map(|value| value.as_ref().to_string()),
        ),
        EntityKind::Character(character) => (
            StageLayerKind::CharacterMain,
            format!("characters[{index}]"),
            character
                .expression
                .as_ref()
                .map(|value| value.as_ref().to_string()),
            Some(character.name.as_ref().to_string()),
            character
                .expression
                .as_ref()
                .map(|value| value.as_ref().to_string()),
        ),
        EntityKind::Text(text) => (
            StageLayerKind::DialogueUi,
            format!("text[{index}]"),
            None,
            None,
            Some(text.content.clone()),
        ),
        EntityKind::Audio(audio) => (
            StageLayerKind::DebugTrace,
            format!("audio[{index}]"),
            Some(audio.path.as_ref().to_string()),
            None,
            None,
        ),
        EntityKind::Video(video) => (
            StageLayerKind::Effects,
            format!("video[{index}]"),
            Some(video.path.as_ref().to_string()),
            None,
            None,
        ),
    }
}

fn overlay_object(name: &str, kind: StageLayerKind, z_index: i32) -> LayeredSceneObject {
    LayeredSceneObject {
        object_id: format!("overlay:{name}"),
        layer_id: format!("{kind:?}"),
        source_node_id: None,
        source_field_path: format!("runtime.current_event.{name}"),
        asset_path: None,
        character_name: None,
        expression: None,
        object_index: 0,
        x: None,
        y: None,
        scale: None,
        z_index,
        visible: true,
        locked: true,
        kind,
    }
}

fn authoring_overlay_object(node_id: u32, node: &StoryNode) -> Option<LayeredSceneObject> {
    match node {
        StoryNode::Dialogue { .. } => Some(overlay_object_with_source(
            "dialogue",
            StageLayerKind::DialogueUi,
            10_000,
            node_id,
            "dialogue",
        )),
        StoryNode::Choice { .. } => Some(overlay_object_with_source(
            "choice",
            StageLayerKind::InteractionUi,
            10_100,
            node_id,
            "choice",
        )),
        _ => None,
    }
}

fn overlay_object_with_source(
    name: &str,
    kind: StageLayerKind,
    z_index: i32,
    node_id: u32,
    field: &str,
) -> LayeredSceneObject {
    LayeredSceneObject {
        object_id: format!("overlay:{name}"),
        layer_id: format!("{kind:?}"),
        source_node_id: Some(node_id),
        source_field_path: format!("graph.nodes[{node_id}].{field}"),
        asset_path: None,
        character_name: None,
        expression: None,
        object_index: 0,
        x: None,
        y: None,
        scale: None,
        z_index,
        visible: true,
        locked: true,
        kind,
    }
}

fn stable_entity_object_id(
    source_node_id: Option<u32>,
    kind: &StageLayerKind,
    field_path: &str,
    index: usize,
) -> String {
    let owner = source_node_id
        .map(|id| format!("node:{id}"))
        .unwrap_or_else(|| "runtime".to_string());
    let mut token = field_path
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    while token.contains("__") {
        token = token.replace("__", "_");
    }
    format!("{owner}:{kind:?}:{index}:{}", token.trim_matches('_'))
}
