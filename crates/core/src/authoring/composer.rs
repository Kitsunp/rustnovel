use serde::{Deserialize, Serialize};

use crate::{
    CharacterPlacementRaw, Engine, EventCompiled, LocalizationCatalog, ResourceLimiter,
    ScenePatchRaw, SecurityPolicy, VnResult,
};

use super::{NodeGraph, StoryNode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerOverride {
    pub visible: bool,
    pub locked: bool,
}

impl Default for LayerOverride {
    fn default() -> Self {
        Self {
            visible: true,
            locked: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayeredSceneObject {
    pub object_id: String,
    pub layer_id: String,
    pub source_node_id: Option<u32>,
    pub source_field_path: String,
    pub asset_path: Option<String>,
    pub character_name: Option<String>,
    pub expression: Option<String>,
    pub object_index: usize,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub scale: Option<f32>,
    pub z_index: i32,
    pub visible: bool,
    pub locked: bool,
    pub kind: StageLayerKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "overlay_kind", rename_all = "snake_case")]
pub enum ComposerOverlay {
    Dialogue {
        speaker: String,
        text: String,
    },
    Choice {
        prompt: String,
        options: Vec<String>,
    },
    Transition {
        kind: String,
        duration_ms: u32,
    },
    DebugTrace {
        label: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComposerSnapshot {
    pub schema: String,
    pub stage_width: u32,
    pub stage_height: u32,
    pub objects: Vec<LayeredSceneObject>,
    pub overlays: Vec<ComposerOverlay>,
}

pub fn list_stage_layers() -> Vec<StageLayerKind> {
    vec![
        StageLayerKind::Background,
        StageLayerKind::Environment,
        StageLayerKind::CharacterBack,
        StageLayerKind::CharacterMain,
        StageLayerKind::CharacterFront,
        StageLayerKind::Effects,
        StageLayerKind::DialogueUi,
        StageLayerKind::InteractionUi,
        StageLayerKind::DebugTrace,
    ]
}

pub fn compose_scene_snapshot(
    graph: &NodeGraph,
    selected_node_id: Option<u32>,
    stage_resolution: Option<(u32, u32)>,
    engine: Option<&Engine>,
    locale: Option<&str>,
    catalog: Option<&LocalizationCatalog>,
) -> ComposerSnapshot {
    let (stage_width, stage_height) = stage_resolution.unwrap_or((1280, 720));
    let mut objects = Vec::new();
    for (node_id, node, _) in graph.nodes() {
        if selected_node_id.is_some() && selected_node_id != Some(*node_id) {
            continue;
        }
        collect_node_objects(*node_id, node, &mut objects);
    }
    let mut overlays = Vec::new();
    if let Some(engine) = engine {
        if let Ok(event) = engine.current_event() {
            overlays.extend(overlays_from_event(&event, locale.unwrap_or("en"), catalog));
        }
    }
    if overlays.is_empty() {
        if let Some(node_id) = selected_node_id {
            if let Some(node) = graph.get_node(node_id) {
                overlays.extend(overlays_from_authoring_node(
                    node,
                    locale.unwrap_or("en"),
                    catalog,
                ));
            }
        }
    }
    ComposerSnapshot {
        schema: "vnengine.composer_snapshot.v1".to_string(),
        stage_width,
        stage_height,
        objects,
        overlays,
    }
}

pub fn list_layered_objects(
    graph: &NodeGraph,
    selected_node_id: Option<u32>,
) -> Vec<LayeredSceneObject> {
    compose_scene_snapshot(graph, selected_node_id, None, None, None, None).objects
}

pub fn set_layer_visible(
    overrides: &mut std::collections::BTreeMap<String, LayerOverride>,
    object_id: &str,
    visible: bool,
) {
    overrides.entry(object_id.to_string()).or_default().visible = visible;
}

pub fn set_layer_locked(
    overrides: &mut std::collections::BTreeMap<String, LayerOverride>,
    object_id: &str,
    locked: bool,
) {
    overrides.entry(object_id.to_string()).or_default().locked = locked;
}

pub fn apply_layer_overrides(
    objects: &mut [LayeredSceneObject],
    overrides: &std::collections::BTreeMap<String, LayerOverride>,
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
    let Some((node_id, index)) = parse_character_object_id(object_id) else {
        return false;
    };
    let Some(node) = graph.get_node_mut(node_id) else {
        return false;
    };
    match node {
        StoryNode::Scene { characters, .. } => move_character(characters, index, x, y, scale),
        StoryNode::ScenePatch(ScenePatchRaw { add, .. }) => move_character(add, index, x, y, scale),
        _ => false,
    }
}

#[derive(Clone, Debug)]
pub struct ComposerPreviewSession {
    engine: Engine,
}

impl ComposerPreviewSession {
    pub fn start_from_node(graph: &NodeGraph, node_id: u32) -> VnResult<Self> {
        let script = graph.to_script_strict()?;
        let mut engine = Engine::new(
            script,
            SecurityPolicy::default(),
            ResourceLimiter::default(),
        )?;
        engine.jump_to_label(&format!("node_{node_id}"))?;
        Ok(Self { engine })
    }

    pub fn advance(&mut self) -> VnResult<()> {
        match self.engine.current_event()? {
            EventCompiled::Choice(_) => Ok(()),
            EventCompiled::ExtCall { .. } => self.engine.resume(),
            _ => self.engine.step().map(|_| ()),
        }
    }

    pub fn choose(&mut self, option_index: usize) -> VnResult<()> {
        self.engine.choose(option_index).map(|_| ())
    }

    pub fn snapshot(
        &self,
        graph: &NodeGraph,
        stage_resolution: Option<(u32, u32)>,
        locale: Option<&str>,
        catalog: Option<&LocalizationCatalog>,
    ) -> ComposerSnapshot {
        compose_scene_snapshot(
            graph,
            None,
            stage_resolution,
            Some(&self.engine),
            locale,
            catalog,
        )
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

fn overlays_from_event(
    event: &EventCompiled,
    locale: &str,
    catalog: Option<&LocalizationCatalog>,
) -> Vec<ComposerOverlay> {
    match event {
        EventCompiled::Dialogue(dialogue) => vec![ComposerOverlay::Dialogue {
            speaker: localize(dialogue.speaker.as_ref(), locale, catalog),
            text: localize(dialogue.text.as_ref(), locale, catalog),
        }],
        EventCompiled::Choice(choice) => vec![ComposerOverlay::Choice {
            prompt: localize(choice.prompt.as_ref(), locale, catalog),
            options: choice
                .options
                .iter()
                .map(|option| localize(option.text.as_ref(), locale, catalog))
                .collect(),
        }],
        EventCompiled::Transition(transition) => vec![ComposerOverlay::Transition {
            kind: transition.kind.to_string(),
            duration_ms: transition.duration_ms,
        }],
        _ => Vec::new(),
    }
}

fn overlays_from_authoring_node(
    node: &StoryNode,
    locale: &str,
    catalog: Option<&LocalizationCatalog>,
) -> Vec<ComposerOverlay> {
    match node {
        StoryNode::Dialogue { speaker, text } => vec![ComposerOverlay::Dialogue {
            speaker: localize(speaker, locale, catalog),
            text: localize(text, locale, catalog),
        }],
        StoryNode::Choice { prompt, options } => vec![ComposerOverlay::Choice {
            prompt: localize(prompt, locale, catalog),
            options: options
                .iter()
                .map(|option| localize(option, locale, catalog))
                .collect(),
        }],
        _ => Vec::new(),
    }
}

fn localize(value: &str, locale: &str, catalog: Option<&LocalizationCatalog>) -> String {
    if let Some(key) = crate::localization_key(value) {
        catalog
            .map(|catalog| catalog.resolve_or_key(locale, key))
            .unwrap_or_else(|| key.to_string())
    } else {
        value.to_string()
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

fn move_character(
    characters: &mut [CharacterPlacementRaw],
    index: usize,
    x: i32,
    y: i32,
    scale: Option<f32>,
) -> bool {
    let Some(character) = characters.get_mut(index) else {
        return false;
    };
    character.x = Some(x);
    character.y = Some(y);
    character.scale = scale;
    true
}
