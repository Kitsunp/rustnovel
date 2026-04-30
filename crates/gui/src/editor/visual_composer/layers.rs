use std::collections::HashMap;

use visual_novel_engine::{Engine, EntityKind, SceneState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StageLayerKind {
    Background,
    Environment,
    CharacterBack,
    CharacterMain,
    CharacterFront,
    Effects,
    DialogueUi,
    InteractionUi,
    DebugTrace,
}

impl StageLayerKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Background => "Background",
            Self::Environment => "Environment",
            Self::CharacterBack => "Character Back",
            Self::CharacterMain => "Character Main",
            Self::CharacterFront => "Character Front",
            Self::Effects => "Effects",
            Self::DialogueUi => "Dialogue UI",
            Self::InteractionUi => "Interaction UI",
            Self::DebugTrace => "Debug Trace",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayeredSceneObject {
    pub object_id: String,
    pub layer_id: String,
    pub source_node_id: Option<u32>,
    pub source_field_path: String,
    pub z_index: i32,
    pub visible: bool,
    pub locked: bool,
    pub kind: StageLayerKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayerOverride {
    pub visible: bool,
    pub locked: bool,
}

pub fn layered_scene_objects(
    scene: &SceneState,
    entity_owners: &HashMap<u32, u32>,
    engine: &Option<Engine>,
) -> Vec<LayeredSceneObject> {
    let mut objects = Vec::new();
    for entity in scene.iter_sorted() {
        let raw_id = entity.id.raw();
        let (kind, field_suffix) = match &entity.kind {
            EntityKind::Image(image) if entity.transform.z_order <= -50 => {
                (StageLayerKind::Background, image.path.as_ref().to_string())
            }
            EntityKind::Image(image) => {
                (StageLayerKind::Environment, image.path.as_ref().to_string())
            }
            EntityKind::Character(character) if entity.transform.z_order < 0 => (
                StageLayerKind::CharacterBack,
                character.name.as_ref().to_string(),
            ),
            EntityKind::Character(character) if entity.transform.z_order > 10 => (
                StageLayerKind::CharacterFront,
                character.name.as_ref().to_string(),
            ),
            EntityKind::Character(character) => (
                StageLayerKind::CharacterMain,
                character.name.as_ref().to_string(),
            ),
            EntityKind::Text(text) => (StageLayerKind::DialogueUi, text.content.clone()),
            EntityKind::Audio(audio) => {
                (StageLayerKind::DebugTrace, audio.path.as_ref().to_string())
            }
            EntityKind::Video(video) => (StageLayerKind::Effects, video.path.as_ref().to_string()),
        };
        let source_node_id = entity_owners.get(&raw_id).copied();
        objects.push(LayeredSceneObject {
            object_id: format!("entity:{raw_id}"),
            layer_id: format!("{kind:?}"),
            source_node_id,
            source_field_path: source_node_id
                .map(|node_id| format!("graph.nodes[{node_id}].visual.{field_suffix}"))
                .unwrap_or_else(|| format!("runtime.scene.entities[{raw_id}]")),
            z_index: entity.transform.z_order,
            visible: true,
            locked: false,
            kind,
        });
    }
    if let Some(engine) = engine {
        if let Ok(event) = engine.current_event() {
            match event {
                visual_novel_engine::EventCompiled::Dialogue(_) => {
                    objects.push(LayeredSceneObject {
                        object_id: "overlay:dialogue".to_string(),
                        layer_id: "dialogue_ui".to_string(),
                        source_node_id: None,
                        source_field_path: "runtime.current_event.dialogue".to_string(),
                        z_index: 10_000,
                        visible: true,
                        locked: true,
                        kind: StageLayerKind::DialogueUi,
                    })
                }
                visual_novel_engine::EventCompiled::Choice(_) => objects.push(LayeredSceneObject {
                    object_id: "overlay:choice".to_string(),
                    layer_id: "interaction_ui".to_string(),
                    source_node_id: None,
                    source_field_path: "runtime.current_event.choice".to_string(),
                    z_index: 10_100,
                    visible: true,
                    locked: true,
                    kind: StageLayerKind::InteractionUi,
                }),
                _ => {}
            }
        }
    }
    objects
}
