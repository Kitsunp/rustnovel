use std::collections::{BTreeMap, VecDeque};

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundFit {
    #[default]
    Cover,
    Contain,
    Stretch,
    Tile,
    Original,
}

impl BackgroundFit {
    pub const ALL: &'static [Self] = &[
        Self::Cover,
        Self::Contain,
        Self::Stretch,
        Self::Tile,
        Self::Original,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Cover => "Cover",
            Self::Contain => "Contain",
            Self::Stretch => "Stretch",
            Self::Tile => "Tile",
            Self::Original => "Original",
        }
    }
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresentationRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresentationLayout {
    pub dialogue_rect: Option<PresentationRect>,
    pub choices_rect: Option<PresentationRect>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationTransition {
    pub kind: String,
    pub duration_ms: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresentationSnapshot {
    pub schema: String,
    pub stage_width: u32,
    pub stage_height: u32,
    pub safe_area: PresentationRect,
    pub layout: PresentationLayout,
    pub visual_background: Option<String>,
    pub visual_music: Option<String>,
    pub visual_character_count: usize,
    pub transition: Option<PresentationTransition>,
    pub objects: Vec<LayeredSceneObject>,
    pub overlays: Vec<ComposerOverlay>,
    pub provenance: Vec<String>,
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
    let preview_engine =
        engine.map(|engine| preview_engine_for_selection(engine, graph, selected_node_id));
    let snapshot_engine = preview_engine.as_ref().or(engine);
    let mut objects = snapshot_engine
        .map(|engine| collect_visual_objects(graph, engine))
        .unwrap_or_default();
    if objects.is_empty() {
        collect_authoring_objects(graph, selected_node_id, &mut objects);
    }
    let mut overlays = Vec::new();
    if let Some(node_id) = selected_node_id {
        if let Some(node) = graph.get_node(node_id) {
            overlays.extend(overlays_from_authoring_node(
                node,
                locale.unwrap_or("en"),
                catalog,
            ));
        }
    }
    if overlays.is_empty() {
        if let Some(engine) = snapshot_engine {
            if let Ok(event) = engine.current_event() {
                overlays.extend(overlays_from_event(&event, locale.unwrap_or("en"), catalog));
            }
        }
    }
    if overlays.is_empty() {
        if let Some(engine) = engine {
            if let Ok(event) = engine.current_event() {
                overlays.extend(overlays_from_event(&event, locale.unwrap_or("en"), catalog));
            }
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
    let overlay_source = selected_node_id
        .and_then(|node_id| {
            graph
                .get_node(node_id)
                .and_then(|node| overlay_source_for_authoring_node(node_id, node))
        })
        .or_else(|| {
            snapshot_engine.and_then(|engine| {
                graph
                    .node_for_event_ip(engine.state().position)
                    .and_then(|node_id| {
                        graph
                            .get_node(node_id)
                            .and_then(|node| overlay_source_for_authoring_node(node_id, node))
                    })
            })
        });
    objects.extend(overlay_layer_objects(&overlays, overlay_source));
    ComposerSnapshot {
        schema: "vnengine.composer_snapshot.v1".to_string(),
        stage_width,
        stage_height,
        objects,
        overlays,
    }
}

fn collect_authoring_objects(
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

pub fn build_presentation_snapshot(
    graph: &NodeGraph,
    selected_node_id: Option<u32>,
    stage_resolution: Option<(u32, u32)>,
    engine: Option<&Engine>,
    locale: Option<&str>,
    catalog: Option<&LocalizationCatalog>,
) -> PresentationSnapshot {
    let composer = compose_scene_snapshot(
        graph,
        selected_node_id,
        stage_resolution,
        engine,
        locale,
        catalog,
    );
    PresentationSnapshot::from_composer_snapshot(composer, engine)
}

impl PresentationSnapshot {
    pub fn from_composer_snapshot(snapshot: ComposerSnapshot, engine: Option<&Engine>) -> Self {
        let safe_area = calculate_safe_area(snapshot.stage_width, snapshot.stage_height);
        let layout = calculate_overlay_layout(&snapshot.overlays, &safe_area);
        let transition = snapshot.overlays.iter().find_map(|overlay| match overlay {
            ComposerOverlay::Transition { kind, duration_ms } => Some(PresentationTransition {
                kind: kind.clone(),
                duration_ms: *duration_ms,
            }),
            _ => None,
        });
        let provenance = snapshot
            .objects
            .iter()
            .map(|object| object.source_field_path.clone())
            .collect();
        let visual = engine.map(|engine| engine.visual_state());

        Self {
            schema: "vnengine.presentation_snapshot.v1".to_string(),
            stage_width: snapshot.stage_width,
            stage_height: snapshot.stage_height,
            safe_area,
            layout,
            visual_background: visual
                .and_then(|state| state.background.as_ref())
                .map(|value| value.as_ref().to_string()),
            visual_music: visual
                .and_then(|state| state.music.as_ref())
                .map(|value| value.as_ref().to_string()),
            visual_character_count: visual.map_or(0, |state| state.characters.len()),
            transition,
            objects: snapshot.objects,
            overlays: snapshot.overlays,
            provenance,
        }
    }
}

fn calculate_safe_area(stage_width: u32, stage_height: u32) -> PresentationRect {
    let margin_x = stage_width as f32 * 0.05;
    let margin_y = stage_height as f32 * 0.05;
    PresentationRect {
        x: margin_x,
        y: margin_y,
        width: (stage_width as f32 - margin_x * 2.0).max(0.0),
        height: (stage_height as f32 - margin_y * 2.0).max(0.0),
    }
}

fn calculate_overlay_layout(
    overlays: &[ComposerOverlay],
    safe_area: &PresentationRect,
) -> PresentationLayout {
    let has_dialogue = overlays
        .iter()
        .any(|overlay| matches!(overlay, ComposerOverlay::Dialogue { .. }));
    let has_choices = overlays
        .iter()
        .any(|overlay| matches!(overlay, ComposerOverlay::Choice { .. }));

    PresentationLayout {
        dialogue_rect: has_dialogue.then_some(PresentationRect {
            x: safe_area.x,
            y: safe_area.y + safe_area.height * 0.72,
            width: safe_area.width,
            height: safe_area.height * 0.24,
        }),
        choices_rect: has_choices.then_some(PresentationRect {
            x: safe_area.x + safe_area.width * 0.12,
            y: safe_area.y + safe_area.height * 0.18,
            width: safe_area.width * 0.76,
            height: safe_area.height * 0.56,
        }),
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

    pub fn presentation_snapshot(
        &self,
        graph: &NodeGraph,
        stage_resolution: Option<(u32, u32)>,
        locale: Option<&str>,
        catalog: Option<&LocalizationCatalog>,
    ) -> PresentationSnapshot {
        build_presentation_snapshot(
            graph,
            None,
            stage_resolution,
            Some(&self.engine),
            locale,
            catalog,
        )
    }
}

fn preview_engine_for_selection(
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

#[derive(Default)]
struct PresentationOwnerHints {
    background_owner: Option<u32>,
    music_owner: Option<u32>,
    character_owners: BTreeMap<String, VecDeque<u32>>,
}

fn collect_visual_objects(graph: &NodeGraph, engine: &Engine) -> Vec<LayeredSceneObject> {
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

fn overlay_source_for_authoring_node(
    node_id: u32,
    node: &StoryNode,
) -> Option<(Option<u32>, &'static str)> {
    match node {
        StoryNode::Dialogue { .. } => Some((Some(node_id), "dialogue")),
        StoryNode::Choice { .. } => Some((Some(node_id), "choice")),
        StoryNode::Transition { .. } => Some((Some(node_id), "transition")),
        _ => None,
    }
}

fn overlay_layer_objects(
    overlays: &[ComposerOverlay],
    source: Option<(Option<u32>, &'static str)>,
) -> Vec<LayeredSceneObject> {
    overlays
        .iter()
        .map(|overlay| {
            let (name, kind, z_index) = match overlay {
                ComposerOverlay::Dialogue { .. } => {
                    ("dialogue", StageLayerKind::DialogueUi, 10_000)
                }
                ComposerOverlay::Choice { .. } => ("choice", StageLayerKind::InteractionUi, 10_100),
                ComposerOverlay::Transition { .. } => {
                    ("transition", StageLayerKind::Effects, 9_900)
                }
                ComposerOverlay::DebugTrace { .. } => {
                    ("debug_trace", StageLayerKind::DebugTrace, 10_200)
                }
            };
            let (source_node_id, field) = source.unwrap_or((None, name));
            LayeredSceneObject {
                object_id: format!("overlay:{name}"),
                layer_id: format!("{kind:?}"),
                source_node_id,
                source_field_path: source_node_id
                    .map(|node_id| format!("graph.nodes[{node_id}].{field}"))
                    .unwrap_or_else(|| format!("runtime.current_event.{name}")),
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
        })
        .collect()
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
        StoryNode::Transition {
            kind, duration_ms, ..
        } => vec![ComposerOverlay::Transition {
            kind: kind.clone(),
            duration_ms: *duration_ms,
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
