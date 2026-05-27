use serde::{Deserialize, Serialize};

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
