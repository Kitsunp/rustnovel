use std::collections::{BTreeMap, VecDeque};

use crate::event::EventCompiled;
use crate::runtime::Engine;

use super::super::{NodeGraph, StoryNode};
use super::{LayeredSceneObject, StageLayerKind};

#[derive(Default)]
struct PresentationOwnerHints {
    background_owner: Option<u32>,
    music_owner: Option<u32>,
    character_owners: BTreeMap<String, VecDeque<u32>>,
}

pub(super) fn collect_visual_objects(
    graph: &NodeGraph,
    engine: &Engine,
) -> Vec<LayeredSceneObject> {
    let visual = engine.visual_state();
    let mut owners = presentation_owner_hints(graph, engine);
    let mut objects = Vec::new();

    if let Some(background) = &visual.background {
        let asset = background.as_ref();
        let owner = owners
            .background_owner
            .or_else(|| first_node_referencing_asset(graph, asset));
        objects.push(visual_background_object(owner, asset));
    }

    for (index, character) in visual.characters.iter().enumerate() {
        let owner = pop_character_owner(
            &mut owners,
            character.name.as_ref(),
            character.expression.as_deref(),
        )
        .or_else(|| {
            character
                .expression
                .as_ref()
                .and_then(|expr| first_node_referencing_asset(graph, expr.as_ref()))
        });
        objects.push(LayeredSceneObject {
            object_id: runtime_object_id(
                owner,
                &StageLayerKind::CharacterMain,
                &runtime_source_path(owner, &format!("visual.characters[{index}]")),
                index,
            ),
            layer_id: format!("{:?}", StageLayerKind::CharacterMain),
            source_node_id: owner,
            source_field_path: runtime_source_path(owner, &format!("visual.characters[{index}]")),
            asset_path: character
                .expression
                .as_ref()
                .map(|value| value.as_ref().to_string()),
            character_name: Some(character.name.as_ref().to_string()),
            expression: character
                .expression
                .as_ref()
                .map(|value| value.as_ref().to_string()),
            object_index: index,
            x: character.x,
            y: character.y,
            scale: character.scale,
            z_index: index as i32,
            visible: true,
            locked: false,
            kind: StageLayerKind::CharacterMain,
        });
    }

    if let Some(music) = &visual.music {
        let asset = music.as_ref();
        let owner = owners
            .music_owner
            .or_else(|| first_node_referencing_asset(graph, asset));
        objects.push(visual_audio_object(owner, asset));
    }

    objects
}

fn presentation_owner_hints(graph: &NodeGraph, engine: &Engine) -> PresentationOwnerHints {
    let mut hints = PresentationOwnerHints::default();
    let upper_bound = engine.state().position;
    for (idx, event) in engine.script().events.iter().enumerate() {
        let ip = idx as u32;
        if ip > upper_bound {
            break;
        }
        let owner = graph.node_for_event_ip(ip);
        match event {
            EventCompiled::Scene(scene) => {
                if scene.background.is_some() {
                    hints.background_owner = owner;
                }
                if scene.music.is_some() {
                    hints.music_owner = owner;
                }
                if let Some(owner_id) = owner {
                    for character in &scene.characters {
                        push_character_owner(
                            &mut hints,
                            character.name.as_ref(),
                            character.expression.as_deref(),
                            owner_id,
                        );
                    }
                }
            }
            EventCompiled::Patch(patch) => {
                if patch.background.is_some() {
                    hints.background_owner = owner;
                }
                if patch.music.is_some() {
                    hints.music_owner = owner;
                }
                if let Some(owner_id) = owner {
                    for character in &patch.add {
                        push_character_owner(
                            &mut hints,
                            character.name.as_ref(),
                            character.expression.as_deref(),
                            owner_id,
                        );
                    }
                    for character in &patch.update {
                        push_character_owner(
                            &mut hints,
                            character.name.as_ref(),
                            character.expression.as_deref(),
                            owner_id,
                        );
                    }
                }
                for removed_name in &patch.remove {
                    remove_character_owner_name(&mut hints, removed_name.as_ref());
                }
            }
            EventCompiled::AudioAction(action) if action.channel == 0 => {
                hints.music_owner = owner;
            }
            EventCompiled::SetCharacterPosition(pos) => {
                if let Some(owner_id) = owner {
                    push_character_owner(&mut hints, pos.name.as_ref(), None, owner_id);
                }
            }
            _ => {}
        }
    }
    hints
}

fn visual_background_object(owner: Option<u32>, path: &str) -> LayeredSceneObject {
    let source_field_path = runtime_source_path(owner, "visual.background");
    LayeredSceneObject {
        object_id: runtime_object_id(owner, &StageLayerKind::Background, &source_field_path, 0),
        layer_id: format!("{:?}", StageLayerKind::Background),
        source_node_id: owner,
        source_field_path,
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

fn visual_audio_object(owner: Option<u32>, path: &str) -> LayeredSceneObject {
    let source_field_path = runtime_source_path(owner, "visual.music");
    LayeredSceneObject {
        object_id: runtime_object_id(owner, &StageLayerKind::DebugTrace, &source_field_path, 0),
        layer_id: format!("{:?}", StageLayerKind::DebugTrace),
        source_node_id: owner,
        source_field_path,
        asset_path: Some(path.to_string()),
        character_name: None,
        expression: None,
        object_index: 0,
        x: Some(12),
        y: Some(12),
        scale: Some(1.0),
        z_index: 500,
        visible: true,
        locked: false,
        kind: StageLayerKind::DebugTrace,
    }
}

fn runtime_source_path(owner: Option<u32>, field: &str) -> String {
    owner
        .map(|node_id| format!("graph.nodes[{node_id}].{field}"))
        .unwrap_or_else(|| format!("runtime.{field}"))
}

fn runtime_object_id(
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

fn push_character_owner(
    hints: &mut PresentationOwnerHints,
    name: &str,
    expression: Option<&str>,
    owner_id: u32,
) {
    hints
        .character_owners
        .entry(character_key(name, expression))
        .or_default()
        .push_back(owner_id);
}

fn pop_character_owner(
    hints: &mut PresentationOwnerHints,
    name: &str,
    expression: Option<&str>,
) -> Option<u32> {
    let key = character_key(name, expression);
    if let Some(queue) = hints.character_owners.get_mut(&key) {
        if let Some(owner) = queue.pop_front() {
            return Some(owner);
        }
    }

    let fallback_key = character_key(name, None);
    hints
        .character_owners
        .get_mut(&fallback_key)
        .and_then(VecDeque::pop_front)
}

fn remove_character_owner_name(hints: &mut PresentationOwnerHints, name: &str) {
    let prefix = format!("{}|", name.trim());
    hints
        .character_owners
        .retain(|key, _| !key.starts_with(&prefix));
}

fn character_key(name: &str, expression: Option<&str>) -> String {
    format!("{}|{}", name.trim(), expression.unwrap_or("").trim())
}

fn first_node_referencing_asset(graph: &NodeGraph, asset: &str) -> Option<u32> {
    graph
        .nodes()
        .find_map(|(node_id, node, _)| node_references_asset(node, asset).then_some(*node_id))
}

fn node_references_asset(node: &StoryNode, asset: &str) -> bool {
    match node {
        StoryNode::Scene {
            background,
            music,
            characters,
            ..
        } => {
            background.as_deref() == Some(asset)
                || music.as_deref() == Some(asset)
                || characters
                    .iter()
                    .any(|character| character.expression.as_deref() == Some(asset))
        }
        StoryNode::ScenePatch(patch) => {
            patch.background.as_deref() == Some(asset)
                || patch.music.as_deref() == Some(asset)
                || patch
                    .add
                    .iter()
                    .any(|character| character.expression.as_deref() == Some(asset))
                || patch
                    .update
                    .iter()
                    .any(|character| character.expression.as_deref() == Some(asset))
        }
        StoryNode::AudioAction {
            asset: Some(audio), ..
        } => audio == asset,
        _ => false,
    }
}
