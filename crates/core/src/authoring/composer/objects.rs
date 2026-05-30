use std::collections::BTreeMap;

use crate::event::{CharacterPlacementRaw, EventCompiled, ScenePatchRaw};
use crate::resource::ResourceLimiter;
use crate::runtime::Engine;

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
    let Some((node_id, index)) = parse_character_object_id(object_id) else {
        return false;
    };
    let Some(node) = graph.get_node_mut(node_id) else {
        return false;
    };
    match node {
        StoryNode::Scene { characters, .. } => set_character_pose(characters, index, x, y, scale),
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

pub(super) fn preview_engine_for_selection(
    engine: &Engine,
    graph: &NodeGraph,
    selected_node_id: Option<u32>,
) -> Engine {
    let Some(target_ip) = selected_node_id.and_then(|node_id| graph.event_ip_for_node(node_id))
    else {
        return engine.clone();
    };
    let mut preview = Engine::from_compiled(
        engine.script().clone(),
        engine.policy().clone(),
        ResourceLimiter::default(),
    )
    .unwrap_or_else(|_| engine.clone());
    let max_steps = (target_ip as usize).saturating_add(64).min(4096);
    for _ in 0..max_steps {
        let current_ip = preview.state().position;
        if current_ip > target_ip {
            break;
        }
        let Ok(event) = preview.current_event() else {
            break;
        };
        let advanced_ok = match &event {
            EventCompiled::ExtCall { .. } => preview.resume().is_ok(),
            EventCompiled::Choice(choice) => {
                if choice.options.is_empty() {
                    false
                } else {
                    preview.choose(0).is_ok()
                }
            }
            EventCompiled::Dialogue(_)
            | EventCompiled::Scene(_)
            | EventCompiled::Patch(_)
            | EventCompiled::SetCharacterPosition(_)
            | EventCompiled::Transition(_)
            | EventCompiled::Jump { .. }
            | EventCompiled::SetFlag { .. }
            | EventCompiled::SetVar { .. }
            | EventCompiled::JumpIf { .. }
            | EventCompiled::AudioAction(_) => preview.step().is_ok(),
        };
        if !advanced_ok || preview.state().position > target_ip {
            break;
        }
    }
    preview
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
                "character",
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

fn parse_character_object_id(object_id: &str) -> Option<(u32, usize)> {
    let mut parts = object_id.split(':');
    (parts.next()? == "node").then_some(())?;
    let node_id = parts.next()?.parse().ok()?;
    (parts.next()? == "character").then_some(())?;
    let index = parts.next()?.parse().ok()?;
    Some((node_id, index))
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
